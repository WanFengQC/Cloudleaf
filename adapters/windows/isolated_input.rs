//! 普通Win32隔离宿主，只写本窗口的文档，不取得任何TSF manager/context。
use super::*;
use cloudleaf_translation_ipc::{Direction, MAX_INPUT_BYTES};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use windows::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::EM_SETLIMITTEXT,
        Input::{
            Ime::{HIMC, IACE_CHILDREN, ImmAssociateContextEx},
            KeyboardAndMouse::*,
        },
        WindowsAndMessaging::*,
    },
};
use windows_core::{PCWSTR, w};
#[path = "isolated_translation.rs"]
mod translation;
use translation::{Captured, Controller as TranslationController};
#[path = "user_panel.rs"]
mod user_panel;
#[path = "user_worker.rs"]
mod user_worker;

#[derive(Default, Debug)]
pub(crate) struct Sample {
    selection: Option<cloudleaf_input::SelectionEvidence>,
    pub core_ns: u128,
    pub projection_ns: u128,
    pub comparison_ns: u128,
    pub transaction_ns: u128,
    pub frame_ns: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TranslationReceipt {
    actual: u64,
    projected: u64,
    outcome: CommitOutcome,
}

pub(crate) struct LocalHost {
    pub host: Shared,
    document: RefCell<String>,
    pub retained: RefCell<String>,
}

impl LocalHost {
    pub fn new(host: Shared) -> Result<Self> {
        host.check()?;
        if !host.reserve_epoch() {
            return Err(Error::new(E_FAIL, "隔离会话预算不足"));
        }
        host.engine.borrow_mut().handle(InputEvent::Activate);
        host.projection.borrow_mut().handle(InputEvent::Activate);
        Ok(Self {
            host,
            document: RefCell::new(String::new()),
            retained: RefCell::new(String::new()),
        })
    }
    pub fn reset(&self) -> Result<()> {
        // 普通窗口没有COM上下文；先失效旧epoch，再重新激活，绝不重放未知事务。
        self.host.invalidate()?;
        if !self.host.reserve_epoch() {
            return Err(Error::new(E_FAIL, "新隔离会话预算不足"));
        }
        self.host.engine.borrow_mut().handle(InputEvent::Activate);
        self.host
            .projection
            .borrow_mut()
            .handle(InputEvent::Activate);
        self.retained.borrow_mut().clear();
        Ok(())
    }
    pub fn switch_scheme(&self, config: cloudleaf_input::SchemeConfig) -> Result<InputFrame> {
        self.host.check()?;
        if self.host.quarantined.get() {
            return Err(Error::new(E_FAIL, "提交结果未知，不能切换输入方案"));
        }
        let mut actual = self.host.engine.borrow_mut();
        let mut projected = self.host.projection.borrow_mut();
        // 两份只读预检全部成功后才替换，避免第二引擎拒绝时留下半次设置变化。
        actual
            .validate_scheme_config(&config)
            .map_err(|_| Error::new(E_FAIL, "当前事务或方案配置不允许切换"))?;
        projected
            .validate_scheme_config(&config)
            .map_err(|_| Error::new(E_FAIL, "投影事务或方案配置不允许切换"))?;
        actual
            .reconfigure_scheme(config.clone())
            .map_err(|_| Error::new(E_FAIL, "输入方案应用失败"))?;
        if projected.reconfigure_scheme(config).is_err() {
            self.host.quarantined.set(true);
            return Err(Error::new(E_FAIL, "双引擎设置未一致，已隔离且未写文档"));
        }
        let frame = actual.frame();
        let mut projection_frame = projected.frame();
        projection_frame.revision = frame.revision;
        if frame != projection_frame {
            self.host.quarantined.set(true);
            return Err(Error::new(E_FAIL, "双引擎方案候选不一致，已隔离且未写文档"));
        }
        Ok(frame)
    }

    fn retain(&self, material: &str) -> Result<()> {
        let mut retained = self.retained.borrow_mut();
        let separator = if retained.is_empty() { "" } else { "\r\n" };
        if retained.len() + separator.len() + material.len() > 16384 {
            self.host.quarantined.set(true);
            return Err(Error::new(
                E_FAIL,
                "保全材料已满；当前组合保留，核对后再开始新会话",
            ));
        }
        retained.push_str(separator);
        retained.push_str(material);
        Ok(())
    }
    fn finish_commit(
        &self,
        actual: &EventResult,
        projected: &EventResult,
        translation_source: Option<&str>,
        sink: &dyn Fn(&str) -> CommitOutcome,
    ) -> Result<(
        TranslationReceipt,
        Option<cloudleaf_input::SelectionEvidence>,
    )> {
        let commit = actual.commit.as_ref().expect("宿主仅在双引擎确认后提交");
        let projected = projected.commit.as_ref().expect("双引擎提交已比对");
        let mut receipt = TranslationReceipt {
            actual: commit.id,
            projected: projected.id,
            outcome: CommitOutcome::NotApplied,
        };
        let mut document = self.document.borrow().clone();
        document.push_str(&commit.text);
        if document.len() <= 16384 {
            receipt.outcome = sink(&document);
        }
        let selection = if receipt.outcome == CommitOutcome::Applied && translation_source.is_none()
        {
            self.host.engine.borrow().pending_selection().cloned()
        } else {
            None
        };
        let applied = receipt.outcome == CommitOutcome::Applied;
        if receipt.outcome != CommitOutcome::Unknown {
            if applied {
                *self.document.borrow_mut() = document;
            }
            let ack = |id| {
                if applied {
                    InputEvent::CommitSucceeded { id }
                } else {
                    InputEvent::CommitFailed { id }
                }
            };
            let actual_ack = self.host.engine.borrow_mut().handle(ack(commit.id));
            let projected_ack = self.host.projection.borrow_mut().handle(ack(projected.id));
            if translation_source.is_some()
                && (!same_semantics(&actual_ack, &projected_ack)
                    || !actual_ack.handled
                    || (applied && actual_ack.error.is_some())
                    || (!applied
                        && self
                            .host
                            .engine
                            .borrow()
                            .validate_failed_translation(commit.id)
                            .is_err()))
            {
                receipt.outcome = CommitOutcome::Unknown;
            }
        }
        if receipt.outcome == CommitOutcome::Unknown {
            self.host.quarantined.set(true);
            self.host.unknown_text.set(true);
            if let Some(source) = translation_source {
                // 翻译控制器还持有有界的原文和译文；恢复栏不足仍须返回 Unknown。
                let _ = self.retain(&format!(
                    "拼音：{}；原文：{}；待核对提交：{}",
                    actual.frame.preedit, source, commit.text
                ));
            } else {
                self.retain(&format!(
                    "拼音：{}；待核对提交：{}",
                    actual.frame.preedit, commit.text
                ))?;
            }
        }
        Ok((receipt, selection))
    }

    fn translation_commit(
        &self,
        capture: &Captured,
        text: &str,
        retry: Option<TranslationReceipt>,
        sink: &dyn Fn(&str) -> CommitOutcome,
    ) -> Result<TranslationReceipt> {
        self.host.check()?;
        if self.host.quarantined.get()
            || self.host.instance != capture.context.session
            || self.host.epoch.get() != capture.context.epoch
        {
            return Err(Error::new(E_FAIL, "翻译请求不属于当前输入会话"));
        }
        let (actual, projected) = {
            let mut actual = self.host.engine.borrow_mut();
            let mut projected = self.host.projection.borrow_mut();
            let frame = actual.frame();
            let mut projected_frame = projected.frame();
            let projected_revision = projected_frame.revision;
            projected_frame.revision = frame.revision;
            let candidate = &capture.source.candidate_id;
            if frame != projected_frame
                || actual.translation_source_identity(candidate.clone()) != capture.source
                || projected.translation_source_identity(candidate.clone()) != capture.source
                || candidate.as_ref().is_some_and(|id| {
                    !frame
                        .candidates
                        .iter()
                        .any(|candidate| candidate.id == *id && candidate.text == capture.original)
                })
            {
                return Err(Error::new(
                    E_FAIL,
                    "两个输入引擎的当前原文身份不一致，未写文档",
                ));
            }
            let rejected = |_: cloudleaf_types::CoreError| {
                Error::new(E_INVALIDARG, "译文不属于当前可提交帧或当前事务不能重试")
            };
            if let Some(prior) = retry {
                if prior.outcome != CommitOutcome::NotApplied
                    || [&*actual, &*projected].iter().any(|engine| {
                        engine
                            .failed_commit()
                            .is_none_or(|failed| failed.text != text)
                    })
                {
                    return Err(Error::new(E_INVALIDARG, "上一次提交没有可重试的译文"));
                }
                actual
                    .validate_translation_retry(prior.actual)
                    .map_err(rejected)?;
                projected
                    .validate_translation_retry(prior.projected)
                    .map_err(rejected)?;
                (
                    actual.handle(InputEvent::RetryCommit),
                    projected.handle(InputEvent::RetryCommit),
                )
            } else {
                actual
                    .validate_translation(capture.context.input_revision, text)
                    .map_err(rejected)?;
                projected
                    .validate_translation(projected_revision, text)
                    .map_err(rejected)?;
                (
                    actual.confirm_translation(capture.context.input_revision, text),
                    projected.confirm_translation(projected_revision, text),
                )
            }
        };
        if !same_semantics(&actual, &projected) || actual.commit.is_none() {
            self.host.quarantined.set(true);
            return Err(Error::new(E_FAIL, "双引擎翻译提交不同，已隔离且未写文档"));
        }
        self.finish_commit(&actual, &projected, Some(&capture.original), sink)
            .map(|(receipt, _)| receipt)
    }

