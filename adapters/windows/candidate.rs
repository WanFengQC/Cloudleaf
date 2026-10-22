//! 候选内容只来自核心帧，原生窗口不取得输入焦点。
use super::{
    PROFILE_ID, boundary,
    session::{Host, Shared},
};
use cloudleaf_types::InputFrame;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use windows::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{TextServices::*, WindowsAndMessaging::*},
};
use windows_core::{BSTR, Error, GUID, Interface, PCWSTR, Result, implement};

struct View {
    _lease: super::service::Lease,
    host: Weak<Host>,
    context: ITfContext,
    epoch: u64,
    frame: RefCell<InputFrame>,
    selected: Cell<usize>,
    shown: Cell<bool>,
    window: Cell<HWND>,
}
impl View {
    fn valid(&self) -> Result<Shared> {
        let h = self
            .host
            .upgrade()
            .ok_or_else(|| Error::from_hresult(E_FAIL))?;
        h.check()?;
        if h.epoch.get() != self.epoch {
            return Err(Error::from_hresult(E_FAIL));
        }
        Ok(h)
    }
    fn finalize(&self) -> Result<()> {
        let h = self.valid()?;
        let frame = self.frame.borrow();
        let id = frame
            .candidates
            .get(self.selected.get())
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?
            .id
            .clone();
        let revision = frame.revision;
        drop(frame);
        h.select(revision, id)
    }
}

pub(crate) struct CandidateUi {
    view: Rc<View>,
    _element: ITfUIElement,
    manager: ITfUIElementMgr,
    id: u32,
    class: Vec<u16>,
    instance: HINSTANCE,
}
struct BegunElement {
    manager: ITfUIElementMgr,
    id: u32,
    active: bool,
}
impl Drop for BegunElement {
    fn drop(&mut self) {
        if self.active {
            // 安全：成功 Begin 后的每个提前返回都唯一地结束 element。
            unsafe {
                let _ = self.manager.EndUIElement(self.id);
            }
        }
    }
}
impl CandidateUi {
    pub fn new(host: Shared, context: ITfContext, frame: InputFrame) -> Result<Self> {
        let view = Rc::new(View {
            _lease: super::service::Lease::new(),
            epoch: host.epoch.get(),
            host: Rc::downgrade(&host),
            context,
            selected: Cell::new(frame.highlighted.unwrap_or(0)),
            frame: RefCell::new(frame),
            shown: Cell::new(true),
            window: Cell::new(HWND::default()),
        });
        let behavior: ITfCandidateListUIElementBehavior = Element { view: view.clone() }.into();
        let element: ITfUIElement = behavior.into();
        let thread_manager = host
            .manager
            .borrow()
            .clone()
            .ok_or_else(|| Error::from_hresult(E_FAIL))?;
        let manager = thread_manager.cast::<ITfUIElementMgr>()?;
        let mut show = true.into();
        let mut id = 0;
        // 安全：本线程 COM 对象和有效输出变量，BeginUIElement 可以同步调用 element。
        unsafe {
            manager.BeginUIElement(&element, &mut show, &mut id)?;
        }
        let mut begun = BegunElement {
            manager: manager.clone(),
            id,
            active: true,
        };
        view.valid()?;
        view.shown.set(show.as_bool());
        // 安全：只获取当前进程模块，不转移句柄所有权。
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None)? }.0);
        let class: Vec<u16> = format!("CloudleafCandidate_{}_{}", host.owner, id)
            .encode_utf16()
            .chain([0])
            .collect();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: PCWSTR(class.as_ptr()),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
            ..Default::default()
        };
        // 安全：class 字符串活至注销；userdata 借用由本对象保持的 Rc，绝不转移所有权。
        unsafe {
            if RegisterClassW(&wc) == 0 {
                return Err(Error::from_thread());
            }
            let data = Rc::as_ptr(&view);
            let window = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                PCWSTR(class.as_ptr()),
                PCWSTR::null(),
                WS_POPUP | WS_BORDER,
                0,
                0,
                300,
                180,
                None,
                None,
                Some(instance),
                Some(data.cast()),
            );
            match window {
                Ok(w) => view.window.set(w),
                Err(e) => {
                    let _ = UnregisterClassW(PCWSTR(class.as_ptr()), Some(instance));
                    return Err(e);
                }
            }
        }
        begun.active = false;
        Ok(Self {
            view,
            _element: element,
            manager,
            id,
            class,
            instance,
        })
    }
    pub fn update(&mut self, frame: InputFrame, rect: RECT) -> Result<()> {
        self.view.valid()?;
        self.view.selected.set(frame.highlighted.unwrap_or(0));
        *self.view.frame.borrow_mut() = frame;
        let height = (self.view.frame.borrow().page_size as i32) * 30 + 8;
        // 安全：window 在本对象拥有的生命周期内有效；不激活、不改变宿主焦点。
        unsafe {
            self.manager.UpdateUIElement(self.id)?;
            self.view.valid()?;
            let _ = SetWindowPos(
                self.view.window.get(),
                Some(HWND_TOPMOST),
                rect.left,
                rect.bottom,
                360,
                height,
                SWP_NOACTIVATE,
            );
            let _ = InvalidateRect(Some(self.view.window.get()), None, true);
            let _ = ShowWindow(
                self.view.window.get(),
                if self.view.shown.get() {
                    SW_SHOWNOACTIVATE
                } else {
                    SW_HIDE
                },
            );
        }
        Ok(())
    }
    pub fn close(&mut self) {
        // 安全：先销毁窗口释放 userdata，再注销独占 class，最后结束 UI element。
        unsafe {
            if !self.view.window.get().is_invalid() {
                let _ = DestroyWindow(self.view.window.replace(HWND::default()));
            }
            let _ = UnregisterClassW(PCWSTR(self.class.as_ptr()), Some(self.instance));
            let _ = self.manager.EndUIElement(self.id);
        }
    }
}
impl Drop for CandidateUi {
    fn drop(&mut self) {
        if !self.view.window.get().is_invalid() {
            self.close();
        }
    }
}

