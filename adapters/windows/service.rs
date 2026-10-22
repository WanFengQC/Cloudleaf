//! 进程内 COM 服务与 TSF 生命周期；不在 DllMain 获取锁或初始化 COM。
use super::{
    SERVICE_ID, boundary, keys,
    session::{Host, Shared},
};
use std::{
    cell::Cell,
    ffi::c_void,
    sync::atomic::{AtomicUsize, Ordering},
};
use windows::Win32::{Foundation::*, System::Com::*, UI::TextServices::*};
use windows_core::{
    BOOL, Error, GUID, HRESULT, IUnknown, IUnknownImpl, Interface, Ref, Result, implement,
};

static OBJECTS: AtomicUsize = AtomicUsize::new(0);
static LOCKS: AtomicUsize = AtomicUsize::new(0);
pub(crate) fn retain() {
    OBJECTS.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn release() {
    OBJECTS.fetch_sub(1, Ordering::Relaxed);
}
pub(crate) struct Lease;
impl Lease {
    pub fn new() -> Self {
        retain();
        Self
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        release();
    }
}

#[implement(IClassFactory, Agile = false)]
struct Factory;
impl Drop for Factory {
    fn drop(&mut self) {
        release();
    }
}
impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        boundary(|| {
            if out.is_null() {
                return Err(Error::from_hresult(E_POINTER));
            }
            // 安全：COM out 指针非空，由调用者提供可写槽；无论失败与否先清零。
            unsafe {
                *out = std::ptr::null_mut();
            }
            if !outer.is_null() {
                return Err(Error::from_hresult(CLASS_E_NOAGGREGATION));
            }
            if iid.is_null() {
                return Err(Error::from_hresult(E_POINTER));
            }
            let host = Host::new()?;
            let service: ITfTextInputProcessor = Service {
                host,
                cookie: Cell::new(None),
            }
            .into();
            // 安全：iid/out 是有效 COM 参数，query 增加返回接口引用计数。
            unsafe { service.query(iid, out).ok() }
        })
    }
    fn LockServer(&self, lock: BOOL) -> Result<()> {
        boundary(|| {
            if lock.as_bool() {
                LOCKS.fetch_add(1, Ordering::Relaxed);
            } else {
                LOCKS
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
                    .map_err(|_| Error::from_hresult(E_UNEXPECTED))?;
            }
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
/// 返回 Cloudleaf 的 COM 类工厂。
///
/// # Safety
/// 非空 class/iid 必须指向有效 GUID，out 必须指向可写 COM 接口槽；调用者遵守 COM ABI。
pub unsafe extern "system" fn DllGetClassObject(
    class: *const GUID,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    boundary(|| {
        if out.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }
        // 安全：导出遵循 COM DllGetClassObject ABI，空参数被拒绝。
        unsafe {
            *out = std::ptr::null_mut();
            if class.is_null() || iid.is_null() {
                return Err(Error::from_hresult(E_POINTER));
            }
            if *class != SERVICE_ID {
                return Err(Error::from_hresult(CLASS_E_CLASSNOTAVAILABLE));
            }
        }
        retain();
        let factory: IClassFactory = Factory.into();
        // 安全：QueryInterface 验证请求 IID 并保持返回引用。
        unsafe { factory.query(iid, out).ok() }
    })
    .map_or_else(|e| e.code(), |_| S_OK)
}
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECTS.load(Ordering::Acquire) == 0 && LOCKS.load(Ordering::Acquire) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[implement(
    ITfTextInputProcessor,
    ITfKeyEventSink,
    ITfThreadMgrEventSink,
    Agile = false
)]
struct Service {
    host: Shared,
    cookie: Cell<Option<u32>>,
}
impl ITfTextInputProcessor_Impl for Service_Impl {
    fn Activate(&self, manager: Ref<ITfThreadMgr>, client: u32) -> Result<()> {
        boundary(|| {
            self.host.check()?;
            let manager = manager
                .as_ref()
                .ok_or_else(|| Error::from_hresult(E_POINTER))?
                .clone();
            if self.host.manager.borrow().is_some() {
                return Err(Error::from_hresult(E_UNEXPECTED));
            }
            self.host.client.set(client);
            *self.host.manager.borrow_mut() = Some(manager.clone());
            let key: ITfKeyEventSink = self.to_interface();
            let thread: ITfThreadMgrEventSink = self.to_interface();
            let mut key_advised = false;
            // 安全：TSF 调用方已经初始化 apartment；sink 和 manager 同线程并由 TSF 保持引用。
            let activated = unsafe {
                (|| -> Result<()> {
                    let keys = manager.cast::<ITfKeystrokeMgr>()?;
                    keys.AdviseKeyEventSink(client, &key, true)?;
                    key_advised = true;
                    let cookie = manager
                        .cast::<ITfSource>()?
                        .AdviseSink(&ITfThreadMgrEventSink::IID, &thread)?;
                    self.cookie.set(Some(cookie));
                    if let Ok(document) = manager.GetFocus()
                        && let Ok(context) = document.GetTop()
                    {
                        self.host.focus(Some(context))?;
                    }
                    Ok(())
                })()
            };
            if let Err(error) = activated {
                let _ = self.host.invalidate();
                // 安全：每一已完成的注册步骤都尝试独立撤销，不因一个失败跳过后续。
                unsafe {
                    if let Some(cookie) = self.cookie.take()
                        && let Ok(source) = manager.cast::<ITfSource>()
                    {
                        let _ = source.UnadviseSink(cookie);
                    }
                    if key_advised && let Ok(keys) = manager.cast::<ITfKeystrokeMgr>() {
                        let _ = keys.UnadviseKeyEventSink(client);
                    }
                }
                self.host.manager.borrow_mut().take();
                self.host.client.set(0);
                self.host
                    .engine
                    .borrow_mut()
                    .handle(cloudleaf_types::InputEvent::Deactivate);
                return Err(error);
            }
            Ok(())
        })
    }
    fn Deactivate(&self) -> Result<()> {
        boundary(|| {
            self.host.check()?;
            let mut cleanup = self.host.invalidate();
            let manager = self.host.manager.borrow_mut().take();
            if let Some(manager) = manager {
                // 安全：先失效会话后撤销 sink，不再让迟到通知访问已卸载状态。
                unsafe {
                    if let Some(cookie) = self.cookie.take() {
                        let result = manager
                            .cast::<ITfSource>()
                            .and_then(|source| source.UnadviseSink(cookie));
                        if cleanup.is_ok() {
                            cleanup = result;
                        }
                    }
                    let result = manager
                        .cast::<ITfKeystrokeMgr>()
                        .and_then(|keys| keys.UnadviseKeyEventSink(self.host.client.get()));
                    if cleanup.is_ok() {
                        cleanup = result;
                    }
                }
            }
            self.host
                .engine
                .borrow_mut()
                .handle(cloudleaf_types::InputEvent::Deactivate);
            self.host.client.set(0);
            cleanup
        })
    }
}
impl ITfKeyEventSink_Impl for Service_Impl {
    fn OnSetFocus(&self, foreground: BOOL) -> Result<()> {
        boundary(|| {
            self.host.check()?;
            if !foreground.as_bool() {
                self.host.invalidate()?;
            } else {
                self.sync_focus()?;
            }
            Ok(())
        })
    }
    fn OnTestKeyDown(&self, context: Ref<ITfContext>, w: WPARAM, l: LPARAM) -> Result<BOOL> {
        boundary(|| {
            self.host.check()?;
            let Some(context) = context.as_ref() else {
                return Ok(false.into());
            };
            // 安全：GetStatus 不需编辑锁，预判只读宿主时不接管按键。
            if unsafe { context.GetStatus()?.dwDynamicFlags & TS_SD_READONLY != 0 } {
                return Ok(false.into());
            }
            match keys::normalize(w, l) {
                Some(key) => Ok(self.host.disposition(key)?.into()),
                None => Ok(false.into()),
            }
        })
    }
    fn OnTestKeyUp(&self, _context: Ref<ITfContext>, _w: WPARAM, _l: LPARAM) -> Result<BOOL> {
        boundary(|| {
            self.host.check()?;
            Ok(false.into())
        })
    }
    fn OnKeyDown(&self, context: Ref<ITfContext>, w: WPARAM, l: LPARAM) -> Result<BOOL> {
        boundary(|| {
            let Some(key) = keys::normalize(w, l) else {
                return Ok(false.into());
            };
            let context = context
                .as_ref()
                .ok_or_else(|| Error::from_hresult(E_POINTER))?
                .clone();
            self.host.focus(Some(context))?;
            Ok(self.host.key(key)?.into())
        })
    }
    fn OnKeyUp(&self, _context: Ref<ITfContext>, _w: WPARAM, _l: LPARAM) -> Result<BOOL> {
        boundary(|| {
            self.host.check()?;
            Ok(false.into())
        })
    }
    fn OnPreservedKey(&self, _context: Ref<ITfContext>, _guid: *const GUID) -> Result<BOOL> {
        boundary(|| {
            self.host.check()?;
            Ok(false.into())
        })
    }
}
impl ITfThreadMgrEventSink_Impl for Service_Impl {
    fn OnInitDocumentMgr(&self, _document: Ref<ITfDocumentMgr>) -> Result<()> {
        boundary(|| self.host.check())
    }
    fn OnUninitDocumentMgr(&self, _document: Ref<ITfDocumentMgr>) -> Result<()> {
        boundary(|| self.host.check())
    }
    fn OnSetFocus(
        &self,
        document: Ref<ITfDocumentMgr>,
        _previous: Ref<ITfDocumentMgr>,
    ) -> Result<()> {
        boundary(|| {
            // 安全：文档来自 TSF 当前线程，GetTop 失败表示当前不可输入。
            let context = document.as_ref().and_then(|d| unsafe { d.GetTop().ok() });
            self.host.focus(context)
        })
    }
    fn OnPushContext(&self, _context: Ref<ITfContext>) -> Result<()> {
        boundary(|| self.sync_focus())
    }
    fn OnPopContext(&self, _context: Ref<ITfContext>) -> Result<()> {
        boundary(|| self.sync_focus())
    }
}
impl Service_Impl {
    fn sync_focus(&self) -> Result<()> {
        self.host.check()?;
        let manager = self.host.manager.borrow().clone();
        // 安全：只从真实焦点文档查询当前顶部 context，不采用无关 push/pop 参数。
        let context =
            manager.and_then(|manager| unsafe { manager.GetFocus().and_then(|d| d.GetTop()).ok() });
        self.host.focus(context)
    }
}
