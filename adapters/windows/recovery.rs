//! 恢复材料与 HWND 分离；broker 只传无文本通知，全部 HWND 操作留在创建 STA。
use super::{boundary, budget, service::Lease, session::Host, watchdog};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicIsize, AtomicU8, AtomicU64, Ordering},
    },
};
use windows::Win32::{
    Foundation::*,
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::{Controls::EM_SETSEL, Input::KeyboardAndMouse::SetFocus, WindowsAndMessaging::*},
};
use windows_core::{Error, PCWSTR, Result};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}
const CLEANUP: u32 = WM_APP + 0x3a2;
const REPAIR: u32 = WM_APP + 0x3a3;
const MATERIAL_LIMIT: usize = 1024 * 1024;
const CONTEXT_LIMIT: usize = 32;
const NOTICE_ATTEMPTS: u8 = 4;
static NEXT_WINDOW: AtomicU64 = AtomicU64::new(1);
static ENDPOINTS: OnceLock<Mutex<Vec<Arc<Endpoint>>>> = OnceLock::new();
static MATERIALS: OnceLock<Mutex<Vec<Arc<MaterialStore>>>> = OnceLock::new();
#[cfg(test)]
thread_local! { static REGISTER_FAILURE: Cell<Option<usize>> = const { Cell::new(None) }; }
#[cfg(test)]
thread_local! { static DROP_FAILURE: Cell<u8> = const { Cell::new(0) }; }
#[cfg(test)]
thread_local! { static CREATE_FAILURE: Cell<bool> = const { Cell::new(false) }; }

