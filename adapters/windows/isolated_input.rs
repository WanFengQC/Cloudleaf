//! 普通Win32隔离宿主，只写本窗口的文档，不取得任何TSF manager/context。
use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use windows::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
use windows_core::{PCWSTR, w};

#[derive(Default, Debug)]
pub(crate) struct Sample {
    pub core_ns: u128,
    pub projection_ns: u128,
    pub comparison_ns: u128,
    pub transaction_ns: u128,
    pub frame_ns: u128,
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
        Ok(())
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
                *self.retained.borrow_mut() = raw;
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
        if let Some(commit) = actual.commit {
            let mut document = self.document.borrow().clone();
            document.push_str(&commit.text);
            let outcome = if document.len() > 16384 {
                CommitOutcome::NotApplied
            } else {
                sink(&document)
            };
            match outcome {
                CommitOutcome::Applied => {
                    *self.document.borrow_mut() = document;
                    self.host
                        .engine
                        .borrow_mut()
                        .handle(InputEvent::CommitSucceeded { id: commit.id });
                    self.host
                        .projection
                        .borrow_mut()
                        .handle(InputEvent::CommitSucceeded {
                            id: projected.commit.unwrap().id,
                        });
                }
                CommitOutcome::NotApplied => {
                    self.host
                        .engine
                        .borrow_mut()
                        .handle(InputEvent::CommitFailed { id: commit.id });
                    self.host
                        .projection
                        .borrow_mut()
                        .handle(InputEvent::CommitFailed {
                            id: projected.commit.unwrap().id,
                        });
                }
                CommitOutcome::Unknown => {
                    *self.retained.borrow_mut() = format!(
                        "拼音：{}；待核对提交：{}",
                        actual.frame.preedit, commit.text
                    );
                    self.host.quarantined.set(true);
                    self.host.unknown_text.set(true);
                }
            }
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
                    *self.retained.borrow_mut() = format!("原生事件待核对：{event:?}");
                    self.host.quarantined.set(true);
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

struct Window {
    local: LocalHost,
    window: Cell<HWND>,
    document: Cell<HWND>,
    status: Cell<HWND>,
    fault: Cell<bool>,
    ui: RefCell<Option<CandidateUi>>,
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
    fn update(self: &Rc<Self>, frame: InputFrame) -> Result<()> {
        let message = format!(
            "拼音：{}  修订：{}  {:?}\r\n{}{}",
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
        rect.bottom = rect.top + 142;
        self.ui.borrow_mut().as_mut().unwrap().update(frame, rect)
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
                    CreateWindowExW(WINDOW_EX_STYLE::default(),w!("STATIC"),w!("直接键入拼音；Space/数字选词，方向键/翻页，Enter确认，未识别拼音原文回退，Esc取消。"),WS_CHILD|WS_VISIBLE,12,10,720,30,Some(window),None,None,None)?;
                    view.status.set(CreateWindowExW(WINDOW_EX_STYLE::default(),w!("STATIC"),w!(""),WS_CHILD|WS_VISIBLE,12,44,720,56,Some(window),None,None,None)?);
                    for (id,label,x,width) in [(1,"下一次提交注入Unknown",12,230),(2,"核对后开始新会话",260,210)] {
                        CreateWindowExW(WINDOW_EX_STYLE::default(),w!("BUTTON"),PCWSTR(wide(label).as_ptr()),WS_CHILD|WS_VISIBLE,x,104,width,30,Some(window),Some(HMENU(id as *mut _)),None,None)?;
                    }
                    view.document.set(CreateWindowExW(WS_EX_CLIENTEDGE,w!("EDIT"),w!(""),WS_CHILD|WS_VISIBLE|WS_VSCROLL|WINDOW_STYLE(ES_READONLY as u32|ES_MULTILINE as u32|ES_AUTOVSCROLL as u32),12,320,720,200,Some(window),None,None,None)?);
                    Ok(LRESULT(0))
                }
                WM_CHAR => {
                    let c = char::from_u32(wparam.0 as u32);
                    if let Some(c) = c.filter(|c| !c.is_control()) { let key = if c==' ' {Key::Space} else {Key::Char(c)}; view.process(InputEvent::Key(key),None)?; }
                    Ok(LRESULT(0))
                }
                WM_KEYDOWN => {
                    let key = match wparam.0 as u16 {
                        x if x==VK_BACK.0 => Some(Key::Backspace), x if x==VK_RETURN.0 => Some(Key::Enter), x if x==VK_ESCAPE.0 => Some(Key::Escape),
                        x if x==VK_LEFT.0 => Some(Key::Left),x if x==VK_RIGHT.0 => Some(Key::Right),x if x==VK_UP.0 => Some(Key::Up),x if x==VK_DOWN.0 => Some(Key::Down),x if x==VK_PRIOR.0 => Some(Key::PageUp),x if x==VK_NEXT.0 => Some(Key::PageDown),_=>None};
                    if let Some(key)=key { view.process(InputEvent::Key(key),None)?; }
                    Ok(LRESULT(0))
                }
                WM_COMMAND if wparam.0&0xffff==1 => { view.fault.set(true); let _=SetFocus(Some(window)); Ok(LRESULT(0)) }
                WM_COMMAND if wparam.0&0xffff==2 => {
                    view.ui.borrow_mut().take();
                    // 明确新会话接受当前本窗口文档，旧Unknown请求只失效，不重发。
                    *view.local.document.borrow_mut()=text(view.document.get());
                    view.local.reset()?; let frame=view.local.host.engine.borrow().frame(); view.update(frame)?; let _=SetFocus(Some(window)); Ok(LRESULT(0))
                }
                WM_KILLFOCUS if !view.local.host.quarantined.get() && GetAncestor(HWND(wparam.0 as *mut _),GA_ROOT)!=window => { view.process(InputEvent::FocusLost,None)?; Ok(LRESULT(0)) }
                WM_SETFOCUS => { if !view.local.host.quarantined.get() { view.process(InputEvent::Activate,None)?; } Ok(LRESULT(0)) }
                WM_CLOSE => { view.ui.borrow_mut().take(); DestroyWindow(window)?; Ok(LRESULT(0)) }
                WM_DESTROY => { PostQuitMessage(0); Ok(LRESULT(0)) }
                WM_NCDESTROY => { SetWindowLongPtrW(window,GWLP_USERDATA,0); Ok(DefWindowProcW(window,message,wparam,lparam)) }
                _ => Ok(DefWindowProcW(window,message,wparam,lparam)),
            }
        }
    }).unwrap_or(LRESULT(0))
}

