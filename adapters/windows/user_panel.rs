//! 隔离输入宿主的用户词库面板；文件和索引操作由调用者的工作线程执行。
use super::*;
use windows::Win32::UI::Controls::EM_SETLIMITTEXT;

const ADD: usize = 101;
const UPDATE: usize = 102;
const DELETE: usize = 103;
const TOGGLE: usize = 104;
const CLEAR_ENTRIES: usize = 105;
const SAVE: usize = 106;
const RELOAD: usize = 107;
const TOGGLE_LEARNING: usize = 108;
const CLEAR_LEARNING: usize = 109;
const CLEAR_ALL: usize = 110;
const EXPORT: usize = 111;
const IMPORT: usize = 112;
const RECOVER: usize = 113;

pub(super) enum Command {
    Add {
        text: String,
        pinyin: String,
    },
    Update {
        id: String,
        text: String,
        pinyin: String,
    },
    Delete {
        id: String,
    },
    Toggle {
        id: String,
    },
    ClearEntries,
    Save,
    Reload,
    ToggleLearning,
    ClearLearning,
    ClearAll,
    Export {
        path: String,
    },
    Import {
        path: String,
    },
    Recover {
        generation: u64,
    },
}

pub(super) struct Panel {
    window: Cell<HWND>,
    id: Cell<HWND>,
    entry_text: Cell<HWND>,
    pinyin: Cell<HWND>,
    path: Cell<HWND>,
    generation: Cell<HWND>,
    listing: Cell<HWND>,
    status: Cell<HWND>,
    callback: Rc<dyn Fn(Command)>,
    class: Vec<u16>,
    instance: HINSTANCE,
    lease_pending: Cell<bool>,
    creation_error: RefCell<Option<Error>>,
}

