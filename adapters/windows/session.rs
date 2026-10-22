//! 会话与编辑事务；TSF 锁内只操作内存与宿主文本。
use super::{
    boundary,
    candidate::CandidateUi,
    ownership::{Ledger, NativeEnvelope, Outcome, OwnedEvent},
    recovery::RecoveryWindow,
    watchdog::{Detection, Watchdog},
};
use cloudleaf_dictionary::{Dictionary, PROTOTYPE_TSV};
use cloudleaf_input::InputEngine;
use cloudleaf_types::{
    CommitOutcome, EventResult, InputEvent, InputFrame, Key, KeyDisposition, utf8_to_utf16_offset,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
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
static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
pub(crate) type Shared = Rc<Host>;
pub(crate) struct Host {
    _lease: super::service::Lease,
    pub owner: u32,
    pub epoch: Cell<u64>,
    pub engine: RefCell<InputEngine>,
    projection: RefCell<InputEngine>,
    instance: u64,
    native_sequence: Cell<u64>,
    owned: RefCell<Ledger>,
    watchdog: RefCell<Option<Watchdog>>,
    recovery: RefCell<Option<Rc<RecoveryWindow>>>,
    scope_proof: Cell<Option<Instant>>,
    native_scheduled: Cell<bool>,
    native_callback: Cell<u64>,
    active_owned: Cell<Option<u64>>,
    draining_native: Cell<bool>,
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
    text_cookie: Cell<Option<u32>>,
    retry_layout: Cell<bool>,
    unknown_text: Cell<bool>,
    pub quarantined: Cell<bool>,
    pub ending: Cell<bool>,
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop_platform();
    }
}
#[derive(Clone)]
enum Operation {
    Key(Key),
    Select(u64, String),
    Refresh,
    Cancel,
    Owned(u64),
}
struct RestoreFlag<'a> {
    flag: &'a Cell<bool>,
    previous: bool,
}
impl<'a> RestoreFlag<'a> {
    fn set(flag: &'a Cell<bool>, value: bool) -> Self {
        Self {
            flag,
            previous: flag.replace(value),
        }
    }
}
impl Drop for RestoreFlag<'_> {
    fn drop(&mut self) {
        self.flag.set(self.previous);
    }
}
fn same_semantics(actual: &EventResult, planned: &EventResult) -> bool {
    let mut planned = planned.clone();
    planned.frame.revision = actual.frame.revision;
    if let (Some(actual), Some(planned)) = (&actual.commit, &mut planned.commit) {
        planned.id = actual.id;
    }
    actual == &planned
}
impl Host {
    pub fn new() -> Result<Shared> {
        let dictionary =
            Dictionary::from_tsv(PROTOTYPE_TSV).map_err(|_| Error::from_hresult(E_FAIL))?;
        let projection = InputEngine::new(dictionary.clone());
        Ok(Rc::new(Self {
            _lease: super::service::Lease::new(),
            // 安全：只读取线程标识，没有外部资源。
            owner: unsafe { GetCurrentThreadId() },
            epoch: Cell::new(1),
            engine: RefCell::new(InputEngine::new(dictionary)),
            projection: RefCell::new(projection),
            instance: NEXT_INSTANCE
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                    value.checked_add(1)
                })
                .map_err(|_| Error::from_hresult(E_UNEXPECTED))?,
            native_sequence: Cell::new(1),
            owned: RefCell::new(Ledger::new()),
            watchdog: RefCell::new(None),
            recovery: RefCell::new(None),
            scope_proof: Cell::new(None),
            native_scheduled: Cell::new(false),
            native_callback: Cell::new(1),
            active_owned: Cell::new(None),
            draining_native: Cell::new(false),
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
            text_cookie: Cell::new(None),
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
    pub fn start_platform(self: &Shared) -> Result<()> {
        self.check()?;
        super::watchdog::reap();
        super::recovery::reap();
        if self
            .watchdog
            .borrow()
            .as_ref()
            .is_some_and(Watchdog::healthy)
        {
            return Ok(());
        }
        self.stop_platform();
        let window = Rc::new(RecoveryWindow::new(Rc::downgrade(self), self.instance)?);
        let monitor = Watchdog::start().map_err(|error| Error::new(E_FAIL, error.to_string()))?;
        monitor.set_window(window.handle());
        *self.recovery.borrow_mut() = Some(window);
        *self.watchdog.borrow_mut() = Some(monitor);
        Ok(())
    }
    pub fn stop_platform(&self) {
        let monitor = self.watchdog.borrow_mut().take();
        let window = self.recovery.borrow_mut().take();
        if let Some(monitor) = monitor {
            let completed = monitor.completion();
            monitor.stop();
            if let Some(window) = window {
                super::recovery::retire(window, completed);
            }
            drop(monitor);
        } else if let Some(window) = window {
            super::recovery::retire(
                window,
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            );
        }
    }
    fn offer_recovery(&self, reason: &str) -> bool {
        let events = self.owned.borrow().snapshot();
        if events.is_empty() {
            return true;
        }
        let epoch = events[0].native.epoch;
        let material = super::ownership::recovery_material(reason, &events);
        let window = self.recovery.borrow().clone();
        let transferred = window.is_some_and(|window| window.offer(epoch, material));
        if !transferred {
            self.quarantined.set(true);
        }
        transferred
    }
    fn isolate_owned(&self, reason: &str) {
        self.unknown_text.set(true);
        self.native_callback
            .set(self.native_callback.get().saturating_add(1));
        self.native_scheduled.set(false);
        if !self.owned.borrow().is_empty() {
            self.owned.borrow_mut().isolate();
        } else {
            self.owned.borrow_mut().clear();
        }
        self.queue
            .borrow_mut()
            .retain(|operation| !matches!(operation, Operation::Owned(_)));
        self.quarantined.set(true);
        self.scope_proof.set(None);
        self.offer_recovery(reason);
    }
    pub fn watchdog_notice(self: &Shared) -> Result<()> {
        self.check()?;
        let detection = self.watchdog.borrow().as_ref().map(Watchdog::detection);
        if detection.is_some_and(|state| state != Detection::Healthy) {
            if detection == Some(Detection::Unknown) {
                self.unknown_text.set(true);
            }
            self.isolate_owned("独立监测已检测超时或监测故障；宿主恢复须由 STA 完成");
        }
        Ok(())
    }
    pub fn discard_recovery(self: &Shared) -> Result<()> {
        self.check()?;
        self.owned.borrow_mut().clear();
        self.unknown_text.set(true);
        let context = self.context.borrow().clone();
        self.invalidate()?;
        self.focus(context)
    }
    fn scope_current(&self) -> bool {
        self.scope_proof
            .get()
            .is_some_and(|proof| proof.elapsed() < Duration::from_millis(100))
    }
    pub fn native_probe(&self, key: Option<Key>) -> Result<bool> {
        self.check()?;
        if self.quarantined.get() {
            return Ok(key == Some(Key::Escape));
        }
        if !self.owned.borrow().is_empty() {
            return Ok(true);
        }
        let Some(key) = key else {
            return Ok(false);
        };
        if self
            .watchdog
            .borrow()
            .as_ref()
            .is_some_and(Watchdog::healthy)
            && self.scope_current()
        {
            return Ok(self.owned.borrow().len() < QUEUE_LIMIT
                && matches!(
                    self.projection.borrow().key_disposition(key),
                    KeyDisposition::Handle
                ));
        }
        self.disposition(key)
    }
    pub fn native_key(
        self: &Shared,
        w: windows::Win32::Foundation::WPARAM,
        l: windows::Win32::Foundation::LPARAM,
    ) -> Result<bool> {
        self.check()?;
        self.watchdog_notice()?;
        let key = super::keys::normalize(w, l);
        if self.quarantined.get() {
            if key == Some(Key::Escape) {
                self.discard_recovery()?;
                return Ok(true);
            }
            return Ok(false);
        }
        if self.owned.borrow().front().is_some_and(|event| {
            event.native.epoch != self.epoch.get()
                || !matches!(
                    event.outcome,
                    Outcome::Queued | Outcome::Started | Outcome::Writing
                )
        }) {
            self.isolate_owned("旧会话材料尚未交付恢复窗口，新事件未被接管");
            return Ok(false);
        }
        let owns = key.is_some_and(|key| {
            matches!(
                self.projection.borrow().key_disposition(key),
                KeyDisposition::Handle
            )
        });
        if !owns {
            self.native_barrier()?;
            // 原生命令可能改变选区/旧range。先结束自己的组合，绝不擦除宿主新增字符。
            if self.composition.borrow().is_some() {
                let context = self.context.borrow().clone();
                self.unknown_text.set(true);
                self.invalidate()?;
                self.focus(context)?;
            }
            return Ok(false);
        }
        if self
            .recovery
            .borrow()
            .as_ref()
            .is_some_and(|window| !window.can_offer())
        {
            self.isolate_owned("恢复材料达到容量上限，新事件未被接管");
            return Ok(false);
        }
        if !self
            .watchdog
            .borrow()
            .as_ref()
            .is_some_and(Watchdog::healthy)
            || !self.scope_current()
        {
            self.native_barrier()?;
            let handled = self.key(key.unwrap())?;
            return Ok(handled);
        }
        if self.owned.borrow().len() >= QUEUE_LIMIT || self.queue.borrow().len() >= QUEUE_LIMIT {
            self.isolate_owned("接管队列已满，新原生事件未被接管");
            return Ok(false);
        }
        let sequence = self.native_sequence.get();
        let next = sequence
            .checked_add(1)
            .ok_or_else(|| Error::from_hresult(E_UNEXPECTED))?;
        let native = NativeEnvelope::capture(
            self.instance,
            self.epoch.get(),
            sequence,
            key.unwrap(),
            w,
            l,
        );
        if !self.owned.borrow().can_accept(&native) {
            self.isolate_owned("原生信封未通过入队验证，新事件未被接管");
            return Ok(false);
        }
        // 此处是实际 OnKeyDown 的 ownership 决策；之后才修改投影引擎。
        self.native_sequence.set(next);
        let planned = self
            .projection
            .borrow_mut()
            .handle(InputEvent::Key(native.key));
        if let Some(commit) = &planned.commit {
            self.projection
                .borrow_mut()
                .handle(InputEvent::CommitSucceeded { id: commit.id });
        }
        let event = OwnedEvent {
            native,
            planned,
            outcome: Outcome::Queued,
            admitted: Instant::now(),
        };
        self.owned
            .borrow_mut()
            .push(event)
            .map_err(|_| Error::from_hresult(E_FAIL))?;
        self.queue
            .borrow_mut()
            .push_back(Operation::Owned(sequence));
        if let Err(_error) = self.schedule_native() {
            self.isolate_owned("异步编辑会话申请失败，已接管输入保留供恢复");
        }
        Ok(true)
    }
    fn native_barrier(self: &Shared) -> Result<()> {
        if self.owned.borrow().is_empty() {
            return Ok(());
        }
        let context = self
            .context
            .borrow()
            .clone()
            .ok_or_else(|| Error::from_hresult(E_FAIL))?;
        let called = Rc::new(Cell::new(false));
        let edit: ITfEditSession = NativeDrain {
            host: self.clone(),
            context: context.clone(),
            epoch: self.epoch.get(),
            callback: self.native_callback.get(),
            called: called.clone(),
        }
        .into();
        // 安全：同步锁优先于 TSF 已排异步锁，因此显式按 FIFO 排空全部先前所有权。
        let result = unsafe {
            context.RequestEditSession(self.client.get(), &edit, TF_ES_READWRITE | TF_ES_SYNC)
        }
        .and_then(|hr| hr.ok());
        called.set(true);
        if result.is_err() || !self.owned.borrow().is_empty() {
            self.isolate_owned("原生命令 barrier 未完成，旧输入隔离保全；原命令不被文本化");
        }
        Ok(())
    }
    fn schedule_native(self: &Shared) -> Result<()> {
        if self.native_scheduled.replace(true) {
            return Ok(());
        }
        let context = self
            .context
            .borrow()
            .clone()
            .ok_or_else(|| Error::from_hresult(E_FAIL))?;
        let event = self
            .owned
            .borrow()
            .snapshot()
            .first()
            .cloned()
            .ok_or_else(|| Error::from_hresult(E_FAIL))?;
        let monitor = self.watchdog.borrow();
        if !monitor.as_ref().is_some_and(|monitor| {
            monitor.begin(
                event.native.sequence,
                Duration::from_millis(500).saturating_sub(event.admitted.elapsed()),
            )
        }) {
            self.native_scheduled.set(false);
            return Err(Error::from_hresult(E_FAIL));
        }
        drop(monitor);
        let edit: ITfEditSession = NativeDrain {
            host: self.clone(),
            context: context.clone(),
            epoch: self.epoch.get(),
            callback: self.native_callback.get(),
            called: Rc::new(Cell::new(false)),
        }
        .into();
        // 安全：原始消息已取得 ownership，回调拥有 Host，所有文字仅留在 STA 的有界 ledger。
        unsafe {
            context
                .RequestEditSession(
                    self.client.get(),
                    &edit,
                    TF_ES_READWRITE | TF_ES_ASYNCDONTCARE,
                )
                .and_then(|hr| hr.ok())
        }
    }
    pub fn disposition(&self, key: Key) -> Result<bool> {
        self.check()?;
        if self.context.borrow().is_none() {
            return Ok(false);
        }
        if self.quarantined.get() || self.layout_expired() {
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
        let previous_epoch = self.epoch.get();
        self.invalidate()?;
        let epoch = previous_epoch
            .checked_add(1)
            .ok_or_else(|| Error::from_hresult(E_UNEXPECTED))?;
        if self.epoch.get() != epoch {
            return Err(Error::from_hresult(windows::Win32::Foundation::E_ABORT));
        }
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
                    if self.epoch.get() == epoch {
                        self.context.borrow_mut().take();
                    }
                    return Err(error);
                }
            };
            if self.epoch.get() != epoch {
                // 安全：订阅期间重入切换焦点，只撤销本次旧 context 的 cookie。
                unsafe {
                    let _ = context
                        .cast::<ITfSource>()
                        .and_then(|source| source.UnadviseSink(cookie));
                }
                return Err(Error::from_hresult(windows::Win32::Foundation::E_ABORT));
            }
            self.layout_cookie.set(Some(cookie));
            let observer: ITfTextEditSink = TextObserver {
                host: Rc::downgrade(self),
                epoch,
                _lease: super::service::Lease::new(),
            }
            .into();
            // 安全：同 context 的文本/选区/属性编辑通知仅在所属 STA 内接收。
            let text_cookie = unsafe {
                context
                    .cast::<ITfSource>()?
                    .AdviseSink(&ITfTextEditSink::IID, &observer)
            };
            match text_cookie {
                Ok(cookie) if self.epoch.get() == epoch => self.text_cookie.set(Some(cookie)),
                Ok(cookie) => {
                    unsafe {
                        let _ = context
                            .cast::<ITfSource>()
                            .and_then(|source| source.UnadviseSink(cookie));
                    }
                    return Err(Error::from_hresult(E_FAIL));
                }
                Err(error) => {
                    self.invalidate()?;
                    return Err(error);
                }
            }
            self.engine.borrow_mut().handle(InputEvent::Activate);
            self.projection.borrow_mut().handle(InputEvent::Activate);
        }
        Ok(())
    }
    pub fn invalidate(self: &Shared) -> Result<()> {
        self.invalidate_mode(true)
    }
    fn invalidate_mode(self: &Shared, synchronous_cleanup: bool) -> Result<()> {
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
        self.projection.borrow_mut().handle(InputEvent::FocusLost);
        self.scope_proof.set(None);
        self.native_scheduled.set(false);
        self.native_callback.set(
            self.native_callback
                .get()
                .checked_add(1)
                .ok_or_else(|| Error::from_hresult(E_UNEXPECTED))?,
        );
        if !self.owned.borrow().is_empty() {
            self.owned.borrow_mut().isolate();
        } else {
            self.owned.borrow_mut().clear();
        }
        if let Some(sequence) = self.active_owned.take()
            && let Some(monitor) = self.watchdog.borrow().as_ref()
        {
            monitor.cancel(sequence);
        }
        if !self.owned.borrow().is_empty()
            && self.offer_recovery("焦点或生命周期改变，未完成输入未自动移交其他文档")
        {
            // 材料已由独立窗口取得所有权；没有窗口或容量失败时保留 ledger 并拒绝新接管。
            self.owned.borrow_mut().clear();
        }
        let erase = !self.unknown_text.replace(false);
        self.quarantined.set(false);
        let ui = self.ui.borrow_mut().take();
        let composition = self.composition.borrow_mut().take();
        let context = self.context.borrow_mut().take();
        let cookie = self.layout_cookie.take();
        let text_cookie = self.text_cookie.take();
        let client = self.client.get();
        self.retry_layout.set(false);
        // 所有旧资源先一次性脱离 Host；清理外调即使重入，也不会取走新 epoch 的资源。
        if let Some(mut ui) = ui {
            ui.close();
        }
        let mut failure = None;
        if let (Some(context), Some(cookie)) = (&context, text_cookie) {
            // 安全：文本 sink 与布局 sink 独立撤销，任何失败不跳过其他旧资源。
            unsafe {
                if let Err(error) = context
                    .cast::<ITfSource>()
                    .and_then(|source| source.UnadviseSink(cookie))
                {
                    failure = Some(error);
                }
            }
        }
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
            let executed = Rc::new(Cell::new(false));
            let cleanup: ITfEditSession = Cleanup {
                composition: composition.clone(),
                erase,
                called: executed.clone(),
                deadline: None,
                _lease: super::service::Lease::new(),
            }
            .into();
            // 安全：清理只持有旧组合，独立于新会话队列；迟到回调不能写新文档。
            unsafe {
                let sync = if synchronous_cleanup {
                    context
                        .RequestEditSession(client, &cleanup, TF_ES_READWRITE | TF_ES_SYNC)
                        .and_then(|hr| hr.ok())
                } else {
                    Err(Error::from_hresult(TS_E_SYNCHRONOUS))
                };
                let ran = executed.replace(true);
                let result = if ran {
                    sync
                } else {
                    let safe: ITfEditSession = Cleanup {
                        composition,
                        erase: false,
                        called: Rc::new(Cell::new(false)),
                        deadline: Some(Instant::now() + Duration::from_millis(500)),
                        _lease: super::service::Lease::new(),
                    }
                    .into();
                    context
                        .RequestEditSession(client, &safe, TF_ES_READWRITE | TF_ES_ASYNCDONTCARE)
                        .and_then(|hr| hr.ok())
                };
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
        let called = Rc::new(Cell::new(false));
        let epoch = self.epoch.get();
        let edit: ITfEditSession = KeyEdit {
            host: self.clone(),
            context: context.clone(),
            epoch,
            action: Operation::Key(key),
            executed: executed.clone(),
            called: called.clone(),
        }
        .into();
        self.scheduled.set(true);
        // 安全：按键只允许同步写锁；回调运行之前不改核心或队列，拒锁原生键透传。
        let requested = unsafe {
            context.RequestEditSession(self.client.get(), &edit, TF_ES_READWRITE | TF_ES_SYNC)
        };
        // 同步申请返回后封闭回调，即使违约宿主保留对象也不能迟到处理已透传原生键。
        called.set(true);
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
        self.native_barrier()?;
        if self.quarantined.get() || self.scheduled.get() || !self.queue.borrow().is_empty() {
            return Err(Error::from_hresult(E_FAIL));
        }
        self.sync_operation(Operation::Select(revision, id))
    }
    pub fn cancel(self: &Shared) -> Result<()> {
        self.check()?;
        self.native_barrier()?;
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
        let called = Rc::new(Cell::new(false));
        let edit: ITfEditSession = KeyEdit {
            host: self.clone(),
            epoch,
            context: context.clone(),
            action,
            executed: executed.clone(),
            called: called.clone(),
        }
        .into();
        self.scheduled.set(true);
        // 安全：候选和独立取消不允许延迟写；未授锁不会改变核心。
        let result = unsafe {
            context.RequestEditSession(self.client.get(), &edit, TF_ES_READWRITE | TF_ES_SYNC)
        }
        .and_then(|hr| hr.ok());
        called.set(true);
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
            if matches!(self.queue.borrow().front(), Some(Operation::Owned(_)))
                && !self.draining_native.get()
            {
                break;
            }
            let Some(op) = self.queue.borrow_mut().pop_front() else {
                break;
            };
            let projected_action = match &op {
                Operation::Key(key) => Some(InputEvent::Key(*key)),
                Operation::Cancel => Some(InputEvent::Key(Key::Escape)),
                _ => None,
            };
            let projected_selection = match &op {
                Operation::Select(revision, id) => Some((*revision, id.clone())),
                _ => None,
            };
            let owned_operation = matches!(op, Operation::Owned(_));
            let result = match op {
                Operation::Owned(sequence) => {
                    if !scope_allows_input(context, ec)? {
                        self.isolate_owned("执行前输入scope失效，暂停并保留队首事件");
                        return Err(Error::from_hresult(E_FAIL));
                    }
                    if epoch != self.epoch.get() || self.context.borrow().as_ref() != Some(context)
                    {
                        return Ok(());
                    }
                    let record = self.owned.borrow_mut().start(sequence)?;
                    if record.native.instance != self.instance || record.native.epoch != epoch {
                        self.isolate_owned("所有权身份不匹配，拒绝旧会话写入");
                        return Err(Error::from_hresult(E_UNEXPECTED));
                    }
                    let started = self
                        .watchdog
                        .borrow()
                        .as_ref()
                        .is_some_and(|monitor| monitor.start_operation(sequence));
                    if !started {
                        self.isolate_owned("队首编辑事务已超时或失效");
                        return Err(Error::from_hresult(E_FAIL));
                    }
                    self.active_owned.set(Some(sequence));
                    let result = self
                        .engine
                        .borrow_mut()
                        .handle(InputEvent::Key(record.native.key));
                    if !result.handled || !same_semantics(&result, &record.planned) {
                        self.isolate_owned("执行引擎与投影不一致，已接管原事件未被静默丢弃");
                        return Err(Error::from_hresult(E_FAIL));
                    }
                    result
                }
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
                return Err(Error::from_hresult(
                    windows::Win32::Foundation::E_INVALIDARG,
                ));
            }
            if !owned_operation {
                let projected = if let Some(event) = projected_action {
                    Some(self.projection.borrow_mut().handle(event))
                } else {
                    projected_selection.map(|(_, id)| {
                        let revision = self.projection.borrow().frame().revision;
                        self.projection.borrow_mut().select_candidate(revision, &id)
                    })
                };
                // 投影只模拟未来接管语义，不发布帧、不学习、不确认宿主写入。
                if let Some(projected) = projected {
                    // 旧隔离可令两个命名空间计数不同，但任何语义差异仍须在写入前隔离。
                    if !same_semantics(&result, &projected) {
                        self.isolate_owned("同步事务投影语义不一致，禁止写入目标文档");
                        return Err(Error::from_hresult(E_FAIL));
                    }
                    if let Some(commit) = projected.commit {
                        self.projection
                            .borrow_mut()
                            .handle(InputEvent::CommitSucceeded { id: commit.id });
                    }
                }
            }
            let outcome = self.render(context, ec, &result);
            // 外部 COM 可重入焦点通知；旧事务不能确认新会话或继续消费新队列。
            if epoch != self.epoch.get() {
                return Ok(());
            }
            if let Some(sequence) = self.active_owned.get() {
                if outcome.is_ok()
                    && !self
                        .watchdog
                        .borrow()
                        .as_ref()
                        .is_some_and(|monitor| monitor.finish(sequence))
                {
                    self.unknown_text.set(true);
                    self.isolate_owned("独立检测已使事务失效，不能在返回后伪造 Applied");
                    return Err(Error::from_hresult(E_FAIL));
                }
                if let Err((classification, _)) = &outcome {
                    let _ = self.owned.borrow_mut().fail(
                        sequence,
                        if *classification == CommitOutcome::NotApplied {
                            Outcome::NotApplied
                        } else {
                            Outcome::Unknown
                        },
                    );
                }
            }
            if outcome.is_ok() {
                self.unknown_text.set(false);
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
                if self.active_owned.get().is_some() {
                    self.isolate_owned("已接管事件写入失败，恢复材料保留");
                }
                return Err(e);
            }
            if let Some(sequence) = self.active_owned.take() {
                self.owned.borrow_mut().applied(sequence)?;
                let next = self.owned.borrow().front().cloned();
                if let Some(next) = next
                    && !self.watchdog.borrow().as_ref().is_some_and(|monitor| {
                        monitor.begin(
                            next.native.sequence,
                            Duration::from_millis(500).saturating_sub(next.admitted.elapsed()),
                        )
                    })
                {
                    self.isolate_owned("下一条所有权事件已过期或监测失效");
                    return Err(Error::from_hresult(E_FAIL));
                }
            }
            let frame = self.engine.borrow().frame();
            self.update_ui(context, ec, frame)?;
        }
        if epoch == self.epoch.get() {
            self.scheduled.set(false);
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
            if self.active_owned.get().is_some()
                && !self
                    .watchdog
                    .borrow()
                    .as_ref()
                    .is_some_and(Watchdog::healthy)
            {
                return Err(Error::from_hresult(E_FAIL));
            }
            Ok(())
        };
        let mut begin_write = || -> Result<()> {
            // 空文本删除与普通写入共用不确定性边界，失败后旧 range 绝不自动擦除。
            attempted = true;
            self.unknown_text.set(true);
            if let Some(sequence) = self.active_owned.get() {
                self.owned.borrow_mut().writing(sequence)?;
                if !self
                    .watchdog
                    .borrow()
                    .as_ref()
                    .is_some_and(|monitor| monitor.writing(sequence))
                {
                    return Err(Error::from_hresult(E_FAIL));
                }
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
                    if let Some(c) = composition {
                        begin_write()?;
                        range.SetText(ec, 0, &[])?;
                        current()?;
                        // 成功删除后仍须证明结束组合成功；失败保留组合供无擦除清理。
                        self.composition.borrow_mut().take();
                        let _ending = RestoreFlag::set(&self.ending, true);
                        let ended = c.EndComposition(ec);
                        if ended.is_err()
                            && epoch == self.epoch.get()
                            && self.context.borrow().as_ref() == Some(context)
                        {
                            *self.composition.borrow_mut() = Some(c);
                        }
                        ended?;
                        current()?;
                    }
                    return Ok(());
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
                begin_write()?;
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
            let layout = (|| -> Result<()> {
                unsafe {
                    let view = context.GetActiveView()?;
                    current()?;
                    let range = c.GetRange()?;
                    current()?;
                    view.GetTextExt(ec, &range, &mut rect, &mut clipped)
                }
            })();
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
#[implement(ITfTextEditSink, Agile = false)]
struct TextObserver {
    host: std::rc::Weak<Host>,
    epoch: u64,
    _lease: super::service::Lease,
}
impl ITfTextEditSink_Impl for TextObserver_Impl {
    fn OnEndEdit(
        &self,
        context: windows_core::Ref<ITfContext>,
        _ec: u32,
        _record: windows_core::Ref<ITfEditRecord>,
    ) -> Result<()> {
        boundary(|| {
            let Some(host) = self.host.upgrade() else {
                return Ok(());
            };
            host.check()?;
            if self.epoch != host.epoch.get() || context.as_ref() != host.context.borrow().as_ref()
            {
                return Ok(());
            }
            host.scope_proof.set(None);
            // 安全：官方接口在通知内按 client 识别刚完成的写会话；失败不得误认自身编辑。
            let own = unsafe { context.as_ref().unwrap().InWriteSession(host.client.get()) };
            if self.epoch != host.epoch.get() || context.as_ref() != host.context.borrow().as_ref()
            {
                return Ok(());
            }
            if matches!(own, Ok(value) if value.as_bool()) {
                return Ok(());
            }
            // 不读取外部正文；任何非本事务的文本/选区/属性编辑都结束旧组合且禁止擦范围。
            host.unknown_text.set(true);
            host.isolate_owned("目标应用修改了文本、选区或属性，旧组合不再安全可写");
            host.invalidate_mode(false)?;
            Ok(())
        })
    }
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
struct NativeDrain {
    host: Shared,
    context: ITfContext,
    epoch: u64,
    callback: u64,
    called: Rc<Cell<bool>>,
}
impl ITfEditSession_Impl for NativeDrain_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        boundary(|| {
            self.host.check()?;
            if self.called.replace(true)
                || self.epoch != self.host.epoch.get()
                || self.callback != self.host.native_callback.get()
                || self.host.context.borrow().as_ref() != Some(&self.context)
            {
                return Ok(());
            }
            // 安全：仅在授予的写锁中重新检查 readonly 和输入 scope。
            let allowed = unsafe { self.context.GetStatus()?.dwDynamicFlags & TS_SD_READONLY == 0 }
                && scope_allows_input(&self.context, ec)?;
            if !allowed {
                self.host
                    .isolate_owned("目标输入范围已改变，不向 readonly/password 区域写入");
                return Err(Error::from_hresult(E_FAIL));
            }
            self.host.scope_proof.set(Some(Instant::now()));
            let _draining = RestoreFlag::set(&self.host.draining_native, true);
            let result = self.host.run(&self.context, ec, self.epoch);
            if self.epoch == self.host.epoch.get() {
                self.host.native_scheduled.set(false);
                self.host.native_callback.set(
                    self.host
                        .native_callback
                        .get()
                        .checked_add(1)
                        .ok_or_else(|| Error::from_hresult(E_UNEXPECTED))?,
                );
                if result.is_err() {
                    self.host
                        .isolate_owned("异步事务失败或结果未知，未完成事件仅供用户核对恢复");
                }
            }
            result
        })
    }
}
#[implement(ITfEditSession, Agile = false)]
struct KeyEdit {
    host: Shared,
    epoch: u64,
    context: ITfContext,
    action: Operation,
    executed: Rc<Cell<bool>>,
    called: Rc<Cell<bool>>,
}
impl ITfEditSession_Impl for KeyEdit_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        boundary(|| {
            self.host.check()?;
            if self.called.replace(true) {
                return Ok(());
            }
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
            self.host.scope_proof.set(Some(Instant::now()));
            self.executed.set(true);
            self.host.queue.borrow_mut().push_back(self.action.clone());
            let result = self.host.run(&self.context, ec, self.epoch);
            if result
                .as_ref()
                .is_err_and(|error| error.code() != windows::Win32::Foundation::E_INVALIDARG)
                && self.epoch == self.host.epoch.get()
            {
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
    called: Rc<Cell<bool>>,
    deadline: Option<Instant>,
    _lease: super::service::Lease,
}
impl ITfEditSession_Impl for Cleanup_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        boundary(|| {
            if self.called.replace(true) {
                return Ok(());
            }
            if self
                .deadline
                .is_some_and(|deadline| Instant::now() > deadline)
            {
                return Err(Error::from_hresult(E_FAIL));
            }
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

#[cfg(test)]
#[path = "render_tests.rs"]
mod render_tests;
