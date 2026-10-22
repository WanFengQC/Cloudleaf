//! 会话与编辑事务；TSF 锁内只操作内存与宿主文本。
use super::{boundary, candidate::CandidateUi};
use cloudleaf_dictionary::{Dictionary, PROTOTYPE_TSV};
use cloudleaf_input::InputEngine;
use cloudleaf_types::{
    CommitOutcome, EventResult, InputEvent, InputFrame, Key, KeyDisposition, utf8_to_utf16_offset,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{E_FAIL, E_UNEXPECTED, RECT},
    System::{
        Com::CoTaskMemFree,
        Threading::GetCurrentThreadId,
        Variant::{VARIANT, VT_EMPTY, VT_UNKNOWN, VariantClear},
    },
    UI::TextServices::*,
};
use windows_core::{Error, Interface, Result, implement};
pub(crate) const QUEUE_LIMIT: usize = 32;
pub(crate) type Shared = Rc<Host>;
pub(crate) struct Host {
    _lease: super::service::Lease,
    pub owner: u32,
    pub epoch: Cell<u64>,
    pub engine: RefCell<InputEngine>,
    pub context: RefCell<Option<ITfContext>>,
    pub composition: RefCell<Option<ITfComposition>>,
    pub manager: RefCell<Option<ITfThreadMgr>>,
    pub client: Cell<u32>,
    pub ui: RefCell<Option<CandidateUi>>,
    queue: RefCell<VecDeque<Operation>>,
    scheduled: Cell<bool>,
    pending_layout: RefCell<Option<(u64, Instant)>>,
    next_operation: Cell<u64>,
    layout_cookie: Cell<Option<u32>>,
    retry_layout: Cell<bool>,
    unknown_text: Cell<bool>,
    pub quarantined: Cell<bool>,
    pub ending: Cell<bool>,
}
#[derive(Clone)]
enum Operation {
    Key(Key),
    Select(u64, String),
    Refresh,
    Cancel,
}
impl Host {
    pub fn new() -> Result<Shared> {
        let dictionary =
            Dictionary::from_tsv(PROTOTYPE_TSV).map_err(|_| Error::from_hresult(E_FAIL))?;
        Ok(Rc::new(Self {
            _lease: super::service::Lease::new(),
            // 安全：只读取线程标识，没有外部资源。
            owner: unsafe { GetCurrentThreadId() },
            epoch: Cell::new(1),
            engine: RefCell::new(InputEngine::new(dictionary)),
            context: RefCell::new(None),
            composition: RefCell::new(None),
            manager: RefCell::new(None),
            client: Cell::new(0),
            ui: RefCell::new(None),
            queue: RefCell::new(VecDeque::new()),
            scheduled: Cell::new(false),
            pending_layout: RefCell::new(None),
            next_operation: Cell::new(1),
            layout_cookie: Cell::new(None),
            retry_layout: Cell::new(false),
            unknown_text: Cell::new(false),
            quarantined: Cell::new(false),
            ending: Cell::new(false),
        }))
    }
    pub fn check(&self) -> Result<()> {
        // 安全：只比较本线程标识。
        if unsafe { GetCurrentThreadId() } != self.owner {
            return Err(Error::from_hresult(E_UNEXPECTED));
        }
        Ok(())
    }
    pub fn disposition(&self, key: Key) -> Result<bool> {
        self.check()?;
        if self.context.borrow().is_none() {
            return Ok(false);
        }
        if self.quarantined.get() {
            return Ok(key == Key::Escape);
        }
        if self.scheduled.get() || !self.queue.borrow().is_empty() {
            return Ok(false);
        }
        Ok(matches!(
            self.engine
                .try_borrow()
                .map_err(|_| Error::from_hresult(E_UNEXPECTED))?
                .key_disposition(key),
            KeyDisposition::Handle
        ))
    }
    pub fn focus(self: &Shared, context: Option<ITfContext>) -> Result<()> {
        self.check()?;
        if *self.context.borrow() == context {
            return Ok(());
        }
        self.invalidate()?;
        *self.context.borrow_mut() = context.clone();
        if let Some(context) = context {
            let observer: ITfTextLayoutSink = LayoutObserver {
                host: Rc::downgrade(self),
                epoch: self.epoch.get(),
                _lease: super::service::Lease::new(),
            }
            .into();
            // 安全：订阅当前 apartment 的 context，弱引用不会形成 COM 循环。
            let subscribed = unsafe {
                context
                    .cast::<ITfSource>()
                    .and_then(|source| source.AdviseSink(&ITfTextLayoutSink::IID, &observer))
            };
            let cookie = match subscribed {
                Ok(cookie) => cookie,
                Err(error) => {
                    self.context.borrow_mut().take();
                    return Err(error);
                }
            };
            self.layout_cookie.set(Some(cookie));
            self.engine.borrow_mut().handle(InputEvent::Activate);
        }
        Ok(())
    }
    pub fn invalidate(self: &Shared) -> Result<()> {
        self.check()?;
        self.epoch.set(
            self.epoch
                .get()
                .checked_add(1)
                .ok_or_else(|| Error::from_hresult(E_UNEXPECTED))?,
        );
        self.queue.borrow_mut().clear();
        self.scheduled.set(false);
        self.pending_layout.borrow_mut().take();
        self.engine.borrow_mut().handle(InputEvent::FocusLost);
        let erase = !self.unknown_text.replace(false);
        self.quarantined.set(false);
        let ui = self.ui.borrow_mut().take();
        let composition = self.composition.borrow_mut().take();
        let context = self.context.borrow_mut().take();
        let cookie = self.layout_cookie.take();
        let client = self.client.get();
        self.retry_layout.set(false);
        // 所有旧资源先一次性脱离 Host；清理外调即使重入，也不会取走新 epoch 的资源。
        if let Some(mut ui) = ui {
            ui.close();
        }
        let mut failure = None;
        if let (Some(context), Some(cookie)) = (&context, cookie) {
            // 安全：先使 epoch 失效再撤销旧 context 的布局 sink。
            unsafe {
                if let Err(error) = context
                    .cast::<ITfSource>()
                    .and_then(|source| source.UnadviseSink(cookie))
                {
                    failure = Some(error);
                }
            }
        }
        if let (Some(composition), Some(context)) = (composition, context) {
            let cleanup: ITfEditSession = Cleanup {
                composition,
                erase,
                _lease: super::service::Lease::new(),
            }
            .into();
            // 安全：清理只持有旧组合，独立于新会话队列；迟到回调不能写新文档。
            unsafe {
                let result = context
                    .RequestEditSession(client, &cleanup, TF_ES_READWRITE | TF_ES_ASYNCDONTCARE)
                    .and_then(|hr| hr.ok());
                if failure.is_none() {
                    failure = result.err();
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }
    pub fn key(self: &Shared, key: Key) -> Result<bool> {
        if self.layout_expired() {
            self.quarantined.set(true);
        }
        if !self.disposition(key)? {
            return Ok(false);
        }
        if key == Key::Escape && self.quarantined.get() {
            self.invalidate()?;
            return Ok(true);
        }
        if self.queue.borrow().len() >= QUEUE_LIMIT {
            self.quarantined.set(true);
            return Err(Error::from_hresult(E_FAIL));
        }
        let Some(context) = self.context.borrow().clone() else {
            return Ok(false);
        };
        let executed = Rc::new(Cell::new(false));
        let epoch = self.epoch.get();
        let edit: ITfEditSession = KeyEdit {
            host: self.clone(),
            context: context.clone(),
            epoch,
            action: Operation::Key(key),
            executed: executed.clone(),
        }
        .into();
        self.scheduled.set(true);
        // 安全：按键只允许同步写锁；回调运行之前不改核心或队列，拒锁原生键透传。
        let requested = unsafe {
            context.RequestEditSession(self.client.get(), &edit, TF_ES_READWRITE | TF_ES_SYNC)
        };
        if epoch == self.epoch.get() {
            self.scheduled.set(false);
        }
        if executed.get() {
            if !matches!(requested, Ok(hr) if hr.is_ok()) {
                self.quarantined.set(true);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn select(self: &Shared, revision: u64, id: String) -> Result<()> {
        self.check()?;
        if self.quarantined.get() || self.scheduled.get() || !self.queue.borrow().is_empty() {
            return Err(Error::from_hresult(E_FAIL));
        }
        self.sync_operation(Operation::Select(revision, id))
    }
    pub fn cancel(self: &Shared) -> Result<()> {
        self.check()?;
        if self.quarantined.get() {
            return self.invalidate();
        }
        if self.queue.borrow().len() >= QUEUE_LIMIT {
            return Err(Error::from_hresult(E_FAIL));
        }
        if self.scheduled.get() && self.pending_layout.borrow().is_none() {
            self.queue.borrow_mut().push_back(Operation::Cancel);
            Ok(())
        } else {
            self.sync_operation(Operation::Cancel)
        }
    }
    fn sync_operation(self: &Shared, action: Operation) -> Result<()> {
        let context = self
            .context
            .borrow()
            .clone()
            .ok_or_else(|| Error::from_hresult(E_FAIL))?;
        let epoch = self.epoch.get();
        let executed = Rc::new(Cell::new(false));
        let edit: ITfEditSession = KeyEdit {
            host: self.clone(),
            epoch,
            context: context.clone(),
            action,
            executed: executed.clone(),
        }
        .into();
        self.scheduled.set(true);
        // 安全：候选和独立取消不允许延迟写；未授锁不会改变核心。
        let result = unsafe {
            context.RequestEditSession(self.client.get(), &edit, TF_ES_READWRITE | TF_ES_SYNC)
        }
        .and_then(|hr| hr.ok());
        if epoch == self.epoch.get() {
            self.scheduled.set(false);
        }
        result?;
        if executed.get() {
            Ok(())
        } else {
            Err(Error::from_hresult(E_FAIL))
        }
    }
    fn layout_expired(&self) -> bool {
        self.pending_layout
            .borrow()
            .as_ref()
            .is_some_and(|(_, start)| start.elapsed() > Duration::from_millis(500))
    }
    fn schedule(self: &Shared) -> Result<()> {
        if self.scheduled.replace(true) {
            return Ok(());
        }
        let Some(context) = self.context.borrow().clone() else {
            self.scheduled.set(false);
            return Ok(());
        };
        let edit: ITfEditSession = Edit {
            host: self.clone(),
            epoch: self.epoch.get(),
            context: context.clone(),
            operation: self.next_operation.get(),
        }
        .into();
        let operation = self.next_operation.get();
        self.next_operation.set(
            operation
                .checked_add(1)
                .ok_or_else(|| Error::from_hresult(E_UNEXPECTED))?,
        );
        *self.pending_layout.borrow_mut() = Some((operation, Instant::now()));
        // 安全：接口属于本 apartment，回调对象拥有全部状态，TSF 管理引用和写锁。
        let result = unsafe {
            context.RequestEditSession(
                self.client.get(),
                &edit,
                TF_ES_READWRITE | TF_ES_ASYNCDONTCARE,
            )
        };
        match result {
            Ok(hr) if hr.is_ok() => Ok(()),
            Ok(hr) => {
                self.scheduled.set(false);
                self.pending_layout.borrow_mut().take();
                self.quarantined.set(true);
                Err(Error::from_hresult(hr))
            }
            Err(e) => {
                self.scheduled.set(false);
                self.pending_layout.borrow_mut().take();
                self.quarantined.set(true);
                Err(e)
            }
        }
    }
    fn run(self: &Shared, context: &ITfContext, ec: u32, epoch: u64) -> Result<()> {
        self.check()?;
        if epoch != self.epoch.get() || self.context.borrow().as_ref() != Some(context) {
            return Ok(());
        }
        loop {
            if self.quarantined.get() || epoch != self.epoch.get() {
                break;
            }
            let Some(op) = self.queue.borrow_mut().pop_front() else {
                break;
            };
            let result = match op {
                Operation::Key(k) => self.engine.borrow_mut().handle(InputEvent::Key(k)),
                Operation::Select(r, id) => self.engine.borrow_mut().select_candidate(r, &id),
                Operation::Cancel => self
                    .engine
                    .borrow_mut()
                    .handle(InputEvent::Key(Key::Escape)),
                Operation::Refresh => {
                    let frame = self.engine.borrow().frame();
                    self.update_ui(context, ec, frame)?;
                    continue;
                }
            };
            if result.error.is_some() {
                continue;
            }
            let outcome = self.render(context, ec, &result);
            // 外部 COM 可重入焦点通知；旧事务不能确认新会话或继续消费新队列。
            if epoch != self.epoch.get() {
                return Ok(());
            }
            if let Some(commit) = result.commit {
                match outcome {
                    Ok(()) => {
                        self.unknown_text.set(false);
                        self.engine
                            .borrow_mut()
                            .handle(InputEvent::CommitSucceeded { id: commit.id });
                    }
                    Err((CommitOutcome::NotApplied, e)) => {
                        self.engine
                            .borrow_mut()
                            .handle(InputEvent::CommitFailed { id: commit.id });
                        self.quarantined.set(true);
                        return Err(e);
                    }
                    Err((_, e)) => {
                        self.unknown_text.set(true);
                        self.quarantined.set(true);
                        return Err(e);
                    }
                }
            } else if let Err((_, e)) = outcome {
                self.quarantined.set(true);
                return Err(e);
            }
            let frame = self.engine.borrow().frame();
            self.update_ui(context, ec, frame)?;
        }
        if epoch == self.epoch.get() {
            self.scheduled.set(false);
        }
        Ok(())
    }
    fn erase(&self, ec: u32) -> Result<()> {
        let composition = self.composition.borrow_mut().take();
        if let Some(c) = composition {
            self.ending.set(true);
            // 安全：在对应 context 写锁内，删除仅限自己的组合范围。
            let r = unsafe {
                c.GetRange()
                    .and_then(|r| r.SetText(ec, 0, &[]))
                    .and_then(|_| c.EndComposition(ec))
            };
            self.ending.set(false);
            r?;
        }
        Ok(())
    }
    fn render(
        self: &Shared,
        context: &ITfContext,
        ec: u32,
        result: &EventResult,
    ) -> std::result::Result<(), (CommitOutcome, Error)> {
        let mut attempted = false;
        let epoch = self.epoch.get();
        let current = || -> Result<()> {
            if self.epoch.get() != epoch || self.context.borrow().as_ref() != Some(context) {
                return Err(Error::from_hresult(windows::Win32::Foundation::E_ABORT));
            }
            Ok(())
        };
        let mut action = || -> Result<()> {
            // 安全：写会话内操作同 apartment COM；RefCell 引用不跨 COM 外调。
            unsafe {
                let status = context.GetStatus()?;
                current()?;
                if status.dwDynamicFlags & TS_SD_READONLY != 0 {
                    return Err(Error::from_hresult(E_FAIL));
                }
                let composition = self.composition.borrow().clone();
                let range = if let Some(c) = &composition {
                    let range = c.GetRange()?;
                    current()?;
                    range
                } else {
                    let mut s = [TF_SELECTION::default()];
                    let mut n = 0;
                    let selected = context.GetSelection(ec, TF_DEFAULT_SELECTION, &mut s, &mut n);
                    let range = s[0].range.take();
                    selected?;
                    current()?;
                    if n != 1 {
                        return Err(Error::from_hresult(E_FAIL));
                    }
                    range.ok_or_else(|| Error::from_hresult(E_FAIL))?
                };
                let text = result
                    .commit
                    .as_ref()
                    .map_or(result.frame.preedit.as_str(), |c| c.text.as_str());
                if text.is_empty() {
                    return self.erase(ec);
                }
                if composition.is_none() {
                    let sink: ITfCompositionSink = CompositionObserver {
                        host: Rc::downgrade(self),
                        epoch: self.epoch.get(),
                        _lease: super::service::Lease::new(),
                    }
                    .into();
                    let creator = context.cast::<ITfContextComposition>()?;
                    current()?;
                    let c = creator.StartComposition(ec, &range, &sink)?;
                    if let Err(error) = current() {
                        // 安全：迟到的新组合仅在旧锁内结束，不能安装到新 epoch 的 Host。
                        let _ = c.EndComposition(ec);
                        return Err(error);
                    }
                    *self.composition.borrow_mut() = Some(c);
                }
                let utf16: Vec<u16> = text.encode_utf16().collect();
                attempted = true;
                // 写入调用本身也可重入失焦；在外调之前标记不确定，旧清理绝不删除可能成功的提交。
                if result.commit.is_some() {
                    self.unknown_text.set(true);
                }
                range.SetText(ec, 0, &utf16)?;
                current()?;
                let caret = range.Clone()?;
                current()?;
                caret.Collapse(ec, TF_ANCHOR_START)?;
                current()?;
                let cursor = if result.commit.is_some() {
                    utf16.len()
                } else {
                    utf8_to_utf16_offset(text, result.frame.cursor)
                        .map_err(|_| Error::from_hresult(E_FAIL))?
                };
                let mut shifted = 0;
                caret.ShiftStart(ec, cursor as i32, &mut shifted, std::ptr::null())?;
                current()?;
                caret.Collapse(ec, TF_ANCHOR_START)?;
                current()?;
                if shifted != cursor as i32 {
                    return Err(Error::from_hresult(E_FAIL));
                }
                let s = TF_SELECTION {
                    range: std::mem::ManuallyDrop::new(Some(caret)),
                    style: TF_SELECTIONSTYLE {
                        ase: TF_AE_NONE,
                        fInterimChar: false.into(),
                    },
                };
                let selected = context.SetSelection(ec, std::slice::from_ref(&s));
                drop(std::mem::ManuallyDrop::into_inner(s.range));
                selected?;
                current()?;
                if result.commit.is_some() {
                    let c = self.composition.borrow_mut().take();
                    if let Some(c) = c {
                        self.ending.set(true);
                        let r = c.EndComposition(ec);
                        self.ending.set(false);
                        if r.is_err() && epoch == self.epoch.get() {
                            *self.composition.borrow_mut() = Some(c);
                        }
                        r?;
                        current()?;
                    }
                }
            }
            Ok(())
        };
        action().map_err(|e| {
            (
                if attempted {
                    CommitOutcome::Unknown
                } else {
                    CommitOutcome::NotApplied
                },
                e,
            )
        })
    }
    fn update_ui(self: &Shared, context: &ITfContext, ec: u32, frame: InputFrame) -> Result<()> {
        let epoch = self.epoch.get();
        let current = || -> Result<()> {
            if epoch != self.epoch.get() || self.context.borrow().as_ref() != Some(context) {
                return Err(Error::from_hresult(windows::Win32::Foundation::E_ABORT));
            }
            Ok(())
        };
        current()?;
        if frame.candidates.is_empty() || frame.preedit.is_empty() {
            let old = self.ui.borrow_mut().take();
            if let Some(mut ui) = old {
                ui.close();
            }
            return Ok(());
        }
        let composition = self.composition.borrow().clone();
        let mut rect = RECT::default();
        let mut clipped = false.into();
        // 安全：布局输出引用为有效局部值，读取当前锁内自己的组合范围。
        if let Some(c) = composition {
            let layout = unsafe {
                let view = context.GetActiveView()?;
                current()?;
                let range = c.GetRange()?;
                current()?;
                view.GetTextExt(ec, &range, &mut rect, &mut clipped)
            };
            current()?;
            if let Err(error) = layout {
                if error.code() != TS_E_NOLAYOUT {
                    return Err(error);
                }
                self.retry_layout.set(true);
                let old = self.ui.borrow_mut().take();
                if let Some(mut ui) = old {
                    ui.close();
                }
                return Ok(());
            }
        }
        self.retry_layout.set(false);
        let old = self.ui.borrow_mut().take();
        let mut ui = match old {
            Some(ui) => ui,
            None => CandidateUi::new(self.clone(), context.clone(), frame.clone())?,
        };
        current()?;
        ui.update(frame, rect)?;
        current()?;
        *self.ui.borrow_mut() = Some(ui);
        Ok(())
    }
}
#[implement(ITfTextLayoutSink, Agile = false)]
struct LayoutObserver {
    host: std::rc::Weak<Host>,
    epoch: u64,
    _lease: super::service::Lease,
}
impl ITfTextLayoutSink_Impl for LayoutObserver_Impl {
    fn OnLayoutChange(
        &self,
        _context: windows_core::Ref<ITfContext>,
        code: TfLayoutCode,
        _view: windows_core::Ref<ITfContextView>,
    ) -> Result<()> {
        boundary(|| {
            let Some(host) = self.host.upgrade() else {
                return Ok(());
            };
            host.check()?;
            if self.epoch != host.epoch.get() {
                return Ok(());
            }
            if code == TF_LC_DESTROY {
                return host.invalidate();
            }
            if host.retry_layout.get() && !host.quarantined.get() {
                if host.queue.borrow().len() >= QUEUE_LIMIT {
                    host.quarantined.set(true);
                    return Err(Error::from_hresult(E_FAIL));
                }
                if !host
                    .queue
                    .borrow()
                    .iter()
                    .any(|op| matches!(op, Operation::Refresh))
                {
                    host.queue.borrow_mut().push_back(Operation::Refresh);
                }
                host.schedule()?;
            }
            Ok(())
        })
    }
}
#[implement(ITfEditSession, Agile = false)]
struct Edit {
    host: Shared,
    epoch: u64,
    context: ITfContext,
    operation: u64,
}
#[implement(ITfEditSession, Agile = false)]
struct KeyEdit {
    host: Shared,
    epoch: u64,
    context: ITfContext,
    action: Operation,
    executed: Rc<Cell<bool>>,
}
impl ITfEditSession_Impl for KeyEdit_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        boundary(|| {
            self.host.check()?;
            if self.epoch != self.host.epoch.get()
                || self.host.context.borrow().as_ref() != Some(&self.context)
            {
                return Ok(());
            }
            // 安全：授锁后的只读状态检查先于核心处理；拒绝 readonly 时不吃键。
            if unsafe { self.context.GetStatus()?.dwDynamicFlags & TS_SD_READONLY != 0 } {
                return Ok(());
            }
            if !scope_allows_input(&self.context, ec)? || self.epoch != self.host.epoch.get() {
                return Ok(());
            }
            self.executed.set(true);
            self.host.queue.borrow_mut().push_back(self.action.clone());
            let result = self.host.run(&self.context, ec, self.epoch);
            if result.is_err() && self.epoch == self.host.epoch.get() {
                self.host.quarantined.set(true);
            }
            result
        })
    }
}
struct OwnedVariant(VARIANT);
impl Drop for OwnedVariant {
    fn drop(&mut self) {
        // 安全：GetValue 返回的 VARIANT 所有权仅由本包装持有且只清理一次。
        unsafe {
            let _ = VariantClear(&mut self.0);
        }
    }
}
fn scope_allows_input(context: &ITfContext, ec: u32) -> Result<bool> {
    // 安全：当前 context 的同步编辑锁有效；只读取选择范围的输入范围属性，不读取文本。
    unsafe {
        let property = context.GetAppProperty(&GUID_PROP_INPUTSCOPE)?;
        let mut selection = [TF_SELECTION::default()];
        let mut fetched = 0;
        let selected = context.GetSelection(ec, TF_DEFAULT_SELECTION, &mut selection, &mut fetched);
        let range = selection[0].range.take();
        selected?;
        let Some(range) = range.filter(|_| fetched == 1) else {
            return Ok(false);
        };
        let value = OwnedVariant(property.GetValue(ec, &range)?);
        let inner = &value.0.Anonymous.Anonymous;
        if inner.vt == VT_EMPTY {
            return Ok(true);
        }
        if inner.vt != VT_UNKNOWN {
            return Ok(false);
        }
        let Some(unknown) = inner.Anonymous.punkVal.as_ref() else {
            return Ok(false);
        };
        let scope: ITfInputScope = unknown.cast()?;
        let mut buffer = std::ptr::null_mut();
        let mut count = 0;
        let result = scope.GetInputScopes(&mut buffer, &mut count);
        let allowed = result.is_ok()
            && count <= 1024
            && (count == 0 || !buffer.is_null())
            && (count == 0
                || !std::slice::from_raw_parts(buffer, count as usize).contains(&IS_PASSWORD));
        CoTaskMemFree(Some(buffer.cast()));
        result?;
        Ok(allowed)
    }
}
impl ITfEditSession_Impl for Edit_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        self.host.check()?;
        if self.epoch != self.host.epoch.get()
            || self
                .host
                .pending_layout
                .borrow()
                .as_ref()
                .map(|(id, _)| *id)
                != Some(self.operation)
        {
            return Ok(());
        }
        if self.host.layout_expired() {
            self.host.pending_layout.borrow_mut().take();
            self.host.scheduled.set(false);
            self.host.quarantined.set(true);
            return Err(Error::from_hresult(E_FAIL));
        }
        self.host.pending_layout.borrow_mut().take();
        let result = boundary(|| self.host.run(&self.context, ec, self.epoch));
        if self.epoch == self.host.epoch.get() {
            self.host.scheduled.set(false);
            if result.is_err() {
                self.host.quarantined.set(true);
            }
        }
        result
    }
}
#[implement(ITfCompositionSink, Agile = false)]
struct CompositionObserver {
    host: std::rc::Weak<Host>,
    epoch: u64,
    _lease: super::service::Lease,
}
impl ITfCompositionSink_Impl for CompositionObserver_Impl {
    fn OnCompositionTerminated(
        &self,
        _ec: u32,
        _c: windows_core::Ref<ITfComposition>,
    ) -> Result<()> {
        boundary(|| {
            if let Some(h) = self.host.upgrade() {
                h.check()?;
                if self.epoch == h.epoch.get() && !h.ending.get() {
                    h.composition.borrow_mut().take();
                    h.invalidate()?;
                }
            }
            Ok(())
        })
    }
}
#[implement(ITfEditSession, Agile = false)]
struct Cleanup {
    composition: ITfComposition,
    erase: bool,
    _lease: super::service::Lease,
}
impl ITfEditSession_Impl for Cleanup_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        boundary(|| {
            // 安全：旧 context 授予写锁，只操作此回调独占的旧组合。
            unsafe {
                if self.erase {
                    self.composition.GetRange()?.SetText(ec, 0, &[])?;
                }
                self.composition.EndComposition(ec)
            }
        })
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
pub(crate) mod tests;