    fn discard_translation(&self, previous: TranslationReceipt) -> Result<()> {
        self.host.check()?;
        if previous.outcome != CommitOutcome::NotApplied || self.host.quarantined.get() {
            return Err(Error::new(E_FAIL, "不能取消未知结果，请先核对宿主文档"));
        }
        let mut actual = self.host.engine.borrow_mut();
        let mut projected = self.host.projection.borrow_mut();
        // Esc 可以先在两套核心都结束已知失败；单侧失效或未确认提交仍须拒绝。
        if [&*actual, &*projected].into_iter().all(|engine| {
            engine.failed_commit().is_none()
                && engine.frame().state != cloudleaf_types::InputState::Committing
        }) {
            return Ok(());
        }
        let rejected = |_: cloudleaf_types::CoreError| Error::new(E_FAIL, "当前译文已不是失败提交");
        actual
            .validate_failed_translation(previous.actual)
            .map_err(rejected)?;
        projected
            .validate_failed_translation(previous.projected)
            .map_err(rejected)?;
        actual
            .discard_failed_translation(previous.actual)
            .map_err(rejected)?;
        projected
            .discard_failed_translation(previous.projected)
            .map_err(rejected)
    }

    pub fn process(
        &self,
        event: InputEvent,
        selection: Option<(u64, String)>,
        sink: &dyn Fn(&str) -> CommitOutcome,
    ) -> Result<(InputFrame, Sample)> {
        self.host.check()?;
        if self.host.quarantined.get() {
            return Err(Error::new(E_FAIL, "提交结果未知；请核对文档后开始新会话"));
        }
        if event == InputEvent::FocusLost {
            let raw = self.host.engine.borrow().frame().preedit;
            if !raw.is_empty() {
                self.retain(&raw)?;
            }
        }
        let mut sample = Sample::default();
        let now = Instant::now();
        let actual = if let Some((revision, id)) = &selection {
            self.host
                .engine
                .borrow_mut()
                .select_candidate(*revision, id)
        } else {
            self.host.engine.borrow_mut().handle(event)
        };
        sample.core_ns = now.elapsed().as_nanos();
        if actual.error.is_some() {
            return Err(Error::new(
                E_INVALIDARG,
                "核心拒绝旧候选或无效事件，原文保留",
            ));
        }
        let now = Instant::now();
        let projected = if let Some((_, id)) = &selection {
            let revision = self.host.projection.borrow().frame().revision;
            self.host
                .projection
                .borrow_mut()
                .select_candidate(revision, id)
        } else {
            self.host.projection.borrow_mut().handle(event)
        };
        sample.projection_ns = now.elapsed().as_nanos();
        let now = Instant::now();
        if !same_semantics(&actual, &projected) {
            self.host.quarantined.set(true);
            return Err(Error::new(E_FAIL, "双引擎结果不同，已隔离且未写文档"));
        }
        sample.comparison_ns = now.elapsed().as_nanos();
        let now = Instant::now();
        if actual.commit.is_some() {
            let (_, selection) = self.finish_commit(&actual, &projected, None, sink)?;
            sample.selection = selection;
        } else if !actual.handled {
            let mut document = self.document.borrow().clone();
            let changed = match event {
                InputEvent::Key(Key::Char(c)) => {
                    document.push(c);
                    true
                }
                InputEvent::Key(Key::Space) => {
                    document.push(' ');
                    true
                }
                InputEvent::Key(Key::Enter) => {
                    document.push_str("\r\n");
                    true
                }
                InputEvent::Key(Key::Backspace) => document.pop().is_some(),
                _ => false,
            };
            if changed {
                if document.len() <= 16384 && sink(&document) == CommitOutcome::Applied {
                    *self.document.borrow_mut() = document;
                } else {
                    self.host.quarantined.set(true);
                    self.retain(&format!("原生事件待核对：{event:?}"))?;
                }
            }
        }
        sample.transaction_ns = now.elapsed().as_nanos();
        let now = Instant::now();
        let frame = self.host.engine.borrow().frame();
        sample.frame_ns = now.elapsed().as_nanos();
        Ok((frame, sample))
    }
}

#[derive(Default, Clone, Copy)]
struct TranslationControls {
    status: HWND,
    english: HWND,
    source: HWND,
    raw_result: HWND,
    acknowledge: HWND,
    confirm: HWND,
}

struct Window {
    local: LocalHost,
    window: Cell<HWND>,
    document: Cell<HWND>,
    status: Cell<HWND>,
    fault: Cell<bool>,
    ui: RefCell<Option<CandidateUi>>,
    users: RefCell<Option<UserState>>,
    dictionary_window: Cell<HWND>,
    translation: RefCell<TranslationController>,
    translation_controls: Cell<TranslationControls>,
}
struct UserState {
    panel: Rc<user_panel::Panel>,
    worker: user_worker::Worker,
    active: Option<user_worker::Prepared>,
    pending: Option<user_worker::Prepared>,
    busy: bool,
    session: String,
}
impl Drop for UserState {
    fn drop(&mut self) {
        self.panel.close();
        self.worker.shutdown();
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}
fn text(window: HWND) -> String {
    // 安全：只读本入口创建且有界的EDIT，未向其他应用查询文字。
    unsafe {
        let mut value = vec![0u16; GetWindowTextLengthW(window) as usize + 1];
        let n = GetWindowTextW(window, &mut value);
        String::from_utf16_lossy(&value[..n as usize])
    }
}
impl Window {
    fn new(local: LocalHost, translation: TranslationController) -> Self {
        Self {
            local,
            window: Cell::new(HWND::default()),
            document: Cell::new(HWND::default()),
            status: Cell::new(HWND::default()),
            fault: Cell::new(false),
            ui: RefCell::new(None),
            users: RefCell::new(None),
            dictionary_window: Cell::new(HWND::default()),
            translation: RefCell::new(translation),
            translation_controls: Cell::default(),
        }
    }

    fn create_translation_controls(&self) -> Result<()> {
        let owner = self.window.get();
        let style = WS_CHILD | WS_VISIBLE;
        let readonly =
            WINDOW_STYLE(ES_READONLY as u32 | ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32);
        // 安全：所有子控件同属本窗口线程；ID 只用于核对这些子控件的通知。
        unsafe {
            let create = |class, caption: &str, styles, x, y, width, height, id: u16| {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class,
                    PCWSTR(wide(caption).as_ptr()),
                    styles,
                    x,
                    y,
                    width,
                    height,
                    Some(owner),
                    (id != 0).then_some(HMENU(usize::from(id) as *mut _)),
                    None,
                    None,
                )
            };
            let button = |caption, x, y, width, id| {
                create(
                    w!("BUTTON"),
                    caption,
                    style | WS_TABSTOP,
                    x,
                    y,
                    width,
                    28,
                    id,
                )
            };
            let label = |caption, y| create(w!("STATIC"), caption, style, 740, y, 366, 25, 0);
            button("翻译高亮候选 → 英文", 740, 12, 184, 21)?;
            button("翻译缓冲区 → 中文", 930, 12, 176, 22)?;
            let status = create(w!("STATIC"), "", style, 740, 46, 366, 78, 0)?;
            label("英文缓冲区", 127)?;
            let english = create(
                w!("EDIT"),
                "",
                style
                    | WS_TABSTOP
                    | WS_BORDER
                    | WS_VSCROLL
                    | WINDOW_STYLE(
                        ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_WANTRETURN as u32,
                    ),
                740,
                151,
                366,
                81,
                translation::ENGLISH_EDIT,
            )?;
            SendMessageW(
                english,
                EM_SETLIMITTEXT,
                Some(WPARAM(MAX_INPUT_BYTES)),
                None,
            );
            button("提交英文原文", 740, 238, 176, 27)?;
            button("清空英文缓冲区", 930, 238, 176, 28)?;
            button("返回中文输入", 740, 552, 176, translation::COMMAND_LAST)?;
            label("本次翻译原文", 272)?;
            let source = create(
                w!("EDIT"),
                "",
                style | readonly | WS_BORDER | WS_VSCROLL,
                740,
                295,
                366,
                56,
                0,
            )?;
            label("模型原始译文", 358)?;
            let raw_result = create(
                w!("EDIT"),
                "",
                style | readonly | WS_BORDER | WS_VSCROLL,
                740,
                381,
                366,
                98,
                0,
            )?;
            button("取消翻译", 740, 485, 176, 23)?;
            button("保留原文", 930, 485, 176, 26)?;
            let acknowledge = button("已核对本次风险", 740, 519, 176, 24)?;
            let confirm = button("确认译文上屏", 930, 519, 176, 25)?;
            self.translation_controls.set(TranslationControls {
                status,
                english,
                source,
                raw_result,
                acknowledge,
                confirm,
            });
        }
        self.refresh_translation()
    }