#[implement(ITfCandidateListUIElementBehavior, Agile = false)]
struct Element {
    view: Rc<View>,
}
impl ITfUIElement_Impl for Element_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        boundary(|| {
            self.view.valid()?;
            Ok(BSTR::from("云笺 Cloudleaf 原型候选"))
        })
    }
    fn GetGUID(&self) -> Result<GUID> {
        boundary(|| {
            self.view.valid()?;
            Ok(PROFILE_ID)
        })
    }
    fn Show(&self, show: windows_core::BOOL) -> Result<()> {
        boundary(|| {
            self.view.valid()?;
            self.view.shown.set(show.as_bool());
            // 安全：窗口由本线程创建，ShowWindow 不获取焦点。
            unsafe {
                let _ = ShowWindow(
                    self.view.window.get(),
                    if show.as_bool() {
                        SW_SHOWNOACTIVATE
                    } else {
                        SW_HIDE
                    },
                );
            }
            Ok(())
        })
    }
    fn IsShown(&self) -> Result<windows_core::BOOL> {
        boundary(|| {
            self.view.valid()?;
            Ok(self.view.shown.get().into())
        })
    }
}
impl ITfCandidateListUIElement_Impl for Element_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        boundary(|| {
            self.view.valid()?;
            Ok(TF_CLUIE_COUNT
                | TF_CLUIE_STRING
                | TF_CLUIE_SELECTION
                | TF_CLUIE_PAGEINDEX
                | TF_CLUIE_CURRENTPAGE
                | TF_CLUIE_DOCUMENTMGR)
        })
    }
    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        boundary(|| {
            self.view.valid()?;
            // 安全：同 apartment 的 context 接口仍由 View 持有。
            unsafe { self.view.context.GetDocumentMgr() }
        })
    }
    fn GetCount(&self) -> Result<u32> {
        boundary(|| {
            self.view.valid()?;
            Ok(self.view.frame.borrow().candidates.len() as u32)
        })
    }
    fn GetSelection(&self) -> Result<u32> {
        boundary(|| {
            self.view.valid()?;
            Ok(self.view.selected.get() as u32)
        })
    }
    fn GetString(&self, index: u32) -> Result<BSTR> {
        boundary(|| {
            self.view.valid()?;
            self.view
                .frame
                .borrow()
                .candidates
                .get(index as usize)
                .map(|c| BSTR::from(c.text.as_str()))
                .ok_or_else(|| Error::from_hresult(E_INVALIDARG))
        })
    }
    fn GetPageIndex(&self, index: *mut u32, size: u32, count: *mut u32) -> Result<()> {
        boundary(|| {
            self.view.valid()?;
            if count.is_null() {
                return Err(Error::from_hresult(E_POINTER));
            }
            let f = self.view.frame.borrow();
            let pages = f.candidates.len().div_ceil(f.page_size) as u32;
            // 安全：COM 调用约定指定输出缓冲区至少 size 个元素；先校验可空规则。
            unsafe {
                *count = pages;
                if index.is_null() {
                    return Ok(());
                }
                for n in 0..size.min(pages) {
                    *index.add(n as usize) = n * f.page_size as u32;
                }
            }
            if size < pages {
                return Err(Error::from_hresult(E_INVALIDARG));
            }
            Ok(())
        })
    }
    fn SetPageIndex(&self, index: *const u32, count: u32) -> Result<()> {
        boundary(|| {
            self.view.valid()?;
            let f = self.view.frame.borrow();
            let pages = f.candidates.len().div_ceil(f.page_size) as u32;
            if count != pages || (count != 0 && index.is_null()) {
                return Err(Error::from_hresult(E_INVALIDARG));
            }
            // 安全：读取调用方声明的 count 个索引，固定分页不可被平台重排。
            unsafe {
                for n in 0..count {
                    if *index.add(n as usize) != n * f.page_size as u32 {
                        return Err(Error::from_hresult(E_INVALIDARG));
                    }
                }
            }
            Ok(())
        })
    }
    fn GetCurrentPage(&self) -> Result<u32> {
        boundary(|| {
            self.view.valid()?;
            let f = self.view.frame.borrow();
            Ok((self.view.selected.get() / f.page_size) as u32)
        })
    }
}

