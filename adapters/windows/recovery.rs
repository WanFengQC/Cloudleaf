//! STA 自有恢复入口与无文本监测通知；只在用户选择后显示/复制临时材料。
use super::{boundary, service::Lease, session::Host, watchdog};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use windows::Win32::{
    Foundation::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::EM_SETSEL, Input::KeyboardAndMouse::SetFocus, WindowsAndMessaging::*},
};
use windows_core::{Error, PCWSTR, Result};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}
struct View {
    host: Weak<Host>,
    window: Cell<HWND>,
    material: RefCell<Vec<RecoveryBatch>>,
    edit: Cell<HWND>,
    _lease: Lease,
}
struct RecoveryBatch {
    epoch: u64,
    text: String,
}
const MATERIAL_LIMIT: usize = 1024 * 1024;
const CONTEXT_LIMIT: usize = 32;
pub(crate) struct RecoveryWindow {
    view: Rc<View>,
    class: Vec<u16>,
    instance: HINSTANCE,
}
static NEXT_WINDOW: AtomicU64 = AtomicU64::new(1);
thread_local! { static RETIRED: RefCell<Vec<(Rc<RecoveryWindow>, Arc<AtomicBool>)>> = const { RefCell::new(Vec::new()) }; }
pub(crate) fn retire(window: Rc<RecoveryWindow>, completion: Arc<AtomicBool>) {
    RETIRED.with(|retired| retired.borrow_mut().push((window, completion)));
}
pub(crate) fn reap() {
    // 只在创建窗口的 STA 释放已确认无后台通知调用的窗口；别的线程不会触碰这些 Rc。
    RETIRED.with(|retired| {
        let previous = std::mem::take(&mut *retired.borrow_mut());
        let (remaining, released): (Vec<_>, Vec<_>) =
            previous.into_iter().partition(|(window, complete)| {
                !complete.load(Ordering::Acquire) || window.has_material()
            });
        *retired.borrow_mut() = remaining;
        // Drop 会同步进入窗口过程，不能跨外调持有 thread_local 的可变借用。
        drop(released);
    });
}
impl RecoveryWindow {
    pub fn new(host: Weak<Host>, identity: u64) -> Result<Self> {
        let view = Rc::new(View {
            host,
            window: Cell::new(HWND::default()),
            material: RefCell::new(Vec::new()),
            edit: Cell::new(HWND::default()),
            _lease: Lease::new(),
        });
        let generation = NEXT_WINDOW
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| Error::from_hresult(E_FAIL))?;
        let class = wide(&format!("CloudleafRecovery_{identity}_{generation}"));
        // 安全：类名与 userdata 由本对象持有至同步销毁窗口之后。
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None)? }.0);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        unsafe {
            if RegisterClassW(&wc) == 0 {
                return Err(Error::from_thread());
            }
            let window = match CreateWindowExW(
                WS_EX_TOOLWINDOW,
                PCWSTR(class.as_ptr()),
                PCWSTR(wide("云笺：未完成输入，请查看恢复材料").as_ptr()),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                640,
                440,
                None,
                None,
                Some(instance),
                Some(Rc::as_ptr(&view).cast()),
            ) {
                Ok(window) => window,
                Err(error) => {
                    let _ = UnregisterClassW(PCWSTR(class.as_ptr()), Some(instance));
                    return Err(error);
                }
            };
            view.window.set(window);
        }
        Ok(Self {
            view,
            class,
            instance,
        })
    }
    pub fn handle(&self) -> HWND {
        self.view.window.get()
    }
    pub fn has_material(&self) -> bool {
        !self.view.material.borrow().is_empty()
    }
    pub fn can_offer(&self) -> bool {
        let material = self.view.material.borrow();
        material.len() < CONTEXT_LIMIT
            && material.iter().map(|batch| batch.text.len()).sum::<usize>() < MATERIAL_LIMIT
    }
    pub fn offer(&self, epoch: u64, material: String) -> bool {
        let mut batches = self.view.material.borrow_mut();
        let existing = batches.iter().position(|batch| batch.epoch == epoch);
        let previous = existing.map_or(0, |index| batches[index].text.len());
        let total = batches.iter().map(|batch| batch.text.len()).sum::<usize>();
        if epoch == 0
            || material.is_empty()
            || material.len() > MATERIAL_LIMIT
            || total - previous > MATERIAL_LIMIT - material.len()
            || (existing.is_none() && batches.len() >= CONTEXT_LIMIT)
        {
            // 原材料保持完整；调用方必须停止接管并继续持有尚未移交的 ledger。
            return false;
        }
        if let Some(index) = existing {
            batches[index].text = material;
        } else {
            batches.push(RecoveryBatch {
                epoch,
                text: material,
            });
        }
        drop(batches);
        // 安全：不激活宿主以外窗口，不等待用户；默认控件仅说明状态，不直接暴露原文。
        unsafe {
            let _ = SetWindowTextW(self.view.edit.get(), windows_core::w!(""));
            let _ = ShowWindow(self.view.window.get(), SW_SHOWNOACTIVATE);
        }
        true
    }
}
impl Drop for RecoveryWindow {
    fn drop(&mut self) {
        // 安全：先同步销毁窗口，再释放 userdata 与注销类。
        unsafe {
            if !self.view.window.get().is_invalid() {
                let _ = DestroyWindow(self.view.window.get());
            }
            let _ = UnregisterClassW(PCWSTR(self.class.as_ptr()), Some(self.instance));
        }
    }
}
unsafe extern "system" fn window_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    boundary(|| {
        // 安全：userdata 是 RecoveryWindow 持有的 Rc 借用；临时强引用保护重入销毁。
        unsafe {
            if message == WM_NCCREATE { let create = &*(l.0 as *const CREATESTRUCTW); SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize); }
            let ptr = GetWindowLongPtrW(window, GWLP_USERDATA) as *const View;
            if ptr.is_null() { return Ok(DefWindowProcW(window, message, w, l)); }
            Rc::increment_strong_count(ptr);
            let view = Rc::from_raw(ptr);
            match message {
                WM_CREATE => {
                    CreateWindowExW(WINDOW_EX_STYLE::default(), windows_core::w!("STATIC"), PCWSTR(wide("输入已暂停。未确认事件保留在内存；查看后可复制自行核对，结果未知不会自动重试。").as_ptr()), WS_CHILD | WS_VISIBLE, 12, 12, 600, 48, Some(window), None, None, None)?;
                    CreateWindowExW(WINDOW_EX_STYLE::default(), windows_core::w!("BUTTON"), PCWSTR(wide("查看恢复材料").as_ptr()), WS_CHILD | WS_VISIBLE, 12, 68, 150, 30, Some(window), Some(HMENU(std::ptr::dangling_mut())), None, None)?;
                    CreateWindowExW(WINDOW_EX_STYLE::default(), windows_core::w!("BUTTON"), PCWSTR(wide("选择全部（Ctrl+C复制）").as_ptr()), WS_CHILD | WS_VISIBLE, 180, 68, 240, 30, Some(window), Some(HMENU(2usize as *mut _)), None, None)?;
                    let edit = CreateWindowExW(WS_EX_CLIENTEDGE, windows_core::w!("EDIT"), PCWSTR::null(), WS_CHILD | WS_VISIBLE | WS_VSCROLL | WINDOW_STYLE(ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32), 12, 112, 600, 270, Some(window), None, None, None)?;
                    view.edit.set(edit);
                    Ok(LRESULT(0))
                },
                watchdog::NOTICE => { if let Some(host) = view.host.upgrade() { host.watchdog_notice()?; } Ok(LRESULT(0)) },
                WM_COMMAND if w.0 & 0xffff == 1 => { let material = view.material.borrow().iter().map(|batch| format!("上下文 epoch={}\r\n{}", batch.epoch, batch.text)).collect::<Vec<_>>().join("\r\n---\r\n"); let text = wide(&material); SetWindowTextW(view.edit.get(), PCWSTR(text.as_ptr()))?; Ok(LRESULT(0)) },
                WM_COMMAND if w.0 & 0xffff == 2 => { SendMessageW(view.edit.get(), EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1))); let _ = SetFocus(Some(view.edit.get())); Ok(LRESULT(0)) },
                WM_CLOSE => { let last_epoch = view.material.borrow().last().map(|batch| batch.epoch); if let Some(host) = view.host.upgrade() && last_epoch == Some(host.epoch.get()) && host.quarantined.get() { host.discard_recovery()?; } let _ = ShowWindow(window, SW_HIDE); view.material.borrow_mut().clear(); SetWindowTextW(view.edit.get(), windows_core::w!(""))?; Ok(LRESULT(0)) },
                WM_NCDESTROY => { view.window.set(HWND::default()); SetWindowLongPtrW(window, GWLP_USERDATA, 0); Ok(DefWindowProcW(window, message, w, l)) },
                _ => Ok(DefWindowProcW(window, message, w, l)),
            }
        }
    }).unwrap_or(LRESULT(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

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
        assert_eq!(window.view.material.borrow().len(), CONTEXT_LIMIT);
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
        assert_eq!(window.view.material.borrow()[0].epoch, source_epoch);
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