    fn english(&self) -> String {
        let control = self.translation_controls.get().english;
        if control.is_invalid() {
            String::new()
        } else {
            text(control)
        }
    }

    fn refresh_translation(&self) -> Result<()> {
        let controls = self.translation_controls.get();
        if controls.status.is_invalid() {
            return Ok(());
        }
        let view = self.translation.borrow().view();
        // 安全：释放控制器借用后才调用可能重新分派窗口通知的 Win32 API。
        unsafe {
            for (control, value) in [
                (controls.status, view.status.as_str()),
                (controls.source, view.source.as_str()),
                (controls.raw_result, view.output.as_str()),
                (
                    controls.confirm,
                    if view.retry {
                        "明确重试提交"
                    } else {
                        "确认译文上屏"
                    },
                ),
            ] {
                SetWindowTextW(control, PCWSTR(wide(value).as_ptr()))?;
            }
            let _ = EnableWindow(
                controls.confirm,
                view.confirm && !self.local.host.quarantined.get(),
            );
            let _ = EnableWindow(
                controls.acknowledge,
                view.acknowledge && !self.local.host.quarantined.get(),
            );
        }
        Ok(())
    }

    fn command_from_own_control(
        &self,
        parameters: WPARAM,
        origin: LPARAM,
    ) -> Option<translation::Command> {
        let id = parameters.0 as u16;
        if !(translation::COMMAND_FIRST..=translation::COMMAND_LAST).contains(&id) || origin.0 == 0
        {
            return None;
        }
        // 安全：本窗口只接受已经创建且身份相符的按钮或编辑框通知。
        let own_control = unsafe { GetDlgItem(Some(self.window.get()), i32::from(id)) }
            .is_ok_and(|control| control == HWND(origin.0 as *mut _));
        translation::command(parameters.0, own_control)
    }

    fn handle_translation(self: &Rc<Self>, command: translation::Command) -> Result<()> {
        use translation::Command::*;
        let english = self.english();
        let focus;
        let result = match command {
            StartChinese | StartEnglish => {
                let direction = if command == StartChinese {
                    Direction::ZhEn
                } else {
                    Direction::EnZh
                };
                focus = Some(direction);
                self.translation
                    .borrow_mut()
                    .start(&self.local, direction, &english)
            }
            Cancel | KeepOriginal => {
                let reason = if command == Cancel {
                    "已取消翻译，原文保留"
                } else {
                    "已保留原文"
                };
                focus = self
                    .translation
                    .borrow()
                    .direction()
                    .or(Some(Direction::ZhEn));
                self.translation.borrow_mut().cancel(&self.local, reason)
            }
            Acknowledge => {
                focus = self.translation.borrow().direction();
                self.translation.borrow_mut().acknowledge();
                Ok(())
            }
            Confirm => {
                focus = self.translation.borrow().direction();
                self.apply_translation(&english)
            }
            SubmitEnglish => {
                focus = Some(Direction::EnZh);
                self.apply_english_original(&english)
            }
            ClearEnglish => {
                focus = Some(Direction::EnZh);
                self.clear_english()
            }
            FocusChinese => {
                focus = Some(Direction::ZhEn);
                Ok(())
            }
            EnglishChanged => {
                let revision = self.local.host.engine.borrow().revision();
                let changed = self.translation.borrow_mut().invalidate_changed(
                    &self.local,
                    Some(&english),
                    revision,
                )?;
                if changed {
                    return self.refresh_translation();
                }
                return Ok(());
            }
        };
        if let Err(error) = result {
            self.translation.borrow_mut().report_error(&error);
        }
        if matches!(
            command,
            StartChinese | StartEnglish | Cancel | KeepOriginal | ClearEnglish
        ) {
            let frame = self.local.host.engine.borrow().frame();
            self.update(frame)?;
        }
        self.refresh_translation()?;
        if let Some(direction) = focus {
            let handle = if direction == Direction::ZhEn {
                self.window.get()
            } else {
                self.translation_controls.get().english
            };
            // 安全：只恢复本宿主内目标控件的键盘焦点。
            unsafe {
                let _ = SetFocus(Some(handle));
            }
        }
        Ok(())
    }

    fn apply_translation(self: &Rc<Self>, english: &str) -> Result<()> {
        let confirmation = self.translation.borrow().prepare(&self.local, english)?;
        let receipt = self.local.translation_commit(
            &confirmation.capture,
            &confirmation.text,
            confirmation.retry,
            &|document| self.sink(document),
        )?;
        self.translation
            .borrow_mut()
            .acknowledge_commit(&self.local, &confirmation, receipt);
        if receipt.outcome == CommitOutcome::Applied
            && confirmation.capture.direction == Direction::EnZh
        {
            self.write_english("")?;
        }
        let frame = self.local.host.engine.borrow().frame();
        self.update(frame)
    }

    fn apply_english_original(self: &Rc<Self>, english: &str) -> Result<()> {
        self.translation
            .borrow_mut()
            .cancel(&self.local, "放弃翻译，处理英文原文")?;
        let capture = Captured::new(&self.local, Direction::EnZh, english)?;
        let receipt = self
            .local
            .translation_commit(&capture, english, None, &|document| self.sink(document))?;
        let message = match receipt.outcome {
            CommitOutcome::Applied => {
                self.write_english("")?;
                "英文原文已提交"
            }
            CommitOutcome::NotApplied => {
                self.local.discard_translation(receipt)?;
                "英文原文未提交，已保留缓冲区"
            }
            CommitOutcome::Unknown => "英文原文提交结果未知。请核对文档后开始新会话。",
        };
        self.translation.borrow_mut().set_message(message);
        let frame = self.local.host.engine.borrow().frame();
        self.update(frame)
    }

    fn clear_english(&self) -> Result<()> {
        if self.local.host.quarantined.get() {
            return Err(Error::new(E_FAIL, "请先核对未知提交再清空原文"));
        }
        if self.translation.borrow().direction() == Some(Direction::EnZh) {
            self.translation
                .borrow_mut()
                .cancel(&self.local, "英文缓冲区已清空")?;
        }
        self.write_english("")
    }

    fn write_english(&self, value: &str) -> Result<()> {
        // 安全：只操作该进程拥有的英文编辑控件。
        unsafe {
            SetWindowTextW(
                self.translation_controls.get().english,
                PCWSTR(wide(value).as_ptr()),
            )
        }
    }

    fn poll_translation(&self) -> Result<()> {
        let english = if self.translation.borrow().direction() == Some(Direction::EnZh) {
            self.english()
        } else {
            String::new()
        };
        let changed = self.translation.borrow_mut().poll(&self.local, &english);
        if changed {
            self.refresh_translation()?;
        }
        Ok(())
    }

    fn deactivate(self: &Rc<Self>, target: HWND) -> Result<()> {
        let translation = self.translation.borrow_mut().set_focus(&self.local, false);
        // 登记的词库面板与本宿主共享原文，但不共享翻译确认权限。
        let opens_dictionary = target == self.dictionary_window.get() && !target.is_invalid();
        if !opens_dictionary && !self.local.host.quarantined.get() {
            self.process(InputEvent::FocusLost, None)?;
        }
        translation?;
        self.refresh_translation()
    }