impl ITfCandidateListUIElementBehavior_Impl for Element_Impl {
    fn SetSelection(&self, index: u32) -> Result<()> {
        boundary(|| {
            self.view.valid()?;
            if index as usize >= self.view.frame.borrow().candidates.len() {
                return Err(Error::from_hresult(E_INVALIDARG));
            }
            self.view.selected.set(index as usize);
            Ok(())
        })
    }
    fn Finalize(&self) -> Result<()> {
        boundary(|| self.view.finalize())
    }
    fn Abort(&self) -> Result<()> {
        boundary(|| self.view.valid()?.cancel())
    }
}

unsafe extern "system" fn window_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    boundary(|| {
        // 安全：创建消息携带 View 的借用指针，CandidateUi 保证销毁窗口后才释放 Rc。
        unsafe {
            if message == WM_NCCREATE {
                let create = &*(l.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
            }
            let ptr = GetWindowLongPtrW(window, GWLP_USERDATA) as *const View;
            if message == WM_NCDESTROY {
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                return Ok(DefWindowProcW(window, message, w, l));
            }
            if !ptr.is_null() {
                // 安全：窗口存活时 CandidateUi 保有 Rc；额外强引用保护同步 COM 重入销毁窗口的路径。
                Rc::increment_strong_count(ptr);
                let view = Rc::from_raw(ptr);
                if message == WM_MOUSEACTIVATE {
                    return Ok(LRESULT(MA_NOACTIVATE as isize));
                }
                if message == WM_LBUTTONUP {
                    let y = ((l.0 >> 16) & 0xffff) as i16 as i32;
                    let f = view.frame.borrow();
                    let n = f.page_start + (y.saturating_sub(4) / 30) as usize;
                    let valid = n < f.candidates.len();
                    drop(f);
                    if valid {
                        view.selected.set(n);
                        view.finalize()?;
                    }
                    return Ok(LRESULT(0));
                }
                if message == WM_PAINT {
                    let mut paint = PAINTSTRUCT::default();
                    let dc = BeginPaint(window, &mut paint);
                    let frame = view.frame.borrow();
                    SetBkMode(dc, TRANSPARENT);
                    for (n, c) in frame
                        .candidates
                        .iter()
                        .skip(frame.page_start)
                        .take(frame.page_size)
                        .enumerate()
                    {
                        let index = frame.page_start + n;
                        let text = format!(
                            "{}  {}{}",
                            n + 1,
                            c.text,
                            c.annotation
                                .as_ref()
                                .map(|a| format!("  {a}"))
                                .unwrap_or_default()
                        );
                        let mut text: Vec<u16> = text.encode_utf16().collect();
                        let mut rect = RECT {
                            left: 8,
                            top: 4 + n as i32 * 30,
                            right: 352,
                            bottom: 34 + n as i32 * 30,
                        };
                        if index == view.selected.get() {
                            FillRect(dc, &rect, GetSysColorBrush(COLOR_HIGHLIGHT));
                            SetTextColor(dc, COLORREF(GetSysColor(COLOR_HIGHLIGHTTEXT)));
                        } else {
                            SetTextColor(dc, COLORREF(GetSysColor(COLOR_WINDOWTEXT)));
                        }
                        DrawTextW(
                            dc,
                            &mut text,
                            &mut rect,
                            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
                        );
                    }
                    let _ = EndPaint(window, &paint);
                    return Ok(LRESULT(0));
                }
            }
            Ok(DefWindowProcW(window, message, w, l))
        }
    })
    .unwrap_or(LRESULT(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudleaf_types::{InputEvent, Key};
    #[test]
    fn application_owned_ui_can_query_page_count_before_allocating_indices() {
        let host = super::super::session::tests::host(0, false);
        host.engine
            .borrow_mut()
            .handle(InputEvent::Key(Key::Char('n')));
        host.engine
            .borrow_mut()
            .handle(InputEvent::Key(Key::Char('i')));
        let frame = host.engine.borrow().frame();
        let pages = frame.candidates.len().div_ceil(frame.page_size) as u32;
        assert!(pages > 1);
        let view = Rc::new(View {
            _lease: super::super::service::Lease::new(),
            host: Rc::downgrade(&host),
            context: host.context.borrow().clone().unwrap(),
            epoch: host.epoch.get(),
            frame: RefCell::new(frame),
            selected: Cell::new(0),
            shown: Cell::new(false),
            window: Cell::new(HWND::default()),
        });
        let element: ITfCandidateListUIElementBehavior = Element { view }.into();
        let mut count = 0;
        // 安全：count 和分配后的索引数组均由本测试持有；首次合法 NULL 查询必须成功。
        unsafe {
            let list: ITfCandidateListUIElement = element.cast().unwrap();
            (list.vtable().GetPageIndex)(list.as_raw(), std::ptr::null_mut(), 0, &mut count)
                .ok()
                .unwrap();
            assert_eq!(count, pages);
            let mut indices = vec![0; count as usize];
            element.GetPageIndex(&mut indices, &mut count).unwrap();
            assert_eq!(indices, (0..pages).map(|n| n * 5).collect::<Vec<_>>());
            assert!(element.GetPageIndex(&mut [], &mut count).is_err());
        }
    }
}