pub(crate) fn run(runtime: &str, runtime_sha: &str, model: Option<(&str, &str)>) -> Result<()> {
    let (host, _, _) = super::input_review::load_host(runtime, runtime_sha, model)?;
    let view = Rc::new(Window {
        local: LocalHost::new(host)?,
        window: Cell::new(HWND::default()),
        document: Cell::new(HWND::default()),
        status: Cell::new(HWND::default()),
        fault: Cell::new(false),
        ui: RefCell::new(None),
    });
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
            w!("云笺 · 隔离中文输入宿主（非TSF）"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            100,
            100,
            770,
            580,
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
        let view = Rc::new(Window {
            local: LocalHost::new(Host::new().unwrap()).unwrap(),
            window: Cell::new(HWND::default()),
            document: Cell::new(HWND::default()),
            status: Cell::new(HWND::default()),
            fault: Cell::new(false),
            ui: RefCell::new(None),
        });
        type_text(&view.local, "nihao");
        let frame = view.local.host.engine.borrow().frame();
        *view.ui.borrow_mut() = Some(CandidateUi::isolated(
            view.local.host.clone(), frame, Rc::new(|_, _| Ok(())),
        ).unwrap());
        // 安全：隐藏窗口使用实际WndProc；只向本测试创建的父子HWND发送焦点/激活消息。
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
            let class = w!("CloudleafRound17ChildFocusRegression");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc), hInstance: instance,
                lpszClassName: class, ..Default::default()
            };
            assert_ne!(RegisterClassW(&wc), 0);
            let window = CreateWindowExW(
                WINDOW_EX_STYLE::default(), class, w!(""), WS_OVERLAPPEDWINDOW,
                0, 0, 770, 580, None, None, Some(instance),
                Some(Rc::as_ptr(&view).cast()),
            ).unwrap();
            SetFocus(Some(window)).unwrap();
            SetFocus(Some(view.document.get())).unwrap();
            assert_eq!(GetFocus(), view.document.get());
            assert_eq!(view.local.host.engine.borrow().frame().preedit, "nihao");
            SendMessageW(window, WM_ACTIVATE, Some(WPARAM(WA_INACTIVE as usize)), Some(LPARAM(0)));
            let after = view.local.host.engine.borrow().frame();
            let retained = view.local.retained.borrow().clone();
            let candidates_closed = view.ui.borrow().is_none();
            DestroyWindow(window).unwrap();
            let mut quit = MSG::default();
            let _ = PeekMessageW(&mut quit, None, WM_QUIT, WM_QUIT, PM_REMOVE);
            UnregisterClassW(class, Some(instance)).unwrap();
            assert!(after.preedit.is_empty(), "整窗失活必须取消旧组合，而不是依赖父窗仍有键盘焦点");
            assert_eq!(retained, "nihao");
            assert!(candidates_closed);
            assert!(view.local.document.borrow().is_empty());
        }
    }

}