    fn sink(&self, document: &str) -> CommitOutcome {
        let value = wide(document);
        // 安全：写入本线程独占EDIT；写后按实际UTF-16内容确认，故障注入发生在写后。
        let written = unsafe { SetWindowTextW(self.document.get(), PCWSTR(value.as_ptr())) };
        if written.is_err() || text(self.document.get()) != document || self.fault.replace(false) {
            CommitOutcome::Unknown
        } else {
            CommitOutcome::Applied
        }
    }
    fn process(self: &Rc<Self>, event: InputEvent, selection: Option<(u64, String)>) -> Result<()> {
        match self
            .local
            .process(event, selection, &|document| self.sink(document))
        {
            Ok((frame, sample)) => {
                if let Some(evidence) = &sample.selection
                    && let Some(users) = &mut *self.users.borrow_mut()
                    && let Some(active) = &users.active
                    && active.learning
                {
                    if users.busy || evidence.snapshot_version != active.snapshot.version() {
                        users.panel.refresh(
                            &active.listing,
                            "本次提交已完成；后台任务未完成或版本已过期，此次学习未记录",
                        );
                    } else {
                        let event =
                            user_worker::learning_event(evidence, &users.session, active.epoch);
                        match users.worker.send(user_worker::Work::Learn {
                            event,
                            generation: active.generation,
                        }) {
                            Ok(()) => users.busy = true,
                            Err(e) => {
                                users.panel.refresh(&active.listing, e);
                            }
                        }
                    }
                }
                let now = Instant::now();
                self.update(frame)?;
                println!(
                    "HOST_SAMPLE core_ns={} projection_ns={} comparison_ns={} transaction_ns={} frame_ns={} candidate_window_ns={} TSF=false",
                    sample.core_ns,
                    sample.projection_ns,
                    sample.comparison_ns,
                    sample.transaction_ns,
                    sample.frame_ns,
                    now.elapsed().as_nanos()
                );
                Ok(())
            }
            Err(error) => {
                let frame = self.local.host.engine.borrow().frame();
                self.update(frame)?;
                Err(error)
            }
        }
    }
    fn open_users(self: &Rc<Self>) -> Result<()> {
        self.translation
            .borrow_mut()
            .cancel(&self.local, "词库面板已打开，旧译文不可提交")?;
        self.refresh_translation()?;
        let current = self
            .users
            .borrow()
            .as_ref()
            .map(|users| users.panel.clone());
        if let Some(panel) = current {
            panel.show();
            return Ok(());
        }
        let (base, language) = self
            .local
            .host
            .engine
            .borrow()
            .search_resources()
            .ok_or_else(|| Error::new(E_FAIL, "没有可管理的搜索快照"))?;
        let weak = Rc::downgrade(self);
        let focus_view = weak.clone();
        let path = user_directory()?;
        let panel = user_panel::Panel::open(
            self.window.get(),
            Rc::new(move |command| {
                if let Some(view) = weak.upgrade() {
                    view.user_command(command);
                }
            }),
            Rc::new(move |target| {
                if let Some(view) = focus_view.upgrade()
                    && target != view.window.get()
                {
                    let _ = view.deactivate(target);
                }
            }),
        )?;
        self.dictionary_window.set(panel.handle());
        let worker = user_worker::Worker::start(base, language, path);
        *self.users.borrow_mut() = Some(UserState {
            panel,
            worker,
            active: None,
            pending: None,
            busy: true,
            session: format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
        });
        let panel = self.users.borrow().as_ref().unwrap().panel.clone();
        panel.show();
        unsafe {
            SetTimer(Some(self.window.get()), 19, 100, None);
        }
        Ok(())
    }
    fn user_command(self: &Rc<Self>, command: user_panel::Command) {
        let mut users = self.users.borrow_mut();
        let Some(users) = users.as_mut() else { return };
        if users.busy || users.pending.is_some() {
            users.panel.refresh("", "任务仍在进行，稍后重试");
            return;
        }
        let engine = self.local.host.engine.borrow();
        let frame = engine.frame();
        if self.local.host.quarantined.get()
            || engine.failed_commit().is_some()
            || frame.state == cloudleaf_types::InputState::Committing
        {
            users
                .panel
                .refresh("", "未决提交阻止用户词库变更，请先核对或取消");
            return;
        }
        if matches!(
            command,
            user_panel::Command::ToggleLearning
                | user_panel::Command::ClearLearning
                | user_panel::Command::ClearAll
        ) && let Some(active) = &mut users.active
        {
            active.learning = false;
        }
        match users.worker.send(user_worker::Work::Manage(command)) {
            Ok(()) => {
                users.busy = true;
                users
                    .panel
                    .refresh("", "正在后台保存和构建，输入不等待磁盘");
            }
            Err(e) => {
                users.panel.refresh("", e);
            }
        }
    }
    fn poll_users(self: &Rc<Self>) -> Result<()> {
        let mut state = self.users.borrow_mut();
        let Some(users) = state.as_mut() else {
            return Ok(());
        };
        if let Some(reply) = users.worker.poll() {
            users.busy = false;
            match reply {
                user_worker::Reply::Error(e) => {
                    let listing = users.active.as_ref().map_or("", |a| a.listing.as_str());
                    users.panel.refresh(listing, &e);
                }
                user_worker::Reply::Ready(p) => users.pending = Some(p),
            }
        }
        let Some(p) = users.pending.as_ref() else {
            return Ok(());
        };
        if self.local.host.quarantined.get() {
            users
                .panel
                .refresh(&p.listing, "已保存，提交结果未知；核对新会话后才发布快照");
            return Ok(());
        }
        for engine in [&self.local.host.engine, &self.local.host.projection] {
            let current = engine.borrow().search_capacity_bounds().unwrap();
            let next = engine
                .borrow()
                .replacement_capacity_bounds(p.snapshot.clone(), p.language.clone())
                .map_err(|_| Error::new(E_FAIL, "用户快照容量或模型无效"))?;
            if next.max_frame_bytes > current.max_frame_bytes
                || next.max_diagnostic_bytes > current.max_diagnostic_bytes
            {
                users.panel.refresh(
                    &p.listing,
                    "已保存但帧/恢复材料超过原Host容量，未发布；请重启构建新Host",
                );
                return Ok(());
            }
            if engine
                .borrow()
                .validate_search_resources(p.snapshot.clone(), p.language.clone())
                .is_err()
            {
                users
                    .panel
                    .refresh(&p.listing, "已保存，等待未决或失败事务收尾后发布");
                return Ok(());
            }
        }
        // 两个实际引擎已共同预检，当前STA中间不分派其他消息。
        for engine in [&self.local.host.engine, &self.local.host.projection] {
            engine
                .borrow_mut()
                .replace_search_resources(p.snapshot.clone(), p.language.clone())
                .map_err(|_| Error::new(E_FAIL, "用户快照发布失败"))?;
        }
        let p = users.pending.take().unwrap();
        users.panel.refresh(&p.listing, &p.status);
        users.active = Some(p);
        drop(state);
        let frame = self.local.host.engine.borrow().frame();
        self.update(frame)
    }
    fn settings_key(self: &Rc<Self>, key: u16) -> Result<()> {
        use cloudleaf_input::{
            CorrectionConfig, DoublePinyinLayout, FuzzyDirection, FuzzyPair, FuzzyRule,
            PinyinScheme,
        };
        let mut config = self
            .local
            .host
            .engine
            .borrow()
            .pinyin_scheme()
            .cloned()
            .ok_or_else(|| Error::new(E_FAIL, "此后端不支持方案设置"))?;
        if key == VK_F6.0 {
            config.scheme = match config.scheme {
                PinyinScheme::FullPinyin => PinyinScheme::AbbreviatedPinyin,
                PinyinScheme::AbbreviatedPinyin => {
                    PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe)
                }
                PinyinScheme::DoublePinyin(_) => PinyinScheme::FullPinyin,
            };
            if matches!(config.scheme, PinyinScheme::DoublePinyin(_)) {
                config.correction = None;
            }
        } else if key == VK_F7.0 {
            if config.fuzzy.iter().any(|rule| rule.pair == FuzzyPair::NL) {
                config.fuzzy.retain(|rule| rule.pair != FuzzyPair::NL);
            } else {
                config.fuzzy.push(FuzzyRule {
                    pair: FuzzyPair::NL,
                    direction: FuzzyDirection::Both,
                });
            }
        } else if key == VK_F8.0 {
            if matches!(config.scheme, PinyinScheme::DoublePinyin(_)) {
                return Err(Error::new(E_FAIL, "小鹤双拼尚不支持拼写纠错，设置未改变"));
            }
            config.correction = if config.correction.is_some() {
                None
            } else {
                Some(CorrectionConfig::default())
            };
        }
        let frame = self.local.switch_scheme(config)?;
        self.update(frame)
    }