impl Panel {
    pub(super) fn open(owner: HWND, callback: Rc<dyn Fn(Command)>) -> Result<Rc<Self>> {
        // 安全：所有控件、消息和Rc均只在创建者的UI线程上使用。
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None)?.0);
            let class = wide(&format!(
                "CloudleafUserDictionaryPanel_{:x}",
                owner.0 as usize
            ));
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: PCWSTR(class.as_ptr()),
                hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                ..Default::default()
            };
            if RegisterClassW(&wc) == 0 {
                return Err(Error::from_thread());
            }
            let panel = Rc::new(Self {
                window: Cell::new(HWND::default()),
                id: Cell::new(HWND::default()),
                entry_text: Cell::new(HWND::default()),
                pinyin: Cell::new(HWND::default()),
                path: Cell::new(HWND::default()),
                generation: Cell::new(HWND::default()),
                listing: Cell::new(HWND::default()),
                status: Cell::new(HWND::default()),
                callback,
                class,
                instance,
                lease_pending: Cell::new(true),
                creation_error: RefCell::new(None),
            });
            let lease = Rc::into_raw(panel.clone());
            // 独立顶层窗口可单独聚焦；生命周期由UserState显式管理，保持正常IME。
            let created = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(panel.class.as_ptr()),
                w!("云笺 · 用户词库与学习"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                970,
                830,
                None,
                None,
                Some(instance),
                Some(lease.cast()),
            );
            if let Err(error) = created {
                // NCCREATE未取得所有权时，Win32不会替我们释放该Rc。
                if panel.lease_pending.replace(false) {
                    drop(Rc::from_raw(lease));
                }
                let error = panel.creation_error.borrow_mut().take().unwrap_or(error);
                return Err(error);
            }
            Ok(panel)
        }
    }

    pub(super) fn refresh(&self, listing: &str, status: &str) {
        // 安全：后台结果回到UI线程后才调用，不接收或持有后台线程的HWND操作。
        unsafe {
            let _ = SetWindowTextW(self.listing.get(), PCWSTR(wide(listing).as_ptr()));
            let _ = SetWindowTextW(self.status.get(), PCWSTR(wide(status).as_ptr()));
        }
    }

    pub(super) fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.window.get(), SW_SHOWNORMAL);
            let _ = SetForegroundWindow(self.window.get());
            let _ = SetFocus(Some(self.entry_text.get()));
        }
    }

    pub(super) fn close(&self) {
        if !self.window.get().is_invalid() {
            unsafe {
                let _ = DestroyWindow(self.window.get());
            }
        }
    }

    fn create_controls(&self) -> Result<()> {
        let window = self.window.get();
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("添加词条时填写文字和拼音。修改、停用或删除时，另填写下方列表中的词条 ID。"),
                WS_CHILD | WS_VISIBLE,
                14,
                12,
                920,
                24,
                Some(window),
                None,
                None,
                None,
            )?;
        }
        self.label("词条 ID", 14, 48, 95)?;
        self.id.set(self.edit(110, 44, 820, 26, 128, false)?);
        self.label("文字", 14, 85, 95)?;
        self.entry_text
            .set(self.edit(110, 81, 330, 28, 256, false)?);
        self.label("拼音", 462, 85, 65)?;
        self.pinyin.set(self.edit(530, 81, 400, 28, 512, false)?);
        for (id, label, x, width) in [
            (ADD, "添加", 14, 100),
            (UPDATE, "修改", 124, 100),
            (TOGGLE, "启用 / 停用", 234, 150),
            (DELETE, "删除词条", 394, 130),
            (CLEAR_ENTRIES, "清空词条", 534, 130),
            (SAVE, "保存", 674, 110),
            (RELOAD, "重新加载", 794, 136),
        ] {
            self.button(id, label, x, 124, width)?;
        }
        for (id, label, x, width) in [
            (TOGGLE_LEARNING, "开启 / 关闭学习", 14, 220),
            (CLEAR_LEARNING, "清空学习统计", 248, 180),
            (CLEAR_ALL, "清空词条和学习统计", 442, 240),
        ] {
            self.button(id, label, x, 166, width)?;
        }
        self.label("文件路径", 14, 215, 95)?;
        self.path.set(self.edit(110, 209, 580, 28, 32767, false)?);
        self.button(EXPORT, "导出", 704, 208, 105)?;
        self.button(IMPORT, "导入", 823, 208, 107)?;
        self.label("恢复版本", 14, 255, 95)?;
        self.generation
            .set(self.edit(110, 249, 270, 28, 20, false)?);
        self.button(RECOVER, "恢复此版本", 394, 248, 160)?;
        self.label(
            "输入已保存的版本号；恢复结果和缺件原因见下方状态。",
            568,
            256,
            365,
        )?;
        self.label("词条与学习状态", 14, 295, 916)?;
        self.listing
            .set(self.edit(14, 320, 916, 300, 1_048_576, true)?);
        self.label("操作结果", 14, 633, 916)?;
        self.status.set(self.edit(14, 658, 916, 116, 32767, true)?);
        Ok(())
    }

    fn label(&self, value: &str, x: i32, y: i32, width: i32) -> Result<()> {
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                PCWSTR(wide(value).as_ptr()),
                WS_CHILD | WS_VISIBLE,
                x,
                y,
                width,
                24,
                Some(self.window.get()),
                None,
                None,
                None,
            )?;
        }
        Ok(())
    }

    fn edit(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        limit: usize,
        readonly: bool,
    ) -> Result<HWND> {
        let style = if readonly {
            WS_CHILD
                | WS_VISIBLE
                | WS_VSCROLL
                | WS_HSCROLL
                | WS_TABSTOP
                | WINDOW_STYLE(
                    ES_READONLY as u32
                        | ES_MULTILINE as u32
                        | ES_AUTOVSCROLL as u32
                        | ES_AUTOHSCROLL as u32,
                )
        } else {
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32)
        };
        unsafe {
            let control = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                w!(""),
                style,
                x,
                y,
                width,
                height,
                Some(self.window.get()),
                None,
                None,
                None,
            )?;
            SendMessageW(
                control,
                EM_SETLIMITTEXT,
                Some(WPARAM(limit)),
                Some(LPARAM(0)),
            );
            Ok(control)
        }
    }

    fn button(&self, id: usize, label: &str, x: i32, y: i32, width: i32) -> Result<()> {
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                PCWSTR(wide(label).as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                x,
                y,
                width,
                31,
                Some(self.window.get()),
                Some(HMENU(id as *mut _)),
                None,
                None,
            )?;
        }
        Ok(())
    }

    fn command(&self, id: usize) -> Result<()> {
        let command = match id {
            ADD => Command::Add {
                text: read_field(self.entry_text.get(), 256, "文字")?,
                pinyin: read_field(self.pinyin.get(), 512, "拼音")?,
            },
            UPDATE => Command::Update {
                id: read_field(self.id.get(), 128, "词条 ID")?,
                text: read_field(self.entry_text.get(), 256, "文字")?,
                pinyin: read_field(self.pinyin.get(), 512, "拼音")?,
            },
            DELETE => Command::Delete {
                id: read_field(self.id.get(), 128, "词条 ID")?,
            },
            TOGGLE => Command::Toggle {
                id: read_field(self.id.get(), 128, "词条 ID")?,
            },
            CLEAR_ENTRIES => {
                if !self.confirm("清空所有用户词条？此操作不会清除学习统计。")
                {
                    return Ok(());
                }
                Command::ClearEntries
            }
            SAVE => Command::Save,
            RELOAD => Command::Reload,
            TOGGLE_LEARNING => Command::ToggleLearning,
            CLEAR_LEARNING => {
                if !self.confirm("清空所有学习统计？此操作不会清除用户词条。")
                {
                    return Ok(());
                }
                Command::ClearLearning
            }
            CLEAR_ALL => {
                if !self.confirm("清空所有用户词条和学习统计？") {
                    return Ok(());
                }
                Command::ClearAll
            }
            EXPORT => Command::Export {
                path: read_field(self.path.get(), 32767, "文件路径")?,
            },
            IMPORT => Command::Import {
                path: read_field(self.path.get(), 32767, "文件路径")?,
            },
            RECOVER => Command::Recover {
                generation: read_field(self.generation.get(), 20, "恢复版本")?
                    .parse()
                    .map_err(|_| Error::new(E_INVALIDARG, "恢复版本必须是无符号整数"))?,
            },
            _ => return Ok(()),
        };
        (self.callback)(command);
        Ok(())
    }

    fn confirm(&self, message: &str) -> bool {
        unsafe {
            MessageBoxW(
                Some(self.window.get()),
                PCWSTR(wide(message).as_ptr()),
                w!("确认清空"),
                MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
            ) == IDYES
        }
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        // HWND持有的强引用只在NCDESTROY释放；此处已没有存活窗口。
        unsafe {
            let _ = UnregisterClassW(PCWSTR(self.class.as_ptr()), Some(self.instance));
        }
    }
}

