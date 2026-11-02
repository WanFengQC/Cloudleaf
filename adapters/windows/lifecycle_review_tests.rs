//! 调用生产 COM 服务的独立审查；空焦点宿主不是 Windows 应用验收。
#![allow(unused_variables)]
use super::*;
use cloudleaf_types::InputState;
use std::process::{Command, Stdio};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct ManagerState {
    fail: Cell<u8>,
    key: RefCell<Option<ITfKeyEventSink>>,
    thread: RefCell<Option<IUnknown>>,
    key_advise: Cell<u32>,
    key_unadvise: Cell<u32>,
    thread_advise: Cell<u32>,
    thread_unadvise: Cell<u32>,
    drops: Cell<u32>,
}
#[implement(ITfThreadMgr, ITfKeystrokeMgr, ITfSource, Agile = false)]
struct ReviewManager {
    state: Rc<ManagerState>,
}
impl Drop for ReviewManager {
    fn drop(&mut self) {
        self.state.drops.set(self.state.drops.get() + 1);
    }
}
impl ITfThreadMgr_Impl for ReviewManager_Impl {
    fn Activate(&self) -> windows_core::Result<u32> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn Deactivate(&self) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn CreateDocumentMgr(&self) -> windows_core::Result<ITfDocumentMgr> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn EnumDocumentMgrs(&self) -> windows_core::Result<IEnumTfDocumentMgrs> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetFocus(&self) -> windows_core::Result<ITfDocumentMgr> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn SetFocus(&self, pdimfocus: windows_core::Ref<ITfDocumentMgr>) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn AssociateFocus(
        &self,
        hwnd: HWND,
        pdimnew: windows_core::Ref<ITfDocumentMgr>,
    ) -> windows_core::Result<ITfDocumentMgr> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn IsThreadFocus(&self) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetFunctionProvider(
        &self,
        clsid: *const windows_core::GUID,
    ) -> windows_core::Result<ITfFunctionProvider> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn EnumFunctionProviders(&self) -> windows_core::Result<IEnumTfFunctionProviders> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetGlobalCompartment(&self) -> windows_core::Result<ITfCompartmentMgr> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
}
impl ITfKeystrokeMgr_Impl for ReviewManager_Impl {
    fn AdviseKeyEventSink(
        &self,
        tid: u32,
        psink: windows_core::Ref<ITfKeyEventSink>,
        fforeground: windows_core::BOOL,
    ) -> windows_core::Result<()> {
        self.state.key_advise.set(self.state.key_advise.get() + 1);
        assert_eq!(tid, 77);
        if self.state.fail.get() == 1 {
            return Err(Error::from_hresult(E_FAIL));
        }
        *self.state.key.borrow_mut() = Some(psink.as_ref().unwrap().clone());
        Ok(())
    }
    fn UnadviseKeyEventSink(&self, tid: u32) -> windows_core::Result<()> {
        self.state
            .key_unadvise
            .set(self.state.key_unadvise.get() + 1);
        assert_eq!(tid, 77);
        if self.state.fail.get() == 4 {
            return Err(Error::from_hresult(E_FAIL));
        }
        self.state.key.borrow_mut().take();
        Ok(())
    }
    fn GetForeground(&self) -> windows_core::Result<windows_core::GUID> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn TestKeyDown(
        &self,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn TestKeyUp(
        &self,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn KeyDown(&self, wparam: WPARAM, lparam: LPARAM) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn KeyUp(&self, wparam: WPARAM, lparam: LPARAM) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetPreservedKey(
        &self,
        pic: windows_core::Ref<ITfContext>,
        pprekey: *const TF_PRESERVEDKEY,
    ) -> windows_core::Result<windows_core::GUID> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn IsPreservedKey(
        &self,
        rguid: *const windows_core::GUID,
        pprekey: *const TF_PRESERVEDKEY,
    ) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn PreserveKey(
        &self,
        tid: u32,
        rguid: *const windows_core::GUID,
        prekey: *const TF_PRESERVEDKEY,
        pchdesc: &windows_core::PCWSTR,
        cchdesc: u32,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn UnpreserveKey(
        &self,
        rguid: *const windows_core::GUID,
        pprekey: *const TF_PRESERVEDKEY,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn SetPreservedKeyDescription(
        &self,
        rguid: *const windows_core::GUID,
        pchdesc: &windows_core::PCWSTR,
        cchdesc: u32,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetPreservedKeyDescription(
        &self,
        rguid: *const windows_core::GUID,
    ) -> windows_core::Result<windows_core::BSTR> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn SimulatePreservedKey(
        &self,
        pic: windows_core::Ref<ITfContext>,
        rguid: *const windows_core::GUID,
    ) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
}
impl ITfSource_Impl for ReviewManager_Impl {
    fn AdviseSink(
        &self,
        riid: *const windows_core::GUID,
        punk: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<u32> {
        self.state
            .thread_advise
            .set(self.state.thread_advise.get() + 1);
        if self.state.fail.get() == 2 {
            return Err(Error::from_hresult(E_FAIL));
        }
        assert_eq!(unsafe { *riid }, ITfThreadMgrEventSink::IID);
        *self.state.thread.borrow_mut() = Some(punk.as_ref().unwrap().clone());
        Ok(41)
    }
    fn UnadviseSink(&self, dwcookie: u32) -> windows_core::Result<()> {
        self.state
            .thread_unadvise
            .set(self.state.thread_unadvise.get() + 1);
        assert_eq!(dwcookie, 41);
        if self.state.fail.get() == 3 {
            return Err(Error::from_hresult(E_FAIL));
        }
        self.state.thread.borrow_mut().take();
        Ok(())
    }
}

fn manager(state: &Rc<ManagerState>) -> ITfThreadMgr {
    ReviewManager {
        state: state.clone(),
    }
    .into()
}
fn processor() -> (ITfTextInputProcessor, std::rc::Weak<Host>) {
    let host = Host::new().unwrap();
    let weak = Rc::downgrade(&host);
    (
        Service {
            host,
            cookie: Cell::new(None),
        }
        .into(),
        weak,
    )
}

fn run_isolated(name: &str) -> bool {
    if std::env::var_os("CLOUDLEAF_LIFECYCLE_REVIEW_CHILD").is_some() {
        return false;
    }
    // 每项用独立进程核对全局 DLL 计数，避免与同进程其他原生测试竞争。
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg(format!("native::service::lifecycle_review_tests::{name}"))
        .args([
            "--exact",
            "--include-ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CLOUDLEAF_LIFECYCLE_REVIEW_CHILD", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            // 只终止本测试自己创建并持有句柄的子进程，不能误终止其他程序。
            child.kill().unwrap();
            let result = child.wait_with_output().unwrap();
            panic!(
                "生命周期子进程超时：{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("test result: ok. 1 passed;"),
        "必须实际执行一项子进程检查，不能用过滤后的零测试冒充成功"
    );
    true
}

#[test]
fn lifecycle_review_factory_lock_and_reference_lease_are_balanced() {
    if run_isolated("lifecycle_review_factory_lock_and_reference_lease_are_balanced") {
        return;
    }
    let baseline = OBJECTS.load(Ordering::Acquire);
    assert_eq!(DllCanUnloadNow(), S_OK);
    let mut raw = std::ptr::null_mut();
    // 安全：本地 GUID 和 out 槽在调用期间有效，接收生产导出的 COM 工厂。
    assert_eq!(
        unsafe { DllGetClassObject(&SERVICE_ID, &IClassFactory::IID, &mut raw) },
        S_OK
    );
    let factory = unsafe { IClassFactory::from_raw(raw) };
    assert_eq!(OBJECTS.load(Ordering::Acquire), baseline + 1);
    assert_eq!(DllCanUnloadNow(), S_FALSE);
    // 安全：测试在单线程内保持工厂和服务引用，不注册或激活系统输入法。
    let service: ITfTextInputProcessor =
        unsafe { factory.CreateInstance(None::<&IUnknown>) }.unwrap();
    assert_eq!(OBJECTS.load(Ordering::Acquire), baseline + 2);
    unsafe { factory.LockServer(true) }.unwrap();
    drop(service);
    assert_eq!(OBJECTS.load(Ordering::Acquire), baseline + 1);
    unsafe { factory.LockServer(false) }.unwrap();
    drop(factory);
    assert_eq!(OBJECTS.load(Ordering::Acquire), baseline);
    assert_eq!(DllCanUnloadNow(), S_OK);
}

#[test]
fn lifecycle_review_repeated_activate_deactivate_releases_manager_and_sinks() {
    if run_isolated("lifecycle_review_repeated_activate_deactivate_releases_manager_and_sinks") {
        return;
    }
    let baseline = OBJECTS.load(Ordering::Acquire);
    let (service, weak) = processor();
    let state = Rc::new(ManagerState::default());
    let mgr = manager(&state);
    for _ in 0..100 {
        // 安全：假 manager 与真实服务同线程，完整持有接口并遵守 COM 返回值。
        unsafe { service.Activate(&mgr, 77) }.unwrap();
        assert!(state.key.borrow().is_some());
        assert!(state.thread.borrow().is_some());
        assert!(weak.upgrade().unwrap().manager.borrow().is_some());
        unsafe { service.Deactivate() }.unwrap();
        assert!(state.key.borrow().is_none());
        assert!(state.thread.borrow().is_none());
        let host = weak.upgrade().unwrap();
        assert!(host.manager.borrow().is_none());
        assert!(host.context.borrow().is_none());
        assert_eq!(host.client.get(), 0);
        assert_eq!(host.engine.borrow().frame().state, InputState::Inactive);
    }
    assert_eq!(state.key_advise.get(), 100);
    assert_eq!(state.key_unadvise.get(), 100);
    assert_eq!(state.thread_advise.get(), 100);
    assert_eq!(state.thread_unadvise.get(), 100);
    drop(service);
    assert!(weak.upgrade().is_none());
    drop(mgr);
    assert_eq!(state.drops.get(), 1);
    wait_for_native_workers_and_windows();
    assert_eq!(OBJECTS.load(Ordering::Acquire), baseline);
    assert_eq!(DllCanUnloadNow(), S_OK);
}

#[test]
fn lifecycle_review_partial_activation_failure_rolls_back_only_completed_steps() {
    if run_isolated("lifecycle_review_partial_activation_failure_rolls_back_only_completed_steps") {
        return;
    }
    let baseline = OBJECTS.load(Ordering::Acquire);
    for fail in [1, 2] {
        let (service, weak) = processor();
        let state = Rc::new(ManagerState::default());
        state.fail.set(fail);
        let mgr = manager(&state);
        // 安全：失败点由本地 fixture 注入，不调用系统注册服务。
        assert!(unsafe { service.Activate(&mgr, 77) }.is_err());
        assert!(state.key.borrow().is_none());
        assert!(state.thread.borrow().is_none());
        assert_eq!(state.key_unadvise.get(), u32::from(fail == 2));
        assert_eq!(state.thread_unadvise.get(), 0);
        let host = weak.upgrade().unwrap();
        assert!(host.manager.borrow().is_none());
        assert_eq!(host.client.get(), 0);
        assert_eq!(host.engine.borrow().frame().state, InputState::Inactive);
        drop(host);
        drop(service);
        assert!(weak.upgrade().is_none());
        drop(mgr);
        assert_eq!(state.drops.get(), 1);
    }
    wait_for_native_workers_and_windows();
    assert_eq!(OBJECTS.load(Ordering::Acquire), baseline);
}

fn wait_for_native_workers_and_windows() {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while DllCanUnloadNow() != S_OK {
        assert!(
            std::time::Instant::now() < deadline,
            "后台线程或 STA 窗口没有在上限内退出，禁止卸载"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn lifecycle_review_unadvise_failure_defangs_host_and_pins_dll_until_external_release() {
    if run_isolated(
        "lifecycle_review_unadvise_failure_defangs_host_and_pins_dll_until_external_release",
    ) {
        return;
    }
    let baseline = OBJECTS.load(Ordering::Acquire);
    for fail in [3, 4] {
        let (service, weak) = processor();
        let state = Rc::new(ManagerState::default());
        let mgr = manager(&state);
        // 安全：同线程调用真实 Activate/Deactivate，假宿主可保留拒绝撤销的引用。
        unsafe { service.Activate(&mgr, 77) }.unwrap();
        state.fail.set(fail);
        assert!(unsafe { service.Deactivate() }.is_err());
        assert_eq!(state.key_unadvise.get(), 1);
        assert_eq!(state.thread_unadvise.get(), 1);
        let host = weak.upgrade().unwrap();
        assert!(host.manager.borrow().is_none());
        assert!(host.context.borrow().is_none());
        assert_eq!(host.client.get(), 0);
        assert_eq!(host.engine.borrow().frame().state, InputState::Inactive);
        drop(host);
        drop(service);
        assert!(weak.upgrade().is_some());
        assert_eq!(DllCanUnloadNow(), S_FALSE);
        // 外部宿主释放仍存活的 sink 后才允许卸载，不能虚假递减模块计数。
        state.key.borrow_mut().take();
        state.thread.borrow_mut().take();
        assert!(weak.upgrade().is_none());
        drop(mgr);
        assert_eq!(state.drops.get(), 1);
    }
    wait_for_native_workers_and_windows();
    assert_eq!(OBJECTS.load(Ordering::Acquire), baseline);
    assert_eq!(DllCanUnloadNow(), S_OK);
}

#[test]
#[ignore = "须显式提供已构建的真实 DLL；独立假 manager 仅测装载退出，不注册或激活系统输入法"]
fn lifecycle_review_real_dll_loader_no_registration() {
    if run_isolated("lifecycle_review_real_dll_loader_no_registration") {
        return;
    }
    use std::time::{Duration, Instant};
    use windows::Win32::{
        System::{LibraryLoader::*, Threading::*},
        UI::WindowsAndMessaging::*,
    };
    use windows_core::{PCSTR, PCWSTR};
    let path = std::path::PathBuf::from(
        std::env::var_os("CLOUDLEAF_REVIEW_DLL").expect("须提供 DLL 路径"),
    );
    assert!(path.is_absolute() && path.is_file());
    let wide: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut baseline = 0;
    let mut final_handles = 0;
    for iteration in 0..21 {
        // 安全：路径和导出名缓冲区持续有效；仅使用核对过的 Cloudleaf COM ABI。
        unsafe {
            let module = LoadLibraryW(PCWSTR(wide.as_ptr())).unwrap();
            let factory_fn: unsafe extern "system" fn(
                *const GUID,
                *const GUID,
                *mut *mut c_void,
            ) -> HRESULT = std::mem::transmute(
                GetProcAddress(module, PCSTR(c"DllGetClassObject".as_ptr().cast())).unwrap(),
            );
            let unload_fn: unsafe extern "system" fn() -> HRESULT = std::mem::transmute(
                GetProcAddress(module, PCSTR(c"DllCanUnloadNow".as_ptr().cast())).unwrap(),
            );
            assert_eq!(unload_fn(), S_OK);
            let mut raw = std::ptr::null_mut();
            assert_eq!(factory_fn(&SERVICE_ID, &IClassFactory::IID, &mut raw), S_OK);
            let factory = IClassFactory::from_raw(raw);
            let service: ITfTextInputProcessor = factory.CreateInstance(None::<&IUnknown>).unwrap();
            let state = Rc::new(ManagerState::default());
            let mgr = manager(&state);
            service.Activate(&mgr, 77).unwrap();
            assert_eq!(unload_fn(), S_FALSE);
            service.Deactivate().unwrap();
            assert!(state.key.borrow().is_none() && state.thread.borrow().is_none());
            drop(service);
            drop(factory);
            drop(mgr);
            assert_eq!(state.drops.get(), 1);
            let deadline = Instant::now() + Duration::from_secs(2);
            while unload_fn() != S_OK && Instant::now() < deadline {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(unload_fn(), S_OK, "退出后两秒内仍有模块租约");
            FreeLibrary(module).unwrap();
            assert!(
                GetModuleHandleW(PCWSTR(wide.as_ptr())).is_err(),
                "真实 DLL 映射仍存活"
            );
            GetProcessHandleCount(GetCurrentProcess(), &mut final_handles).unwrap();
            if iteration == 0 {
                baseline = final_handles;
            }
        }
    }
    assert!(
        final_handles <= baseline + 2,
        "重复装载后句柄增长：{baseline} -> {final_handles}"
    );
    println!(
        "{{\"diagnostic\":\"real_dll_fake_manager_lifecycle\",\"iterations\":20,\"warmup\":1,\"baseline_handles\":{baseline},\"final_handles\":{final_handles},\"registered\":false,\"real_application_acceptance\":false}}"
    );
}