    fn update(self: &Rc<Self>, frame: InputFrame) -> Result<()> {
        let translation_changed =
            self.translation
                .borrow_mut()
                .invalidate_changed(&self.local, None, frame.revision)?;
        if translation_changed {
            self.refresh_translation()?;
        }
        let config = self
            .local
            .host
            .engine
            .borrow()
            .pinyin_scheme()
            .cloned()
            .unwrap_or_default();
        let name = match config.scheme {
            cloudleaf_input::PinyinScheme::FullPinyin => "全拼",
            cloudleaf_input::PinyinScheme::AbbreviatedPinyin => "全拼/简拼混合",
            cloudleaf_input::PinyinScheme::DoublePinyin(_) => "小鹤双拼",
        };
        let message = format!(
            "方案：{name}  n/l模糊：{}  纠错：{}\r\n拼音：{}  修订：{}  {:?}\r\n{}{}",
            if config.fuzzy.is_empty() {
                "关"
            } else {
                "开"
            },
            if config.correction.is_none() {
                "关"
            } else {
                "开"
            },
            frame.preedit,
            frame.revision,
            frame.state,
            if self.local.host.quarantined.get() {
                "结果未知，停止继续输入。"
            } else {
                ""
            },
            self.local.retained.borrow()
        );
        // 安全：所有HWND均由此窗口在同一线程创建。
        unsafe {
            SetWindowTextW(self.status.get(), PCWSTR(wide(&message).as_ptr()))?;
        }
        if frame.preedit.is_empty()
            || frame.candidates.is_empty()
            || self.local.host.quarantined.get()
        {
            self.ui.borrow_mut().take();
            return Ok(());
        }
        if self.ui.borrow().is_none() {
            let weak = Rc::downgrade(self);
            let select = Rc::new(move |revision, id| {
                let view = weak.upgrade().ok_or_else(|| Error::from_hresult(E_FAIL))?;
                view.process(InputEvent::Key(Key::Space), Some((revision, id)))
            });
            *self.ui.borrow_mut() = Some(CandidateUi::isolated(
                self.local.host.clone(),
                frame.clone(),
                select,
            )?);
        }
        let mut rect = RECT::default();
        unsafe {
            GetWindowRect(self.window.get(), &mut rect)?;
        }
        rect.left += 12;
        rect.bottom = rect.top + 170;
        self.ui.borrow_mut().as_mut().unwrap().update(frame, rect)
    }
}
fn user_directory() -> Result<std::path::PathBuf> {
    use windows::Win32::UI::Shell::{
        FOLDERID_LocalAppData, KNOWN_FOLDER_FLAG, SHGetKnownFolderPath,
    };
    // 安全：当前用户标准目录的COM分配字符串转换后立即归还。
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_LocalAppData, KNOWN_FOLDER_FLAG(0), None)?;
        let value = path.to_string();
        windows::Win32::System::Com::CoTaskMemFree(Some(path.0.cast()));
        let mut root = std::path::PathBuf::from(value?);
        root.push("Cloudleaf");
        if let Ok(profile) = std::env::var("CLOUDLEAF_USER_TEST_PROFILE") {
            if profile.is_empty()
                || profile.len() > 64
                || !profile
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                return Err(Error::new(E_INVALIDARG, "测试档案标识无效"));
            }
            root.push("Research");
            root.push(profile);
        }
        root.push("user-dictionary");
        Ok(root)
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    boundary(|| {
        // 安全：userdata借用由run持有的Rc；临时强引用覆盖候选选择重入和销毁。
        unsafe {
            if message == WM_NCCREATE { let create = &*(lparam.0 as *const CREATESTRUCTW); SetWindowLongPtrW(window,GWLP_USERDATA,create.lpCreateParams as isize); }
            let ptr = GetWindowLongPtrW(window,GWLP_USERDATA) as *const Window;
            if ptr.is_null() { return Ok(DefWindowProcW(window,message,wparam,lparam)); }
            Rc::increment_strong_count(ptr); let view = Rc::from_raw(ptr);
            match message {
                WM_CREATE => {
                    view.window.set(window);
                    CreateWindowExW(WINDOW_EX_STYLE::default(),w!("STATIC"),w!("F6方案 / F7 n↔l / F8纠错；Space/数字选词，方向键/翻页，Enter确认/原文，Esc取消。"),WS_CHILD|WS_VISIBLE,12,10,720,30,Some(window),None,None,None)?;
                    view.status.set(CreateWindowExW(WINDOW_EX_STYLE::default(),w!("STATIC"),w!(""),WS_CHILD|WS_VISIBLE,12,44,720,76,Some(window),None,None,None)?);
                    for (id,label,x,width) in [(1,"下一次提交注入Unknown",12,230),(2,"核对后开始新会话",260,210),(3,"纠错开关",485,200)] {
                        CreateWindowExW(WINDOW_EX_STYLE::default(),w!("BUTTON"),PCWSTR(wide(label).as_ptr()),WS_CHILD|WS_VISIBLE,x,128,width,30,Some(window),Some(HMENU(id as *mut _)),None,None)?;
                    }
                    CreateWindowExW(WINDOW_EX_STYLE::default(),w!("BUTTON"),w!("用户词库 / 本地学习"),WS_CHILD|WS_VISIBLE|WS_TABSTOP,485,164,220,30,Some(window),Some(HMENU(4usize as *mut _)),None,None)?;
                    view.document.set(CreateWindowExW(WS_EX_CLIENTEDGE,w!("EDIT"),w!(""),WS_CHILD|WS_VISIBLE|WS_VSCROLL|WINDOW_STYLE(ES_READONLY as u32|ES_MULTILINE as u32|ES_AUTOVSCROLL as u32),12,320,710,230,Some(window),None,None,None)?);
                    view.create_translation_controls()?;
                    SetTimer(Some(window), 22, 80, None);
                    // 只解绑本窗口及已创建子控件的继承IME；拼音由本隔离核心处理，不改变系统布局。
                    if !ImmAssociateContextEx(window, HIMC::default(), 0).as_bool()
                        || !ImmAssociateContextEx(window, HIMC::default(), IACE_CHILDREN).as_bool() {
                        return Ok(LRESULT(-1));
                    }
                    Ok(LRESULT(0))
                }
                WM_CHAR => {
                    let c = char::from_u32(wparam.0 as u32);
                    if let Some(c) = c.filter(|c| !c.is_control()) { let key = if c==' ' {Key::Space} else {Key::Char(c)}; view.process(InputEvent::Key(key),None)?; }
                    Ok(LRESULT(0))
                }
                WM_KEYDOWN => {
                    let virtual_key = wparam.0 as u16;
                    if matches!(virtual_key, x if x == VK_F6.0 || x == VK_F7.0 || x == VK_F8.0)
                        && GetKeyState(i32::from(VK_CONTROL.0)) >= 0
                        && GetKeyState(i32::from(VK_MENU.0)) >= 0
                        && GetKeyState(i32::from(VK_SHIFT.0)) >= 0 {
                        if let Err(error) = view.settings_key(virtual_key) {
                            let message = wide(&format!("设置未改变：{}", error.message()));
                            SetWindowTextW(view.status.get(), PCWSTR(message.as_ptr()))?;
                        }
                        return Ok(LRESULT(0));
                    }
                    let key = match virtual_key {
                        x if x==VK_BACK.0 => Some(Key::Backspace), x if x==VK_RETURN.0 => Some(Key::Enter), x if x==VK_ESCAPE.0 => Some(Key::Escape),
                        x if x==VK_LEFT.0 => Some(Key::Left),x if x==VK_RIGHT.0 => Some(Key::Right),x if x==VK_UP.0 => Some(Key::Up),x if x==VK_DOWN.0 => Some(Key::Down),x if x==VK_PRIOR.0 => Some(Key::PageUp),x if x==VK_NEXT.0 => Some(Key::PageDown),_=>None};
                    if let Some(key)=key { view.process(InputEvent::Key(key),None)?; }
                    Ok(LRESULT(0))
                }
                WM_COMMAND if wparam.0&0xffff==1 => { view.fault.set(true); let _=SetFocus(Some(window)); Ok(LRESULT(0)) }
                WM_COMMAND if wparam.0&0xffff==2 => {
                    if let Some(users)=view.users.borrow().as_ref(){users.worker.cancel_learning();}
                    view.ui.borrow_mut().take();
                    // 明确新会话接受当前本窗口文档，旧Unknown请求只失效，不重发。
                    *view.local.document.borrow_mut()=text(view.document.get());
                    view.local.reset()?;
                    view.translation.borrow_mut().reset();
                    view.refresh_translation()?;
                    let frame=view.local.host.engine.borrow().frame(); view.update(frame)?; let _=SetFocus(Some(window)); Ok(LRESULT(0))
                }
                WM_COMMAND if wparam.0&0xffff==3 => {
                    // 系统热键可能拦截F8；按钮仅切换本窗口配置，不改全局快捷键。
                    if let Err(error)=view.settings_key(VK_F8.0) {
                        SetWindowTextW(view.status.get(),PCWSTR(wide(&format!("设置未改变：{}",error.message())).as_ptr()))?;
                    }
                    let _=SetFocus(Some(window)); Ok(LRESULT(0))
                }
                WM_COMMAND if wparam.0&0xffff==4 => {view.open_users()?;Ok(LRESULT(0))}
                WM_COMMAND => {
                    if let Some(command) = view.command_from_own_control(wparam, lparam) {
                        view.handle_translation(command)?;
                        Ok(LRESULT(0))
                    } else {
                        Ok(DefWindowProcW(window,message,wparam,lparam))
                    }
                }
                WM_TIMER if wparam.0==19 => {view.poll_users()?;Ok(LRESULT(0))}
                WM_TIMER if wparam.0==22 => {view.poll_translation()?;Ok(LRESULT(0))}
                // 内部子控件不改变顶层激活；唯一的跨窗口例外是登记的词库面板。
                WM_ACTIVATE if wparam.0 & 0xffff == WA_INACTIVE as usize => {
                    view.deactivate(HWND(lparam.0 as *mut _))?; Ok(LRESULT(0))
                }
                WM_ACTIVATE | WM_SETFOCUS => {
                    view.translation.borrow_mut().set_focus(&view.local, true)?;
                    if !view.local.host.quarantined.get() { view.process(InputEvent::Activate,None)?; }
                    Ok(LRESULT(0))
                }
                WM_CLOSE => {
                    view.translation.borrow_mut().close();
                    view.ui.borrow_mut().take();
                    let users = view.users.borrow_mut().take();
                    if let Some(mut users) = users { users.panel.close(); users.worker.shutdown(); }
                    DestroyWindow(window)?; Ok(LRESULT(0))
                }
                WM_DESTROY => { PostQuitMessage(0); Ok(LRESULT(0)) }
                WM_NCDESTROY => { SetWindowLongPtrW(window,GWLP_USERDATA,0); Ok(DefWindowProcW(window,message,wparam,lparam)) }
                _ => Ok(DefWindowProcW(window,message,wparam,lparam)),
            }
        }
    }).unwrap_or(LRESULT(0))
}