fn read_field(window: HWND, limit: usize, name: &str) -> Result<String> {
    unsafe {
        let size = GetWindowTextLengthW(window) as usize;
        if size == 0 || size > limit {
            return Err(Error::new(
                E_INVALIDARG,
                format!("请填写{name}，长度上限为{limit}个UTF-16单元"),
            ));
        }
        let mut value = vec![0u16; size + 1];
        let read = GetWindowTextW(window, &mut value) as usize;
        let value = String::from_utf16(&value[..read])
            .map_err(|_| Error::new(E_INVALIDARG, format!("{name}含有不完整的Unicode字符")))?;
        if value.trim().is_empty() {
            return Err(Error::new(E_INVALIDARG, format!("请填写{name}")));
        }
        Ok(value)
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // 安全：NCCREATE接收open交出的强引用，NCDESTROY归还；每次消息另持临时强引用。
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            let panel = &*(create.lpCreateParams as *const Panel);
            SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
            panel.window.set(window);
            panel.lease_pending.set(false);
        }
        let ptr = GetWindowLongPtrW(window, GWLP_USERDATA) as *const Panel;
        if ptr.is_null() {
            return DefWindowProcW(window, message, wparam, lparam);
        }
        Rc::increment_strong_count(ptr);
        let panel = Rc::from_raw(ptr);
        if message == WM_NCDESTROY {
            SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            panel.window.set(HWND::default());
            let result = DefWindowProcW(window, message, wparam, lparam);
            drop(Rc::from_raw(ptr));
            return result;
        }
        if message == WM_CREATE {
            return match boundary(|| panel.create_controls()) {
                Ok(()) => LRESULT(0),
                Err(error) => {
                    *panel.creation_error.borrow_mut() = Some(error);
                    LRESULT(-1)
                }
            };
        }
        let result = boundary(|| match message {
            WM_COMMAND if wparam.0 >> 16 == BN_CLICKED as usize && lparam.0 != 0 => {
                panel.command(wparam.0 & 0xffff)?;
                Ok(LRESULT(0))
            }
            WM_CLOSE => {
                let _ = ShowWindow(window, SW_HIDE);
                Ok(LRESULT(0))
            }
            _ => Ok(DefWindowProcW(window, message, wparam, lparam)),
        });
        match result {
            Ok(value) => value,
            Err(error) => {
                let _ = SetWindowTextW(panel.status.get(), PCWSTR(wide(&error.message()).as_ptr()));
                LRESULT(0)
            }
        }
    }
}