/// 只包含 Send 数据，绝不携带 apartment COM 或 Rc。
struct Endpoint {
    owner: u32,
    broker: AtomicIsize,
    pending: AtomicBool,
    attempts: AtomicU8,
    failed: AtomicBool,
    #[cfg(test)]
    fail_delivery: AtomicBool,
    _lease: Lease,
}
struct RecoveryBatch {
    epoch: u64,
    text: String,
    _reservation: Arc<budget::Reservation>,
}
struct MaterialStore {
    instance: u64,
    batches: Mutex<Vec<RecoveryBatch>>,
    discarded_through: AtomicU64,
    viewer: AtomicU64,
}
struct MaterialSnapshot {
    text: String,
    reservations: Vec<Arc<budget::Reservation>>,
}
impl MaterialStore {
    fn has_material(&self) -> bool {
        self.batches
            .lock()
            .map_or(true, |batches| !batches.is_empty())
    }
    #[cfg(test)]
    fn text(&self) -> Result<String> {
        Ok(self.snapshot()?.text)
    }
    fn snapshot(&self) -> Result<MaterialSnapshot> {
        let batches = self
            .batches
            .lock()
            .map_err(|_| Error::from_hresult(E_FAIL))?;
        let text = batches
            .iter()
            .map(|batch| {
                format!(
                    "instance={} 上下文 epoch={}\r\n{}",
                    self.instance, batch.epoch, batch.text
                )
            })
            .collect::<Vec<_>>()
            .join("\r\n---\r\n");
        let reservations = batches
            .iter()
            .map(|batch| batch._reservation.clone())
            .collect();
        Ok(MaterialSnapshot { text, reservations })
    }
    fn clear(&self) -> Result<Option<u64>> {
        let mut batches = self
            .batches
            .lock()
            .map_err(|_| Error::from_hresult(E_FAIL))?;
        let last = batches.last().map(|batch| batch.epoch);
        if let Some(last) = last {
            self.discarded_through.fetch_max(last, Ordering::AcqRel);
        }
        batches.clear();
        drop(batches);
        if let Some(materials) = MATERIALS.get() {
            materials
                .lock()
                .map_err(|_| Error::from_hresult(E_FAIL))?
                .retain(|store| !std::ptr::eq(store.as_ref(), self));
        }
        Ok(last)
    }
}
struct View {
    host: Weak<Host>,
    owner: u32,
    window: Cell<HWND>,
    broker: Cell<HWND>,
    store: Arc<MaterialStore>,
    edit: Cell<HWND>,
    class: Vec<u16>,
    broker_class: Vec<u16>,
    instance: HINSTANCE,
    endpoint: Arc<Endpoint>,
    generation: u64,
    shown: RefCell<Vec<DisplayedStore>>,
    main_registered: Cell<bool>,
    broker_registered: Cell<bool>,
    _reservation: Arc<budget::Reservation>,
    _lease: Lease,
}
pub(crate) struct RecoveryWindow {
    view: Rc<View>,
}
struct DisplayedStore {
    store: Arc<MaterialStore>,
    _reservations: Vec<Arc<budget::Reservation>>,
}
struct OwnerRetired {
    windows: RefCell<Vec<(Rc<RecoveryWindow>, Arc<AtomicBool>)>>,
}
impl Drop for OwnerRetired {
    fn drop(&mut self) {
        // 线程退出不等于已注销窗口类；不可由 TLS 析构丢弃材料或最后的模块租约。
        for item in self.windows.get_mut().drain(..) {
            std::mem::forget(item);
        }
    }
}
thread_local! { static RETIRED: OwnerRetired = const { OwnerRetired { windows: RefCell::new(Vec::new()) } }; }
pub(crate) fn retire(window: Rc<RecoveryWindow>, completion: Arc<AtomicBool>) {
    let mut item = Some((window, completion));
    if RETIRED
        .try_with(|retired| retired.windows.borrow_mut().push(item.take().unwrap()))
        .is_err()
    {
        // owner TLS 已不可用，保守保活而不从退出线程假称完成资源清理。
        std::mem::forget(item);
    }
}
pub(crate) fn reap() {
    let _ = RETIRED.try_with(|retired| {
        let previous = std::mem::take(&mut *retired.windows.borrow_mut());
        let (remaining, released): (Vec<_>, Vec<_>) =
            previous.into_iter().partition(|(window, complete)| {
                !complete.load(Ordering::Acquire) || window.has_material()
            });
        *retired.windows.borrow_mut() = remaining;
        // DestroyWindow 会重入窗口过程，不跨外调持有 TLS 借用。
        drop(released);
    });
}
/// 任意线程只发无文本消息，不执行 apartment 清理，不等待 owner。
pub(crate) fn request_owner_cleanup() {
    reap();
    let Some(endpoints) = ENDPOINTS.get() else {
        return;
    };
    let Ok(endpoints) = endpoints.try_lock().map(|endpoints| endpoints.clone()) else {
        return;
    };
    for endpoint in &endpoints {
        // owner 身份只供路由/诊断，任何线程均不借此越过 STA 执行清理。
        if endpoint.owner == 0 {
            continue;
        }
        let broker = endpoint.broker.load(Ordering::Acquire);
        if broker == 0 {
            endpoint.failed.store(true, Ordering::Release);
            continue;
        }
        if endpoint.pending.swap(true, Ordering::AcqRel) {
            continue;
        }
        if endpoint
            .attempts
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                (value < NOTICE_ATTEMPTS).then_some(value + 1)
            })
            .is_err()
        {
            endpoint.pending.store(false, Ordering::Release);
            continue;
        }
        // 安全：消息无文本，目标窗口只由其 STA 解释；发送成功不视为处理成功。
        #[cfg(test)]
        if endpoint.fail_delivery.load(Ordering::Acquire) {
            endpoint.failed.store(true, Ordering::Release);
            endpoint.pending.store(false, Ordering::Release);
            continue;
        }
        if unsafe { PostMessageW(Some(HWND(broker as *mut _)), CLEANUP, WPARAM(0), LPARAM(0)) }
            .is_err()
        {
            endpoint.failed.store(true, Ordering::Release);
            endpoint.pending.store(false, Ordering::Release);
        }
    }
}
pub(crate) fn fixed_cost() -> usize {
    std::mem::size_of::<RecoveryWindow>()
        + std::mem::size_of::<View>()
        + std::mem::size_of::<Endpoint>()
        + std::mem::size_of::<MaterialStore>()
        + CONTEXT_LIMIT * std::mem::size_of::<RecoveryBatch>()
        + 4096
}
impl RecoveryWindow {
    pub fn new(host: Weak<Host>, identity: u64) -> Result<Self> {
        let reservation = host
            .upgrade()
            .map(|host| host.recovery_reservation())
            .or_else(|| budget::reserve_host(fixed_cost()).map(Arc::new))
            .ok_or_else(|| Error::new(E_FAIL, "模块恢复对象预算已满"))?;
        let generation = NEXT_WINDOW
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| Error::from_hresult(E_FAIL))?;
        // 安全：只读取当前线程与进程模块身份，所有窗口均由当前 STA 创建。
        let owner = unsafe { GetCurrentThreadId() };
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None)? }.0);
        let endpoint = Arc::new(Endpoint {
            owner,
            broker: AtomicIsize::new(0),
            pending: AtomicBool::new(false),
            attempts: AtomicU8::new(0),
            failed: AtomicBool::new(false),
            #[cfg(test)]
            fail_delivery: AtomicBool::new(false),
            _lease: Lease::new(),
        });
        let view = Rc::new(View {
            host,
            owner,
            window: Cell::new(HWND::default()),
            broker: Cell::new(HWND::default()),
            store: Arc::new(MaterialStore {
                instance: identity,
                batches: Mutex::new(Vec::with_capacity(CONTEXT_LIMIT)),
                discarded_through: AtomicU64::new(0),
                viewer: AtomicU64::new(0),
            }),
            edit: Cell::new(HWND::default()),
            class: wide(&format!("CloudleafRecovery_{identity}_{generation}")),
            broker_class: wide(&format!("CloudleafRecoveryBroker_{identity}_{generation}")),
            instance,
            endpoint,
            generation,
            shown: RefCell::new(Vec::new()),
            main_registered: Cell::new(false),
            broker_registered: Cell::new(false),
            _reservation: reservation,
            _lease: Lease::new(),
        });
        let window = Self { view };
        ENDPOINTS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .map_err(|_| Error::from_hresult(E_FAIL))?
            .push(window.view.endpoint.clone());
        for (index, (class, procedure, registered)) in [
            (
                &window.view.class,
                main_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
                &window.view.main_registered,
            ),
            (
                &window.view.broker_class,
                broker_proc,
                &window.view.broker_registered,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            #[cfg(test)]
            if REGISTER_FAILURE.with(|fault| fault.get() == Some(index)) {
                return Err(Error::new(E_FAIL, "故障注入：注册恢复窗口类失败"));
            }
            let _ = index;
            let wc = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance,
                lpszClassName: PCWSTR(class.as_ptr()),
                ..Default::default()
            };
            // 安全：类名与 View 保持至同 STA 注销完成。
            if unsafe { RegisterClassW(&wc) } == 0 {
                return Err(Error::from_thread());
            }
            registered.set(true);
        }
        window.ensure_broker()?;
        window.ensure_main()?;
        if MATERIALS.get().is_some_and(|materials| {
            materials
                .lock()
                .map_or(true, |materials| !materials.is_empty())
        }) {
            // 旧 owner 已退出时，新 STA 仍显示无原文入口，用户点击后才取全局未解决材料。
            unsafe {
                let _ = ShowWindow(window.handle(), SW_SHOWNOACTIVATE);
            }
        }
        Ok(window)
    }
    fn check_owner(&self) -> Result<()> {
        // 安全：仅比较线程身份；错误线程不读取 Rc 所拥有的 HWND 数据。
        if unsafe { GetCurrentThreadId() } == self.view.owner {
            Ok(())
        } else {
            Err(Error::from_hresult(E_UNEXPECTED))
        }
    }
    fn ensure_broker(&self) -> Result<()> {
        self.check_owner()?;
        if !self.view.broker.get().is_invalid() {
            return Ok(());
        }
        // 安全：独立入口仅展示说明与按钮，绝不自动显示恢复原文。
        let window = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                PCWSTR(self.view.broker_class.as_ptr()),
                PCWSTR(wide("云笺：恢复入口").as_ptr()),
                WS_OVERLAPPED | WS_CAPTION,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                400,
                170,
                None,
                None,
                Some(self.view.instance),
                Some(Rc::as_ptr(&self.view).cast()),
            )?
        };
        self.view.broker.set(window);
        self.view
            .endpoint
            .broker
            .store(window.0 as isize, Ordering::Release);
        Ok(())
    }
    fn ensure_main(&self) -> Result<()> {
        ensure_main(&self.view)
    }
    pub fn handle(&self) -> HWND {
        self.view.broker.get()
    }
    #[cfg(test)]
    pub fn evidence(&self) -> String {
        self.view.store.text().unwrap()
    }
    #[cfg(test)]
    pub fn explicit_all_for_review(&self) -> String {
        unsafe {
            SendMessageW(self.handle(), WM_COMMAND, Some(WPARAM(4)), Some(LPARAM(0)));
            let mut text = vec![0u16; GetWindowTextLengthW(self.view.edit.get()) as usize + 1];
            let length = GetWindowTextW(self.view.edit.get(), &mut text);
            String::from_utf16(&text[..length as usize]).unwrap()
        }
    }
    pub fn repair(&self) -> Result<HWND> {
        self.ensure_broker()?;
        Ok(self.handle())
    }
    pub fn has_material(&self) -> bool {
        self.view.store.has_material()
    }
    pub fn discard_epoch(&self, epoch: u64) -> Result<()> {
        release_display(&self.view)?;
        let mut batches = self
            .view
            .store
            .batches
            .lock()
            .map_err(|_| Error::from_hresult(E_FAIL))?;
        batches.retain(|batch| batch.epoch != epoch);
        self.view
            .store
            .discarded_through
            .fetch_max(epoch, Ordering::AcqRel);
        drop(batches);
        if !self.view.edit.get().is_invalid() {
            unsafe {
                SetWindowTextW(self.view.edit.get(), windows_core::w!(""))?;
            }
        }
        if !self.has_material()
            && let Some(materials) = MATERIALS.get()
            && let Ok(mut materials) = materials.lock()
        {
            materials.retain(|store| !Arc::ptr_eq(store, &self.view.store));
        }
        Ok(())
    }
    pub fn can_offer(&self) -> bool {
        self.view.store.batches.lock().is_ok_and(|batches| {
            batches.len() < CONTEXT_LIMIT
                && batches.iter().map(|batch| batch.text.len()).sum::<usize>() < MATERIAL_LIMIT
        })
    }
    #[cfg(test)]
    pub fn offer(&self, epoch: u64, material: String) -> bool {
        let Some(reservation) = budget::reserve_batch(
            material
                .capacity()
                .saturating_add(128)
                .saturating_mul(16)
                .max(1),
            std::mem::size_of::<RecoveryBatch>(),
        )
        .map(Arc::new) else {
            return false;
        };
        self.offer_reserved(epoch, material, reservation)
    }
    pub fn offer_reserved(
        &self,
        epoch: u64,
        material: String,
        reservation: Arc<budget::Reservation>,
    ) -> bool {
        if epoch != 0 && epoch <= self.view.store.discarded_through.load(Ordering::Acquire) {
            return true;
        }
        let Ok(mut batches) = self.view.store.batches.lock() else {
            return false;
        };
        let existing = batches.iter().position(|batch| batch.epoch == epoch);
        let previous = existing.map_or(0, |index| batches[index].text.len());
        let total = batches.iter().map(|batch| batch.text.len()).sum::<usize>();
        let capacity = material
            .capacity()
            .checked_add(128)
            .and_then(|capacity| capacity.checked_mul(16));
        let retained = existing.map_or(&reservation, |index| &batches[index]._reservation);
        if epoch == 0
            || material.is_empty()
            || capacity.is_none_or(|capacity| capacity > retained.charge().text_bytes)
            || material.len() > MATERIAL_LIMIT
            || total - previous > MATERIAL_LIMIT - material.len()
            || (existing.is_none() && batches.len() >= CONTEXT_LIMIT)
        {
            return false;
        }
        if let Some(index) = existing {
            batches[index].text = material;
        } else {
            batches.push(RecoveryBatch {
                epoch,
                text: material,
                _reservation: reservation,
            });
        }
        drop(batches);
        if let Ok(mut materials) = MATERIALS.get_or_init(|| Mutex::new(Vec::new())).lock() {
            if !materials
                .iter()
                .any(|store| Arc::ptr_eq(store, &self.view.store))
            {
                materials.push(self.view.store.clone());
            }
        } else {
            return false;
        }
        // 外部销毁主窗口不丢 store；broker 的按钮提供显式重开入口。
        if self.ensure_broker().is_err() {
            return true;
        }
        unsafe {
            if release_display(&self.view).is_err() {
                return true;
            }
            if !self.view.edit.get().is_invalid() {
                let _ = SetWindowTextW(self.view.edit.get(), windows_core::w!(""));
            }
            let _ = ShowWindow(self.view.broker.get(), SW_SHOWNOACTIVATE);
        }
        true
    }
}
fn ensure_main(view: &Rc<View>) -> Result<()> {
    if !view.window.get().is_invalid() {
        return Ok(());
    }
    #[cfg(test)]
    if CREATE_FAILURE.with(Cell::get) {
        return Err(Error::new(E_FAIL, "故障注入：创建恢复主窗失败"));
    }
    // 安全：View 由 owner 的 Rc 保留；恢复主 HWND 可反复重建，不更改材料。
    let window = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            PCWSTR(view.class.as_ptr()),
            PCWSTR(wide("云笺：未完成输入，请查看恢复材料").as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            440,
            None,
            None,
            Some(view.instance),
            Some(Rc::as_ptr(view).cast()),
        )?
    };
    view.window.set(window);
    Ok(())
}
impl Drop for RecoveryWindow {
    fn drop(&mut self) {
        if self.check_owner().is_err() {
            std::mem::forget(self.view.clone());
            return;
        }
        // 安全：仅创建 STA 清理自己的窗口。保留临时 Rc 至所有同步窗口过程返回。
        unsafe {
            let mut destroyed = true;
            for window in [self.view.window.get(), self.view.broker.get()] {
                if !window.is_invalid() {
                    #[cfg(test)]
                    let fail = DROP_FAILURE.with(|fault| fault.get() == 1);
                    #[cfg(not(test))]
                    let fail = false;
                    if fail || DestroyWindow(window).is_err() {
                        destroyed = false;
                    }
                }
            }
            if !destroyed {
                std::mem::forget(self.view.clone());
                return;
            }
            #[cfg(test)]
            if DROP_FAILURE.with(|fault| fault.get() == 2) {
                std::mem::forget(self.view.clone());
                return;
            }
            let main = if self.view.main_registered.get() {
                UnregisterClassW(PCWSTR(self.view.class.as_ptr()), Some(self.view.instance))
            } else {
                Ok(())
            };
            let broker = if self.view.broker_registered.get() {
                UnregisterClassW(
                    PCWSTR(self.view.broker_class.as_ptr()),
                    Some(self.view.instance),
                )
            } else {
                Ok(())
            };
            if main.is_err() || broker.is_err() {
                // 注销未获证明时保留 endpoint 租约，不能让 DLL 返回虚假可卸载。
                std::mem::forget(self.view.clone());
                return;
            }
        }
        if let Some(endpoints) = ENDPOINTS.get()
            && let Ok(mut endpoints) = endpoints.lock()
        {
            endpoints.retain(|endpoint| !Arc::ptr_eq(endpoint, &self.view.endpoint));
        }
    }
}
fn release_display(view: &View) -> Result<()> {
    // 已显示正文也继续占用原 reservation；先清 EDIT，之后才可释放最后凭证。
    if !view.edit.get().is_invalid() && unsafe { IsWindow(Some(view.edit.get())) }.as_bool() {
        unsafe {
            SetWindowTextW(view.edit.get(), windows_core::w!(""))?;
            if GetWindowTextLengthW(view.edit.get()) != 0 {
                return Err(Error::new(E_FAIL, "恢复视图未确认清空，材料和预算继续保留"));
            }
        }
    }
    for displayed in view.shown.borrow_mut().drain(..) {
        let _ = displayed.store.viewer.compare_exchange(
            view.generation,
            0,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
    Ok(())
}
fn display(view: &Rc<View>, all: bool) -> Result<()> {
    release_display(view)?;
    ensure_main(view)?;
    let stores = if all {
        MATERIALS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .map_err(|_| Error::from_hresult(E_FAIL))?
            .clone()
    } else {
        vec![view.store.clone()]
    };
    let mut texts = Vec::new();
    for store in stores {
        if !store.has_material() {
            continue;
        }
        if store
            .viewer
            .compare_exchange(0, view.generation, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            release_display(view)?;
            return Err(Error::new(
                E_FAIL,
                "其他恢复视图正在显示该材料，请先关闭该视图或等待其退出",
            ));
        }
        let snapshot = match store.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let _ = store.viewer.compare_exchange(
                    view.generation,
                    0,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                release_display(view)?;
                return Err(error);
            }
        };
        view.shown.borrow_mut().push(DisplayedStore {
            store: store.clone(),
            _reservations: snapshot.reservations,
        });
        texts.push(snapshot.text);
    }
    let content = texts.join("\r\n======\r\n");
    let text = wide(&content);
    // 安全：一次只允许一个视图显示同一 store，避免反复重开形成无界正文副本。
    unsafe {
        SetWindowTextW(view.edit.get(), PCWSTR(text.as_ptr()))?;
        let _ = ShowWindow(view.window.get(), SW_SHOWNOACTIVATE);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn isolated(name: &str) -> bool {
    if std::env::var("CLOUDLEAF_RECOVERY_REVIEW_CHILD").as_deref() == Ok(name) {
        return false;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg(name)
        .args(["--exact", "--nocapture", "--test-threads=1"])
        .env("CLOUDLEAF_RECOVERY_REVIEW_CHILD", name)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() > until {
            child.kill().unwrap();
            panic!("本测试创建的恢复检查子进程超时");
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && text.contains("test result: ok. 1 passed;"),
        "{}\n{}",
        text,
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{text}");
    true
}
unsafe extern "system" fn main_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    dispatch(false, window, message, w, l)
}
unsafe extern "system" fn broker_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    dispatch(true, window, message, w, l)
}
fn dispatch(broker: bool, window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    boundary(|| {
        // 安全：同 STA 的 userdata 来自仍持有模块租约的 View；临时 Rc 保护重入销毁。
        unsafe {
            if message == WM_NCCREATE { let create = &*(l.0 as *const CREATESTRUCTW); SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize); }
            let ptr = GetWindowLongPtrW(window, GWLP_USERDATA) as *const View;
            if ptr.is_null() { return Ok(DefWindowProcW(window, message, w, l)); }
            Rc::increment_strong_count(ptr); let view = Rc::from_raw(ptr);
            match message {
                WM_CREATE if broker => {
                    for (id,text,y) in [(3,"重新打开本实例恢复材料",8),(4,"查看本进程其他未解决材料",44),(5,"明确放弃本进程全部恢复材料",80)] {
                        CreateWindowExW(WINDOW_EX_STYLE::default(), windows_core::w!("BUTTON"), PCWSTR(wide(text).as_ptr()), WS_CHILD | WS_VISIBLE, 12, y, 350, 30, Some(window), Some(HMENU(id as *mut _)), None, None)?;
                    }
                    Ok(LRESULT(0))
                }
                WM_CREATE => {
                    let usage = budget::usage(); let limits = budget::limits();
                    CreateWindowExW(WINDOW_EX_STYLE::default(), windows_core::w!("STATIC"), PCWSTR(wide(&format!("未知结果可能已写入，不自动重试。关闭表示放弃本实例材料。模块预算：实例{}/{}，批次{}/{}。",usage.hosts,limits.hosts,usage.batches,limits.batches)).as_ptr()), WS_CHILD | WS_VISIBLE, 12, 12, 600, 48, Some(window), None, None, None)?;
                    for (id, text, x, width) in [(1, "查看恢复材料", 12, 150), (2, "选择全部（Ctrl+C复制）", 180, 240)] {
                        CreateWindowExW(WINDOW_EX_STYLE::default(), windows_core::w!("BUTTON"), PCWSTR(wide(text).as_ptr()), WS_CHILD | WS_VISIBLE, x, 68, width, 30, Some(window), Some(HMENU(id as *mut _)), None, None)?;
                    }
                    let edit = CreateWindowExW(WS_EX_CLIENTEDGE, windows_core::w!("EDIT"), PCWSTR::null(), WS_CHILD | WS_VISIBLE | WS_VSCROLL | WINDOW_STYLE(ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32), 12, 112, 600, 270, Some(window), None, None, None)?;
                    view.edit.set(edit); Ok(LRESULT(0))
                }
                watchdog::NOTICE => { if let Some(host) = view.host.upgrade() { host.watchdog_notice()?; } Ok(LRESULT(0)) }
                CLEANUP => { view.endpoint.pending.store(false, Ordering::Release); view.endpoint.attempts.store(0, Ordering::Release); super::watchdog::reap(); reap(); Ok(LRESULT(0)) }
                REPAIR => { if view.store.has_material() { let _ = ShowWindow(view.broker.get(), SW_SHOWNOACTIVATE); } Ok(LRESULT(0)) }
                WM_COMMAND if w.0 & 0xffff == 3 => { release_display(&view)?; ensure_main(&view)?; SetWindowTextW(view.edit.get(), windows_core::w!(""))?; let _ = ShowWindow(view.window.get(), SW_SHOWNOACTIVATE); Ok(LRESULT(0)) }
                WM_COMMAND if w.0 & 0xffff == 1 => { display(&view, false)?; Ok(LRESULT(0)) }
                WM_COMMAND if w.0 & 0xffff == 4 => { display(&view, true)?; Ok(LRESULT(0)) }
                WM_COMMAND if w.0 & 0xffff == 5 => {
                    release_display(&view)?;
                    let stores = MATERIALS.get_or_init(|| Mutex::new(Vec::new())).lock().map_err(|_| Error::from_hresult(E_FAIL))?.clone();
                    for store in stores { store.clear()?; }
                    if !view.edit.get().is_invalid() { SetWindowTextW(view.edit.get(), windows_core::w!(""))?; }
                    Ok(LRESULT(0))
                }
                WM_COMMAND if w.0 & 0xffff == 2 => { SendMessageW(view.edit.get(), EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1))); let _ = SetFocus(Some(view.edit.get())); Ok(LRESULT(0)) }
                WM_CLOSE => {
                    release_display(&view)?;
                    let last = view.store.clear()?;
                    if let Some(host) = view.host.upgrade() && last == Some(host.epoch.get()) && host.quarantined.get() { host.discard_recovery()?; }
                    for hwnd in [view.window.get(), view.broker.get()] { if !hwnd.is_invalid() { let _ = ShowWindow(hwnd, SW_HIDE); } }
                    if !view.edit.get().is_invalid() { SetWindowTextW(view.edit.get(), windows_core::w!(""))?; }
                    Ok(LRESULT(0))
                }
                WM_NCDESTROY => {
                    if broker { view.broker.set(HWND::default()); view.endpoint.broker.store(0, Ordering::Release); } else { release_display(&view)?; view.window.set(HWND::default()); view.edit.set(HWND::default()); }
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0); Ok(DefWindowProcW(window, message, w, l))
                }
                _ => Ok(DefWindowProcW(window, message, w, l)),
            }
        }
    }).unwrap_or(LRESULT(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    thread_local! { static EDIT_PREVIOUS: Cell<isize> = const { Cell::new(0) }; }
    unsafe extern "system" fn reject_clear(
        window: HWND,
        message: u32,
        w: WPARAM,
        l: LPARAM,
    ) -> LRESULT {
        // 安全：只 subclass 本测试创建的 EDIT；拒绝清屏以复现原生 WM_SETTEXT 失败。
        unsafe {
            if message == WM_SETTEXT && l.0 != 0 && *(l.0 as *const u16) == 0 {
                return LRESULT(0);
            }
            let previous: WNDPROC = EDIT_PREVIOUS.with(|old| std::mem::transmute(old.get()));
            CallWindowProcW(previous, window, message, w, l)
        }
    }
    struct Apartment;
    impl Apartment {
        fn new() -> Self {
            unsafe {
                windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
                )
                .ok()
                .unwrap();
            }
            Self
        }
    }
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                windows::Win32::System::Com::CoUninitialize();
            }
        }
    }

    fn edit_text(window: &RecoveryWindow) -> String {
        let mut buffer = [0u16; 4096];
        // 安全：只读取本测试创建的 EDIT 控件和固定容量 UTF-16 缓冲区。
        let size = unsafe { GetWindowTextW(window.view.edit.get(), &mut buffer) };
        String::from_utf16(&buffer[..size as usize]).unwrap()
    }
    fn command(window: &RecoveryWindow, id: usize) {
        // 安全：同 STA 调用生产窗口过程；不操作宿主文档或系统输入法配置。
        unsafe {
            SendMessageW(
                window.handle(),
                WM_COMMAND,
                Some(WPARAM(id)),
                Some(LPARAM(0)),
            );
        }
    }
    fn close(window: &RecoveryWindow) {
        // 安全：同 STA 向自有窗口发送用户关闭命令。
        unsafe {
            SendMessageW(window.handle(), WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0)));
        }
    }

    fn pump() {
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = DispatchMessageW(&message);
            }
        }
    }

    #[test]
    fn failed_clear_keeps_display_token_after_source_was_explicitly_discarded() {
        if isolated(
            "native::recovery::tests::failed_clear_keeps_display_token_after_source_was_explicitly_discarded",
        ) {
            return;
        }
        let host = Host::new().unwrap();
        let source = RecoveryWindow::new(Rc::downgrade(&host), 1).unwrap();
        assert!(source.offer(1, "Unknown 保留显示副本".into()));
        let reader = RecoveryWindow::new(Rc::downgrade(&host), 2).unwrap();
        command(&reader, 4);
        let shown = edit_text(&reader);
        assert!(shown.contains("保留显示副本"));
        close(&source);
        assert!(!source.has_material());
        assert_eq!(budget::usage().batches, 1);
        let old = unsafe {
            SetWindowLongPtrW(
                reader.view.edit.get(),
                GWLP_WNDPROC,
                reject_clear as *const () as isize,
            )
        };
        EDIT_PREVIOUS.with(|previous| previous.set(old));
        close(&reader);
        assert_eq!(edit_text(&reader), shown);
        assert_eq!(reader.view.shown.borrow().len(), 1);
        assert_eq!(budget::usage().batches, 1);
        unsafe {
            SetWindowLongPtrW(reader.view.edit.get(), GWLP_WNDPROC, old);
        }
        close(&reader);
        assert!(edit_text(&reader).is_empty());
        assert_eq!(budget::usage().batches, 0);
    }

    #[test]
    fn failed_main_creation_does_not_leave_a_claim_that_blocks_source_reopen() {
        let host = Host::new().unwrap();
        let source = RecoveryWindow::new(Rc::downgrade(&host), 1).unwrap();
        assert!(source.offer(1, "Unknown 主窗创建失败".into()));
        let reader = RecoveryWindow::new(Rc::downgrade(&host), 2).unwrap();
        unsafe {
            DestroyWindow(reader.view.window.get()).unwrap();
        }
        CREATE_FAILURE.with(|fault| fault.set(true));
        command(&reader, 4);
        CREATE_FAILURE.with(|fault| fault.set(false));
        assert_eq!(source.view.store.viewer.load(Ordering::Acquire), 0);
        assert!(reader.view.shown.borrow().is_empty());
        drop(reader);
        command(&source, 1);
        assert!(edit_text(&source).contains("主窗创建失败"));
        close(&source);
    }

    #[test]
    fn one_locked_snapshot_binds_text_and_tokens_across_real_owner_updates_and_clear() {
        if isolated(
            "native::recovery::tests::one_locked_snapshot_binds_text_and_tokens_across_real_owner_updates_and_clear",
        ) {
            return;
        }
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (control_tx, control_rx) = std::sync::mpsc::channel();
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        let owner = std::thread::spawn(move || {
            let _apartment = Apartment::new();
            let host = Host::new().unwrap();
            let window = RecoveryWindow::new(Rc::downgrade(&host), 1).unwrap();
            assert!(window.offer(1, "旧批state=Unknown".into()));
            ready_tx.send(window.view.store.clone()).unwrap();
            control_rx.recv().unwrap();
            assert!(window.offer(2, "新批state=Unknown".into()));
            ack_tx.send(()).unwrap();
            control_rx.recv().unwrap();
            close(&window);
        });
        let store = ready_rx.recv().unwrap();
        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.reservations.len(), 1);
        assert!(snapshot.text.contains("旧批"));
        control_tx.send(()).unwrap();
        ack_rx.recv().unwrap();
        assert!(!snapshot.text.contains("新批"));
        let newest = store.snapshot().unwrap();
        assert_eq!(newest.reservations.len(), 2);
        assert!(newest.text.contains("新批"));
        drop(newest);
        control_tx.send(()).unwrap();
        owner.join().unwrap();
        assert_eq!(budget::usage().batches, 1);
        drop(snapshot);
        assert_eq!(budget::usage().batches, 0);
    }

    #[test]
    fn actual_class_unregister_failure_pins_userdata_and_host_budget_until_admission_stops() {
        if isolated(
            "native::recovery::tests::actual_class_unregister_failure_pins_userdata_and_host_budget_until_admission_stops",
        ) {
            return;
        }
        let mut raw_windows = Vec::new();
        for index in 0..budget::limits().hosts {
            let host = Host::new().unwrap();
            let window = RecoveryWindow::new(Rc::downgrade(&host), index as u64 + 1).unwrap();
            // 安全：额外自有窗口使真实 UnregisterClassW 返回“类仍有窗口”，不是伪造成功。
            let extra = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    PCWSTR(window.view.class.as_ptr()),
                    windows_core::w!(""),
                    WS_OVERLAPPED,
                    0,
                    0,
                    50,
                    50,
                    None,
                    None,
                    Some(window.view.instance),
                    Some(Rc::as_ptr(&window.view).cast()),
                )
                .unwrap()
            };
            let view = Rc::downgrade(&window.view);
            drop(window);
            drop(host);
            assert!(view.upgrade().is_some());
            assert!(unsafe { IsWindow(Some(extra)) }.as_bool());
            raw_windows.push(extra);
            assert_eq!(budget::usage().hosts, index + 1);
        }
        assert!(Host::new().is_err());
        assert_eq!(super::super::service::DllCanUnloadNow(), S_FALSE);
        assert_eq!(raw_windows.len(), budget::limits().hosts);
        println!(
            "{{\"experiment\":\"real-unregister-failure-cap\",\"retained_hosts\":{},\"new_admission_rejected\":true,\"dll_unload\":\"S_FALSE\"}}",
            raw_windows.len()
        );
    }

    #[test]
    fn destroyed_main_reopens_from_broker_without_losing_or_duplicating_unknown_material() {
        let host = Host::new().unwrap();
        let window = RecoveryWindow::new(Rc::downgrade(&host), 0x7633).unwrap();
        assert!(window.offer(
            7,
            "sequence=1 state=Unknown\r\nsequence=2 state=NotApplied\r\n保留文本".into()
        ));
        let original = window.view.store.text().unwrap();
        for _ in 0..20 {
            command(&window, 1);
            assert_eq!(edit_text(&window), original);
            unsafe {
                DestroyWindow(window.view.window.get()).unwrap();
            }
            assert!(window.view.window.get().is_invalid());
            assert!(window.has_material());
            assert_eq!(window.view.store.batches.lock().unwrap().len(), 1);
            command(&window, 3);
            assert!(edit_text(&window).is_empty());
        }
        command(&window, 1);
        assert_eq!(edit_text(&window), original);
        close(&window);
        assert!(!window.has_material());
        assert!(window.offer(7, "迟到回调不能复活已放弃材料".into()));
        assert!(!window.has_material());
    }

    #[test]
    fn unload_other_thread_requests_owner_without_destroying_or_waiting_for_message_pump() {
        if isolated(
            "native::recovery::tests::unload_other_thread_requests_owner_without_destroying_or_waiting_for_message_pump",
        ) {
            return;
        }
        let host = Host::new().unwrap();
        let window = Rc::new(RecoveryWindow::new(Rc::downgrade(&host), 0x7633).unwrap());
        let broker = window.handle();
        let main = window.view.window.get();
        retire(window.clone(), Arc::new(AtomicBool::new(true)));
        drop(window);
        drop(host);
        let result = std::thread::spawn(|| {
            let start = std::time::Instant::now();
            let status = super::super::service::DllCanUnloadNow();
            (status, start.elapsed())
        })
        .join()
        .unwrap();
        assert_eq!(result.0, S_FALSE);
        assert!(result.1 < std::time::Duration::from_millis(250));
        assert!(unsafe { IsWindow(Some(main)) }.as_bool());
        assert!(unsafe { IsWindow(Some(broker)) }.as_bool());
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(unsafe { IsWindow(Some(main)) }.as_bool());
        pump();
        assert!(!unsafe { IsWindow(Some(main)) }.as_bool());
        assert!(!unsafe { IsWindow(Some(broker)) }.as_bool());
        assert_eq!(super::super::service::DllCanUnloadNow(), S_OK);
        assert_eq!(budget::usage(), budget::Usage::default());
    }

    #[test]
    fn partial_window_class_failure_balances_registered_classes_and_module_leases() {
        if isolated(
            "native::recovery::tests::partial_window_class_failure_balances_registered_classes_and_module_leases",
        ) {
            return;
        }
        for index in [0, 1] {
            let host = Host::new().unwrap();
            REGISTER_FAILURE.with(|fault| fault.set(Some(index)));
            assert!(RecoveryWindow::new(Rc::downgrade(&host), 0x7633).is_err());
            REGISTER_FAILURE.with(|fault| fault.set(None));
            drop(host);
            assert_eq!(super::super::service::DllCanUnloadNow(), S_OK);
            assert_eq!(budget::usage(), budget::Usage::default());
        }
    }

    #[test]
    fn failed_cleanup_notice_is_bounded_and_destroyed_broker_is_repaired_only_by_owner() {
        if isolated(
            "native::recovery::tests::failed_cleanup_notice_is_bounded_and_destroyed_broker_is_repaired_only_by_owner",
        ) {
            return;
        }
        let host = Host::new().unwrap();
        let window = RecoveryWindow::new(Rc::downgrade(&host), 0x7633).unwrap();
        assert!(window.offer(11, "state=Unknown 通知失败仍保留".into()));
        let original = window.evidence();
        let usage = budget::usage();
        window
            .view
            .endpoint
            .fail_delivery
            .store(true, Ordering::Release);
        std::thread::spawn(|| {
            for _ in 0..20 {
                assert_eq!(super::super::service::DllCanUnloadNow(), S_FALSE);
            }
        })
        .join()
        .unwrap();
        assert_eq!(window.view.endpoint.attempts.load(Ordering::Acquire), 4);
        assert!(window.view.endpoint.failed.load(Ordering::Acquire));
        assert_eq!(window.evidence(), original);
        assert_eq!(budget::usage(), usage);
        let old = window.handle();
        unsafe {
            DestroyWindow(old).unwrap();
        }
        assert!(window.handle().is_invalid());
        std::thread::spawn(|| assert_eq!(super::super::service::DllCanUnloadNow(), S_FALSE))
            .join()
            .unwrap();
        let repaired = window.repair().unwrap();
        assert!(!repaired.is_invalid());
        assert_eq!(window.evidence(), original);
        command(&window, 1);
        assert_eq!(edit_text(&window), original);
        close(&window);
        drop(window);
        drop(host);
        assert_eq!(super::super::service::DllCanUnloadNow(), S_OK);
    }

    #[test]
    fn physical_sta_exit_keeps_data_reachable_from_new_explicit_entry_but_pins_unfinished_owner() {
        if isolated(
            "native::recovery::tests::physical_sta_exit_keeps_data_reachable_from_new_explicit_entry_but_pins_unfinished_owner",
        ) {
            return;
        }
        std::thread::spawn(|| {
            let _apartment = Apartment::new();
            let host = Host::new().unwrap();
            let window = Rc::new(RecoveryWindow::new(Rc::downgrade(&host), 0xdead).unwrap());
            assert!(window.offer(19, "旧STA sequence=9 state=Unknown 不自动重试".into()));
            unsafe {
                DestroyWindow(window.view.window.get()).unwrap();
                DestroyWindow(window.handle()).unwrap();
            }
            retire(window, Arc::new(AtomicBool::new(true)));
            drop(host);
        })
        .join()
        .unwrap();
        let _apartment = Apartment::new();
        let host = Host::new().unwrap();
        let window = RecoveryWindow::new(Rc::downgrade(&host), 0xbeef).unwrap();
        assert!(edit_text(&window).is_empty());
        command(&window, 4);
        let recovered = edit_text(&window);
        assert!(recovered.contains("instance=57005"));
        assert!(recovered.contains("epoch=19"));
        assert!(recovered.contains("state=Unknown"));
        assert_eq!(recovered.matches("旧STA").count(), 1);
        command(&window, 5);
        assert_eq!(budget::usage().batches, 0);
        drop(window);
        drop(host);
        // 旧 STA 的 HWND类注销尚无安全证据：此处必须明确保留阻塞，不能伪造退出成功。
        assert_eq!(super::super::service::DllCanUnloadNow(), S_FALSE);
        assert_eq!(budget::usage().hosts, 1);
    }

    #[test]
    fn offer_hides_previous_display_and_separates_epochs_without_changing_new_context() {
        let host = Host::new().unwrap();
        let window = RecoveryWindow::new(Rc::downgrade(&host), 0x726576696577).unwrap();
        assert!(window.offer(host.epoch.get(), "旧上下文私有材料".into()));
        assert!(edit_text(&window).is_empty());
        command(&window, 1);
        assert!(edit_text(&window).contains("旧上下文私有材料"));
        host.epoch.set(host.epoch.get() + 1);
        assert!(window.offer(host.epoch.get(), "新上下文私有材料".into()));
        assert!(edit_text(&window).is_empty());
        command(&window, 1);
        assert!(edit_text(&window).contains("旧上下文私有材料"));
        assert!(edit_text(&window).contains("新上下文私有材料"));
        host.epoch.set(host.epoch.get() + 1);
        host.quarantined.set(true);
        let before = host.epoch.get();
        close(&window);
        assert_eq!(host.epoch.get(), before);
        assert!(host.quarantined.get());
        assert!(!window.has_material());
        assert!(edit_text(&window).is_empty());
    }

    #[test]
    fn retired_recovery_remains_accessible_after_host_release_until_explicit_close() {
        let host = Host::new().unwrap();
        let weak = Rc::downgrade(&host);
        let window = Rc::new(RecoveryWindow::new(weak.clone(), 0x726576696577).unwrap());
        assert!(window.offer(host.epoch.get(), "已停用服务仍待核对的材料".into()));
        let handle = window.handle();
        retire(window.clone(), Arc::new(AtomicBool::new(true)));
        drop(host);
        assert!(weak.upgrade().is_none());
        reap();
        // 安全：窗口在退役集合持有，测试只查询自己的窗口。
        assert!(unsafe { IsWindow(Some(handle)) }.as_bool());
        command(&window, 1);
        assert!(edit_text(&window).contains("已停用服务仍待核对的材料"));
        close(&window);
        drop(window);
        reap();
        assert!(!unsafe { IsWindow(Some(handle)) }.as_bool());
    }

    #[test]
    fn material_budget_rejects_new_context_without_overwriting_unreleased_records() {
        let host = Host::new().unwrap();
        let window = RecoveryWindow::new(Rc::downgrade(&host), 0x726576696577).unwrap();
        for index in 0..CONTEXT_LIMIT {
            host.epoch.set(index as u64 + 1);
            assert!(window.offer(host.epoch.get(), format!("保全材料{index}")));
        }
        assert!(!window.can_offer());
        host.epoch.set(CONTEXT_LIMIT as u64 + 1);
        assert!(!window.offer(host.epoch.get(), "不得覆盖旧材料".into()));
        assert_eq!(
            window.view.store.batches.lock().unwrap().len(),
            CONTEXT_LIMIT
        );
        command(&window, 1);
        let content = edit_text(&window);
        assert!(content.contains("保全材料0"));
        assert!(content.contains("保全材料31"));
        assert!(!content.contains("不得覆盖旧材料"));
        close(&window);
    }

    #[test]
    fn late_transfer_keeps_source_epoch_after_host_has_already_invalidated() {
        let host = Host::new().unwrap();
        let window = RecoveryWindow::new(Rc::downgrade(&host), 0x726576696577).unwrap();
        let source_epoch = host.epoch.get();
        // 生产失效路径先使 Host 进入下一代，再转交旧 ledger；来源不能由当前 Host 推断。
        host.epoch.set(source_epoch + 1);
        assert!(!window.offer(0, "缺少来源身份，不得覆盖材料".into()));
        assert!(!window.has_material());
        assert!(window.offer(source_epoch, "旧上下文未完成的原始事件".into()));
        assert_eq!(
            window.view.store.batches.lock().unwrap()[0].epoch,
            source_epoch
        );
        command(&window, 1);
        assert!(edit_text(&window).contains(&format!("epoch={source_epoch}")));
        host.quarantined.set(true);
        let new_epoch = host.epoch.get();
        close(&window);
        // 关闭旧材料只能放弃旧材料，不能调用 discard_recovery 取消新的隔离上下文。
        assert_eq!(host.epoch.get(), new_epoch);
        assert!(host.quarantined.get());
        assert!(!window.has_material());
    }
}