pub(crate) fn run(args: &[String]) -> Result<()> {
    let options = translation::LaunchOptions::parse(args)?;
    let (host, _, _) =
        super::input_review::load_host_k(options.runtime, options.runtime_sha, options.model, 10)?;
    let view = Rc::new(Window::new(LocalHost::new(host)?, options.controller()?));
    // 安全：创建、运行和销毁本进程普通Win32窗口；没有任何注册或TSF激活动作。
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        let class = w!("CloudleafIsolatedInputHost");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class,
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(Error::from_thread());
        }
        let window = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("云笺 · 隔离输入与翻译"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1140,
            628,
            None,
            None,
            Some(instance),
            Some(Rc::as_ptr(&view).cast()),
        )?;
        let _ = SetFocus(Some(window));
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        view.ui.borrow_mut().take();
        let _ = UnregisterClassW(class, Some(instance));
    }
    view.translation.borrow_mut().close();
    if !view.translation.borrow_mut().wait() {
        return Err(Error::new(
            E_FAIL,
            "未能在规定时间内核实专用翻译工作进程退出",
        ));
    }
    Ok(())
}

pub(crate) fn benchmark(
    runtime: &str,
    runtime_sha: &str,
    model: &str,
    model_sha: &str,
    events: &str,
) -> Result<()> {
    let (host, _, _) =
        super::input_review::load_host(runtime, runtime_sha, Some((model, model_sha)))?;
    let local = LocalHost::new(host)?;
    let events =
        std::fs::read_to_string(events).map_err(|_| Error::new(E_FAIL, "无法读取显式宿主事件"))?;
    if events.len() > 65536 {
        return Err(Error::new(E_INVALIDARG, "宿主事件材料超限"));
    }
    let mut number = 0;
    for line in events
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let (case, sequence) = line
            .split_once('\t')
            .ok_or_else(|| Error::new(E_INVALIDARG, "缺少案例标识"))?;
        for token in sequence.split_ascii_whitespace() {
            let events = match token {
                "<Space>" => vec![InputEvent::Key(Key::Space)],
                "<Enter>" => vec![InputEvent::Key(Key::Enter)],
                "<Esc>" => vec![InputEvent::Key(Key::Escape)],
                "<Left>" => vec![InputEvent::Key(Key::Left)],
                "<Right>" => vec![InputEvent::Key(Key::Right)],
                "<Backspace>" => vec![InputEvent::Key(Key::Backspace)],
                "<Down>" => vec![InputEvent::Key(Key::Down)],
                "<PageDown>" => vec![InputEvent::Key(Key::PageDown)],
                "<Blur>" => vec![InputEvent::FocusLost],
                _ => token
                    .chars()
                    .map(|c| InputEvent::Key(Key::Char(c)))
                    .collect(),
            };
            for event in events {
                let now = Instant::now();
                let (frame, sample) = local.process(event, None, &|_| CommitOutcome::Applied)?;
                let total = now.elapsed().as_nanos();
                number += 1;
                println!(
                    "HOST_BENCH\t{case}\t{number}\t{event:?}\tcore={}\tprojection={}\tcompare={}\ttransaction={}\tframe={}\ttotal={}\trevision={}\twindow=not-run\tsink=memory-only",
                    sample.core_ns,
                    sample.projection_ns,
                    sample.comparison_ns,
                    sample.transaction_ns,
                    sample.frame_ns,
                    total,
                    frame.revision
                );
            }
        }
        let frame = local.host.engine.borrow().frame();
        println!("HOST_FRAME\t{case}\t{frame:?}");
        if let Some(diagnostic) = local.host.engine.borrow().search_diagnostic() {
            println!("HOST_SEARCH\t{case}\t{diagnostic:?}");
        }
    }
    println!(
        "HOST_SUMMARY events={number} document_bytes={} registered=false hwnd=not-run native_com_write=not-run formal_gold=null",
        local.document.borrow().len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round22_dual_translation_preflight_rejects_projection_without_touching_actual() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        type_text(&local, "nihao");
        let capture = Captured::new(&local, Direction::ZhEn, "").unwrap();
        let before = local.host.engine.borrow().frame();
        assert_eq!(capture.original, "你好");
        local
            .host
            .projection
            .borrow_mut()
            .handle(InputEvent::Key(Key::Char('m')));
        let outcome = local.translation_commit(&capture, "Hello.", None, &|_| {
            panic!("预检失败不能访问文档")
        });
        assert!(outcome.is_err());
        assert_eq!(local.host.engine.borrow().frame(), before);
        assert!(!local.host.quarantined.get());
        assert!(local.document.borrow().is_empty());
    }

    #[test]
    fn round22_translation_retry_has_new_ids_and_preserves_no_learning() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        type_text(&local, "nihao");
        let capture = Captured::new(&local, Direction::ZhEn, "").unwrap();
        let refused = local
            .translation_commit(&capture, "Hello.", None, &|doc| {
                assert_eq!(doc, "Hello.");
                assert!(local.host.engine.borrow().pending_selection().is_none());
                assert!(local.host.projection.borrow().pending_selection().is_none());
                CommitOutcome::NotApplied
            })
            .unwrap();
        assert_eq!(refused.outcome, CommitOutcome::NotApplied);
        assert_eq!(local.host.engine.borrow().frame().preedit, "nihao");
        assert!(local.document.borrow().is_empty());

        let retry = local
            .translation_commit(&capture, "Hello.", Some(refused), &|doc| {
                assert_eq!(doc, "Hello.");
                assert!(local.host.engine.borrow().pending_selection().is_none());
                CommitOutcome::Applied
            })
            .unwrap();
        assert_eq!(retry.outcome, CommitOutcome::Applied);
        assert_ne!(retry.actual, refused.actual);
        assert_ne!(retry.projected, refused.projected);
        assert_eq!(&*local.document.borrow(), "Hello.");
        assert!(local.host.engine.borrow().frame().preedit.is_empty());
        assert!(
            local
                .translation_commit(&capture, "Hello.", Some(refused), &|_| panic!(
                    "不能重复应用"
                ))
                .is_err()
        );
    }

    #[test]
    fn round22_discard_known_failed_translation_preserves_exact_composition() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        type_text(&local, "nihao");
        let mut before = local.host.engine.borrow().frame();
        let capture = Captured::new(&local, Direction::ZhEn, "").unwrap();
        let refused = local
            .translation_commit(&capture, "Hello.", None, &|_| CommitOutcome::NotApplied)
            .unwrap();
        local.discard_translation(refused).unwrap();
        let after = local.host.engine.borrow().frame();
        before.revision = after.revision;
        assert_eq!(before, after);
        assert!(local.host.projection.borrow().failed_commit().is_none());
        let (_, ordinary) = local
            .process(InputEvent::Key(Key::Space), None, &|doc| {
                assert_eq!(doc, "你好");
                CommitOutcome::Applied
            })
            .unwrap();
        assert_eq!(ordinary.selection, None, "64 条基线后端无真实搜索凭据");
        assert_eq!(&*local.document.borrow(), "你好");
    }

    #[test]
    fn round22_unknown_translation_preserves_english_and_never_replays() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        let capture = Captured::new(&local, Direction::EnZh, "Hello.").unwrap();
        let external_doc = RefCell::new(String::new());
        let unknown = local
            .translation_commit(&capture, "你好。", None, &|doc| {
                external_doc.replace(doc.into());
                CommitOutcome::Unknown
            })
            .unwrap();
        assert_eq!(unknown.outcome, CommitOutcome::Unknown);
        assert!(local.host.quarantined.get());
        assert!(local.retained.borrow().contains("Hello."));
        assert!(local.retained.borrow().contains("你好。"));
        assert!(local.discard_translation(unknown).is_err());
        assert!(
            local
                .translation_commit(&capture, "你好。", Some(unknown), &|_| panic!(
                    "Unknown 不能重放"
                ))
                .is_err()
        );
        assert_eq!(&*external_doc.borrow(), "你好。");
        *local.document.borrow_mut() = external_doc.take();
        local.reset().unwrap();
        assert!(
            local
                .translation_commit(&capture, "你好。", None, &|_| panic!(
                    "新会话不能应用旧译文"
                ))
                .is_err()
        );
        assert_eq!(&*local.document.borrow(), "你好。");
    }

    fn type_text(local: &LocalHost, value: &str) {
        for c in value.chars() {
            local
                .process(InputEvent::Key(Key::Char(c)), None, &|_| {
                    CommitOutcome::Applied
                })
                .unwrap();
        }
    }
    #[test]
    fn actual_host_controller_commit_once_stale_selection_and_original_fallback() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        assert!(local.host.context.borrow().is_none() && local.host.manager.borrow().is_none());
        type_text(&local, "nihao");
        let old = local.host.engine.borrow().frame();
        let writes = Cell::new(0);
        let sink = |_: &str| {
            writes.set(writes.get() + 1);
            CommitOutcome::Applied
        };
        local
            .process(
                InputEvent::Key(Key::Space),
                Some((old.revision, old.candidates[0].id.clone())),
                &sink,
            )
            .unwrap();
        assert_eq!(local.document.borrow().as_str(), "你好");
        assert_eq!(writes.get(), 1);
        assert!(
            local
                .process(
                    InputEvent::Key(Key::Space),
                    Some((old.revision, old.candidates[0].id.clone())),
                    &sink
                )
                .is_err()
        );
        assert_eq!(writes.get(), 1);
        type_text(&local, "qz");
        local
            .process(InputEvent::Key(Key::Enter), None, &sink)
            .unwrap();
        assert_eq!(local.document.borrow().as_str(), "你好qz");
    }
    #[test]
    fn unknown_blocks_replay_and_explicit_new_epoch_rejects_old_candidate() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        type_text(&local, "nihao");
        let old = local.host.engine.borrow().frame();
        let epoch = local.host.epoch.get();
        let writes = Cell::new(0);
        let sink = |_: &str| {
            writes.set(writes.get() + 1);
            CommitOutcome::Unknown
        };
        local
            .process(InputEvent::Key(Key::Space), None, &sink)
            .unwrap();
        assert_eq!(writes.get(), 1);
        assert!(local.host.quarantined.get());
        assert_eq!(
            local.host.engine.borrow().frame().state,
            cloudleaf_types::InputState::Committing
        );
        assert!(
            local
                .process(InputEvent::Key(Key::Space), None, &sink)
                .is_err()
        );
        assert!(local.process(InputEvent::FocusLost, None, &sink).is_err());
        assert_eq!(writes.get(), 1);
        assert!(local.retained.borrow().contains("nihao"));
        local.reset().unwrap();
        assert!(local.host.epoch.get() > epoch);
        assert!(
            local
                .process(
                    InputEvent::Key(Key::Space),
                    Some((old.revision, old.candidates[0].id.clone())),
                    &sink
                )
                .is_err()
        );
        assert_eq!(writes.get(), 1);
        assert!(!local.host.quarantined.get());
    }

    #[test]
    fn not_applied_keeps_original_and_a_later_explicit_selection_can_commit_once() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        type_text(&local, "nihao");
        let writes = Cell::new(0);
        let (frame, _) = local
            .process(InputEvent::Key(Key::Space), None, &|_| {
                writes.set(writes.get() + 1);
                CommitOutcome::NotApplied
            })
            .unwrap();
        assert_eq!(frame.preedit, "nihao");
        assert!(local.document.borrow().is_empty());
        assert!(!local.host.quarantined.get());
        assert_eq!(writes.get(), 1);
        local
            .process(InputEvent::RetryCommit, None, &|_| {
                writes.set(writes.get() + 1);
                CommitOutcome::Applied
            })
            .unwrap();
        assert_eq!(local.document.borrow().as_str(), "你好");
        assert_eq!(writes.get(), 2);
    }
    #[test]
    fn child_edit_focus_then_host_deactivation_preserves_raw_and_closes_candidates() {
        let view = Rc::new(Window::new(
            LocalHost::new(Host::new().unwrap()).unwrap(),
            TranslationController::new(None, Default::default()),
        ));
        type_text(&view.local, "nihao");
        let frame = view.local.host.engine.borrow().frame();
        *view.ui.borrow_mut() = Some(
            CandidateUi::isolated(view.local.host.clone(), frame, Rc::new(|_, _| Ok(()))).unwrap(),
        );
        // 安全：隐藏窗口使用实际WndProc；只向本测试创建的父子HWND发送焦点/激活消息。
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
            let class = w!("CloudleafRound17ChildFocusRegression");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            };
            assert_ne!(RegisterClassW(&wc), 0);
            let window = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!(""),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                770,
                580,
                None,
                None,
                Some(instance),
                Some(Rc::as_ptr(&view).cast()),
            )
            .unwrap();
            SetFocus(Some(window)).unwrap();
            SetFocus(Some(view.document.get())).unwrap();
            assert_eq!(GetFocus(), view.document.get());
            assert_eq!(view.local.host.engine.borrow().frame().preedit, "nihao");
            SendMessageW(
                window,
                WM_ACTIVATE,
                Some(WPARAM(WA_INACTIVE as usize)),
                Some(LPARAM(0)),
            );
            let after = view.local.host.engine.borrow().frame();
            let retained = view.local.retained.borrow().clone();
            let candidates_closed = view.ui.borrow().is_none();
            DestroyWindow(window).unwrap();
            let mut quit = MSG::default();
            let _ = PeekMessageW(&mut quit, None, WM_QUIT, WM_QUIT, PM_REMOVE);
            UnregisterClassW(class, Some(instance)).unwrap();
            assert!(
                after.preedit.is_empty(),
                "整窗失活必须取消旧组合，而不是依赖父窗仍有键盘焦点"
            );
            assert_eq!(retained, "nihao");
            assert!(candidates_closed);
            assert!(view.local.document.borrow().is_empty());
        }
    }

    #[test]
    fn isolated_window_and_child_do_not_inherit_external_ime() {
        use windows::Win32::UI::Input::Ime::{ImmGetContext, ImmReleaseContext};
        let view = Rc::new(Window::new(
            LocalHost::new(Host::new().unwrap()).unwrap(),
            TranslationController::new(None, Default::default()),
        ));
        // 安全：检查本测试隐藏父子窗口的上下文；不切换用户布局或其他窗口的输入法。
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
            let class = w!("CloudleafRound17OwnImeRegression");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            };
            assert_ne!(RegisterClassW(&wc), 0);
            let window = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!(""),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                770,
                580,
                None,
                None,
                Some(instance),
                Some(Rc::as_ptr(&view).cast()),
            )
            .unwrap();
            let mut inherited = Vec::new();
            for hwnd in [window, view.document.get()] {
                let context = ImmGetContext(hwnd);
                inherited.push(!context.is_invalid());
                if !context.is_invalid() {
                    assert!(ImmReleaseContext(hwnd, context).as_bool());
                }
            }
            DestroyWindow(window).unwrap();
            let mut quit = MSG::default();
            let _ = PeekMessageW(&mut quit, None, WM_QUIT, WM_QUIT, PM_REMOVE);
            UnregisterClassW(class, Some(instance)).unwrap();
            assert_eq!(
                inherited,
                [false, false],
                "隔离键盘入口不得再把拼音交给系统输入法"
            );
        }
    }

    #[test]
    fn repeated_focus_loss_and_unknown_preserve_each_unacknowledged_raw_input() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        type_text(&local, "nihao");
        local
            .process(InputEvent::FocusLost, None, &|_| CommitOutcome::Applied)
            .unwrap();
        assert!(local.retained.borrow().contains("nihao"));
        local
            .process(InputEvent::Activate, None, &|_| CommitOutcome::Applied)
            .unwrap();
        type_text(&local, "zhongguo");
        local
            .process(InputEvent::FocusLost, None, &|_| CommitOutcome::Applied)
            .unwrap();
        assert!(local.retained.borrow().contains("nihao"));
        assert!(local.retained.borrow().contains("zhongguo"));
        local
            .process(InputEvent::Activate, None, &|_| CommitOutcome::Applied)
            .unwrap();
        type_text(&local, "woaini");
        local
            .process(InputEvent::Key(Key::Space), None, &|_| {
                CommitOutcome::Unknown
            })
            .unwrap();
        assert!(local.host.quarantined.get());
        for raw in ["nihao", "zhongguo", "woaini"] {
            assert!(
                local.retained.borrow().contains(raw),
                "尚未确认的旧材料不能被后续失焦或Unknown覆盖：{raw}"
            );
        }
        local.reset().unwrap();
        assert!(
            local.retained.borrow().is_empty(),
            "明确新会话才确认并清除旧保全材料"
        );
    }

    #[test]
    fn retained_capacity_failure_quarantines_before_clearing_current_composition() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        *local.retained.borrow_mut() = "x".repeat(16384);
        type_text(&local, "nihao");
        let before = local.host.engine.borrow().frame();
        let projected_before = local.host.projection.borrow().frame();
        let result = local.process(InputEvent::FocusLost, None, &|_| CommitOutcome::Applied);
        assert!(result.is_err());
        assert!(local.host.quarantined.get());
        assert_eq!(local.retained.borrow().as_str(), "x".repeat(16384));
        assert_eq!(local.host.engine.borrow().frame(), before);
        assert_eq!(local.host.projection.borrow().frame(), projected_before);
        assert_eq!(before.preedit, "nihao");
        assert!(local.document.borrow().is_empty());
    }

    fn scheme_local() -> LocalHost {
        let snapshot = Arc::new(SearchSnapshot::from_verified_tsv("# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\nnihao\tni hao\t你好\t\tunknown\t\tmechanical\tnihao\tMIT\nnonghu\tnong hu\t农户\t\tunknown\t\tmechanical\tnonghu\tMIT\nzhongguo\tzhong guo\t中国\t\tunknown\t\tmechanical\tzhongguo\tMIT\nni0\tni\t你\t\tunknown\t\tmechanical\tni0\tMIT\nni1\tni\t尼\t\tunknown\t\tmechanical\tni1\tMIT\nni2\tni\t泥\t\tunknown\t\tmechanical\tni2\tMIT\nni3\tni\t拟\t\tunknown\t\tmechanical\tni3\tMIT\nni4\tni\t逆\t\tunknown\t\tmechanical\tni4\tMIT\nni5\tni\t匿\t\tunknown\t\tmechanical\tni5\tMIT\nni6\tni\t妮\t\tunknown\t\tmechanical\tni6\tMIT\nni7\tni\t霓\t\tunknown\t\tmechanical\tni7\tMIT\nni8\tni\t腻\t\tunknown\t\tmechanical\tni8\tMIT\nni9\tni\t溺\t\tunknown\t\tmechanical\tni9\tMIT\nni10\tni\t呢\t\tunknown\t\tmechanical\tni10\tMIT\nni11\tni\t旎\t\tunknown\t\tmechanical\tni11\tMIT\n", "87fc5f6e465ef421da4428b4e0fa08c88907b850c932b9b8690294d3fac53ded").unwrap());
        let config = cloudleaf_search::Config {
            ranker: cloudleaf_search::Ranker::Structural,
            method: cloudleaf_search::Method::PositionDag,
            output_k: 10,
            max_input_bytes: 64,
            max_letters: 64,
            ..Default::default()
        };
        LocalHost::new(Host::with_search(snapshot, config, InputConfig::default()).unwrap())
            .unwrap()
    }

    #[test]
    fn scheme_switch_preserves_raw_cursor_and_rejects_the_old_candidate() {
        let local = scheme_local();
        type_text(&local, "nihao");
        local
            .process(InputEvent::Key(Key::Left), None, &|_| {
                CommitOutcome::Applied
            })
            .unwrap();
        let before = local.host.engine.borrow().frame();
        let config = cloudleaf_input::SchemeConfig {
            scheme: cloudleaf_input::PinyinScheme::AbbreviatedPinyin,
            ..Default::default()
        };
        let after = local.switch_scheme(config.clone()).unwrap();
        assert_eq!(after.preedit, before.preedit);
        assert_eq!(after.cursor, before.cursor);
        assert!(after.revision > before.revision);
        assert_eq!(local.host.engine.borrow().pinyin_scheme(), Some(&config));
        assert_eq!(
            local.host.projection.borrow().pinyin_scheme(),
            Some(&config)
        );
        assert!(
            local
                .process(
                    InputEvent::Key(Key::Space),
                    Some((before.revision, before.candidates[0].id.clone())),
                    &|_| panic!("旧候选不能写文档")
                )
                .is_err()
        );
        assert!(local.document.borrow().is_empty());
    }

    #[test]
    fn failed_or_unknown_commit_blocks_settings_without_partial_change() {
        for outcome in [CommitOutcome::NotApplied, CommitOutcome::Unknown] {
            let local = scheme_local();
            type_text(&local, "nihao");
            local
                .process(InputEvent::Key(Key::Space), None, &|_| outcome)
                .unwrap();
            let actual = local.host.engine.borrow().frame();
            let projected = local.host.projection.borrow().frame();
            let config = cloudleaf_input::SchemeConfig {
                scheme: cloudleaf_input::PinyinScheme::AbbreviatedPinyin,
                ..Default::default()
            };
            assert!(local.switch_scheme(config).is_err());
            assert_eq!(local.host.engine.borrow().frame(), actual);
            assert_eq!(local.host.projection.borrow().frame(), projected);
            assert_eq!(
                local.host.engine.borrow().pinyin_scheme(),
                Some(&cloudleaf_input::SchemeConfig::default())
            );
            assert!(local.document.borrow().is_empty());
        }
    }

    #[test]
    fn projection_preflight_failure_never_changes_the_first_engine() {
        let local = scheme_local();
        type_text(&local, "nihao");
        local
            .host
            .projection
            .borrow_mut()
            .handle(InputEvent::Key(Key::Space));
        let actual = local.host.engine.borrow().frame();
        let projected = local.host.projection.borrow().frame();
        let config = cloudleaf_input::SchemeConfig {
            scheme: cloudleaf_input::PinyinScheme::AbbreviatedPinyin,
            ..Default::default()
        };
        assert!(local.switch_scheme(config).is_err());
        assert_eq!(local.host.engine.borrow().frame(), actual);
        assert_eq!(local.host.projection.borrow().frame(), projected);
    }

    #[test]
    fn configured_host_keeps_page_selection_in_the_real_input_core() {
        let local = scheme_local();
        type_text(&local, "ni");
        let before = local.host.engine.borrow().frame();
        assert_eq!(before.candidates.len(), 10);
        local
            .process(InputEvent::Key(Key::PageDown), None, &|_| {
                CommitOutcome::Applied
            })
            .unwrap();
        let paged = local.host.engine.borrow().frame();
        assert_eq!(paged.page_start, 5);
        let expected = paged.candidates[6].text.clone();
        local
            .process(InputEvent::Key(Key::Char('2')), None, &|_| {
                CommitOutcome::Applied
            })
            .unwrap();
        assert_eq!(local.document.borrow().as_str(), expected);
        assert_eq!(local.host.engine.borrow().frame().preedit, "");
    }

    #[test]
    fn selection_proof_is_returned_only_after_applied_and_retry_has_new_identity() {
        let local = scheme_local();
        type_text(&local, "nihao");
        let (_, rejected) = local
            .process(InputEvent::Key(Key::Space), None, &|_| {
                CommitOutcome::NotApplied
            })
            .unwrap();
        assert!(rejected.selection.is_none());
        let failed = local
            .host
            .engine
            .borrow()
            .pending_selection()
            .unwrap()
            .clone();
        assert!(local.document.borrow().is_empty());
        let (_, applied) = local
            .process(InputEvent::RetryCommit, None, &|_| CommitOutcome::Applied)
            .unwrap();
        let selected = applied.selection.unwrap();
        assert_ne!(selected.commit_id, failed.commit_id);
        assert_eq!(selected.entries, failed.entries);
        assert_eq!(selected.candidate_id, failed.candidate_id);
        assert!(local.host.engine.borrow().pending_selection().is_none());
        let before = local.document.borrow().clone();
        assert!(
            local
                .process(
                    InputEvent::CommitSucceeded {
                        id: failed.commit_id
                    },
                    None,
                    &|_| { panic!("迟到确认不能再次调用写入端") }
                )
                .is_err()
        );
        assert_eq!(*local.document.borrow(), before);

        let unknown = scheme_local();
        type_text(&unknown, "nihao");
        let (_, uncertain) = unknown
            .process(InputEvent::Key(Key::Space), None, &|_| {
                CommitOutcome::Unknown
            })
            .unwrap();
        assert!(uncertain.selection.is_none());
        assert!(unknown.host.quarantined.get());
        assert!(unknown.document.borrow().is_empty());
        assert!(
            unknown
                .process(InputEvent::RetryCommit, None, &|_| {
                    panic!("Unknown不能自动重放或产生学习凭据")
                })
                .is_err()
        );
        assert!(unknown.retained.borrow().contains("nihao"));
    }
}
