#![allow(unsafe_code)]

use crate::controller::PanelController;
use crate::platform::{WindowsCredentialStore, copy_unicode};
use crate::settings::{DirectionChoice, ProviderChoice, Settings};
use cloudleaf_translation::{QualityStatus, Secret, TranslationError, TranslationKind};
use std::cell::RefCell;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::Arc;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use zeroize::Zeroize;

const INPUT: usize = 101;
const DIRECTION: usize = 102;
const KIND: usize = 103;
const START: usize = 104;
const CANCEL: usize = 105;
const CLEAR: usize = 106;
const COPY: usize = 107;
const DETAILS: usize = 108;
const RESULT: usize = 109;
const STATUS: usize = 110;
const PROVIDER: usize = 201;
const DICTIONARY: usize = 202;
const ENDPOINT: usize = 203;
const MODEL: usize = 204;
const CONNECT: usize = 205;
const TOTAL: usize = 206;
const CACHE: usize = 207;
const NETWORK: usize = 208;
const LOOPBACK: usize = 209;
const CONSENT: usize = 210;
const KEY: usize = 211;
const SAVE_KEY: usize = 212;
const DELETE_KEY: usize = 213;
const APPLY: usize = 214;
const RESET: usize = 215;

struct Control {
    hwnd: HWND,
    id: usize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    right: bool,
}
struct WindowState {
    controller: PanelController,
    controls: Vec<Control>,
    parent: HWND,
    font: HFONT,
    dpi: u32,
    serial: u64,
    last_result: String,
    last_status: String,
    closing: bool,
}

pub fn show_error(text: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(text).as_ptr(),
            wide("Cloudleaf").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

pub fn run(directory: PathBuf, dictionary: Option<PathBuf>) -> Result<(), String> {
    let controller = PanelController::new(
        directory,
        dictionary,
        Arc::new(WindowsCredentialStore::default()),
    )
    .map_err(|_| "无法启动后台设置线程".to_owned())?;
    let state = Box::new(RefCell::new(WindowState {
        controller,
        controls: Vec::new(),
        parent: null_mut(),
        font: null_mut(),
        dpi: 96,
        serial: u64::MAX,
        last_result: String::new(),
        last_status: String::new(),
        closing: false,
    }));
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        let class_name = wide("CloudleafTranslationPanelRound12");
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            ..std::mem::zeroed()
        };
        if RegisterClassW(&class) == 0 {
            return Err("窗口类注册失败".into());
        }
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide("云笺 Cloudleaf · 翻译面板").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1120,
            850,
            null_mut(),
            null_mut(),
            instance,
            &*state as *const RefCell<WindowState> as *const _,
        );
        if hwnd.is_null() {
            UnregisterClassW(class_name.as_ptr(), instance);
            return Err("窗口创建失败".into());
        }
        let dpi = GetDpiForWindow(hwnd).max(96) as i32;
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            1120 * dpi / 96,
            850 * dpi / 96,
            SWP_NOMOVE | SWP_NOZORDER,
        );
        ShowWindow(hwnd, SW_SHOW);
        let mut message: MSG = std::mem::zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result == 0 {
                break;
            }
            if result == -1 {
                DestroyWindow(hwnd);
                return Err("窗口消息读取失败".into());
            }
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if !state.borrow().font.is_null() {
            DeleteObject(state.borrow().font);
        }
        UnregisterClassW(class_name.as_ptr(), instance);
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<WindowState>;
        if message == WM_NCDESTROY {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        if !pointer.is_null() {
            // 原生控件可同步发通知，内部更新时不重入可变借用。
            if let Ok(mut state) = (*pointer).try_borrow_mut() {
                match message {
                    WM_CREATE => {
                        state.create_controls(hwnd);
                        state.set_font(hwnd);
                        state.layout(hwnd);
                        SetTimer(hwnd, 1, 50, None);
                        return 0;
                    }
                    WM_SIZE => {
                        state.layout(hwnd);
                        return 0;
                    }
                    WM_DPICHANGED => {
                        let rect = &*(lparam as *const RECT);
                        SetWindowPos(
                            hwnd,
                            null_mut(),
                            rect.left,
                            rect.top,
                            rect.right - rect.left,
                            rect.bottom - rect.top,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                        state.set_font(hwnd);
                        state.layout(hwnd);
                        return 0;
                    }
                    WM_GETMINMAXINFO => {
                        let info = &mut *(lparam as *mut MINMAXINFO);
                        info.ptMinTrackSize = POINT {
                            x: state.scale(990),
                            y: state.scale(790),
                        };
                        return 0;
                    }
                    WM_COMMAND => {
                        state.command(hwnd, wparam);
                        state.render();
                        return 0;
                    }
                    WM_ACTIVATE => {
                        state
                            .controller
                            .focus((wparam & 0xffff) != WA_INACTIVE as usize);
                        return 0;
                    }
                    WM_CLOSE => {
                        state.closing = true;
                        state.controller.begin_close();
                        SetWindowTextW(state.control(KEY), wide("").as_ptr());
                        state.render();
                        return 0;
                    }
                    WM_TIMER => {
                        state.controller.tick();
                        state.render();
                        if state.closing
                            && (state.controller.close_ready()
                                || state.controller.close_deadline_reached())
                        {
                            KillTimer(hwnd, 1);
                            DestroyWindow(hwnd);
                        }
                        return 0;
                    }
                    WM_DESTROY => {
                        PostQuitMessage(0);
                        return 0;
                    }
                    _ => {}
                }
            } else if message == WM_DESTROY {
                PostQuitMessage(0);
                return 0;
            }
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }
}

impl WindowState {
    unsafe fn create_controls(&mut self, hwnd: HWND) {
        self.parent = hwnd;
        unsafe {
            self.add(0, "STATIC", "翻译工作区", 0, [16, 16, 600, 28], false);
            self.add(
                0,
                "STATIC",
                "原文 · 最多 16 KiB UTF-8",
                0,
                [16, 54, 600, 22],
                false,
            );
            self.add(
                INPUT,
                "EDIT",
                "",
                ES_MULTILINE as u32
                    | ES_AUTOVSCROLL as u32
                    | ES_WANTRETURN as u32
                    | WS_VSCROLL
                    | WS_BORDER,
                [16, 80, 600, 165],
                false,
            );
            SendMessageW(self.control(INPUT), EM_SETLIMITTEXT, 16384, 0);
            self.add(
                DIRECTION,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
                [16, 257, 300, 180],
                false,
            );
            for text in [
                "中文 → 英文",
                "英文 → 中文",
                "自动检测 → 英文",
                "自动检测 → 中文",
            ] {
                self.item(DIRECTION, text);
            }
            self.add(
                KIND,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32,
                [330, 257, 130, 150],
                false,
            );
            for text in ["单词", "短语", "整句"] {
                self.item(KIND, text);
            }
            SendMessageW(self.control(KIND), CB_SETCURSEL, 0, 0);
            for (id, text, x, width) in [
                (START, "开始 / 重译", 16, 110),
                (CANCEL, "取消", 136, 70),
                (CLEAR, "清空", 216, 70),
                (COPY, "复制结果", 296, 95),
                (DETAILS, "错误详情", 401, 95),
            ] {
                self.add(
                    id,
                    "BUTTON",
                    text,
                    BS_PUSHBUTTON as u32,
                    [x, 301, width, 32],
                    false,
                );
            }
            self.add(
                0,
                "STATIC",
                "结果 · 研究释义不等同于正式翻译",
                0,
                [16, 347, 600, 22],
                false,
            );
            self.add(
                RESULT,
                "EDIT",
                "",
                ES_MULTILINE as u32
                    | ES_READONLY as u32
                    | ES_AUTOVSCROLL as u32
                    | WS_VSCROLL
                    | WS_BORDER,
                [16, 375, 600, 340],
                false,
            );
            self.add(
                STATUS,
                "STATIC",
                "正在初始化…",
                0,
                [16, 733, 1000, 46],
                false,
            );
            self.add(0, "STATIC", "提供者与设置", 0, [0, 16, 400, 28], true);
            self.add(
                PROVIDER,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32,
                [0, 54, 400, 140],
                true,
            );
            for text in ["本地研究词典", "用户自定义在线 API", "离线模型（未安装）"]
            {
                self.item(PROVIDER, text);
            }
            self.add(
                0,
                "STATIC",
                "研究词典 catalog.json 路径",
                0,
                [0, 96, 400, 22],
                true,
            );
            self.add(
                DICTIONARY,
                "EDIT",
                "",
                ES_AUTOHSCROLL as u32 | WS_BORDER,
                [0, 122, 400, 27],
                true,
            );
            self.add(
                0,
                "STATIC",
                "API 完整端点（HTTPS；不提供默认服务）",
                0,
                [0, 161, 400, 22],
                true,
            );
            self.add(
                ENDPOINT,
                "EDIT",
                "",
                ES_AUTOHSCROLL as u32 | WS_BORDER,
                [0, 187, 400, 27],
                true,
            );
            self.add(0, "STATIC", "模型名称", 0, [0, 226, 400, 22], true);
            self.add(
                MODEL,
                "EDIT",
                "",
                ES_AUTOHSCROLL as u32 | WS_BORDER,
                [0, 252, 400, 27],
                true,
            );
            self.add(
                0,
                "STATIC",
                "连接超时(秒)                 翻译总超时(秒)",
                0,
                [0, 291, 400, 22],
                true,
            );
            self.add(
                CONNECT,
                "EDIT",
                "5",
                ES_NUMBER as u32 | WS_BORDER,
                [0, 317, 90, 27],
                true,
            );
            self.add(
                TOTAL,
                "EDIT",
                "15",
                ES_NUMBER as u32 | WS_BORDER,
                [210, 317, 90, 27],
                true,
            );
            for (id, text, y) in [
                (CACHE, "启用内存缓存（不写输入历史）", 360),
                (NETWORK, "启用联网（重启后恢复离线）", 394),
                (LOOPBACK, "允许本机 HTTP 服务（仅 loopback）", 428),
                (CONSENT, "允许本次发送到已应用的上方 API 端点", 462),
            ] {
                self.add(
                    id,
                    "BUTTON",
                    text,
                    BS_AUTOCHECKBOX as u32,
                    [0, y, 400, 28],
                    true,
                );
            }
            self.add(
                0,
                "STATIC",
                "API Key · 仅存 Windows 当前用户凭据管理器",
                0,
                [0, 505, 400, 22],
                true,
            );
            self.add(
                KEY,
                "EDIT",
                "",
                ES_PASSWORD as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER,
                [0, 531, 400, 27],
                true,
            );
            SendMessageW(self.control(KEY), EM_SETLIMITTEXT, 8192, 0);
            self.add(
                SAVE_KEY,
                "BUTTON",
                "保存 / 更新密钥",
                BS_PUSHBUTTON as u32,
                [0, 570, 160, 32],
                true,
            );
            self.add(
                DELETE_KEY,
                "BUTTON",
                "删除该端点密钥",
                BS_PUSHBUTTON as u32,
                [175, 570, 180, 32],
                true,
            );
            self.add(
                APPLY,
                "BUTTON",
                "应用并保存设置",
                BS_PUSHBUTTON as u32,
                [0, 624, 180, 34],
                true,
            );
            self.add(
                RESET,
                "BUTTON",
                "恢复默认设置",
                BS_PUSHBUTTON as u32,
                [195, 624, 180, 34],
                true,
            );
            self.add(
                0,
                "STATIC",
                "本程序不向其他应用自动输入；离线模型未安装。",
                0,
                [0, 676, 400, 40],
                true,
            );
        }
    }

    unsafe fn add(
        &mut self,
        id: usize,
        class: &str,
        text: &str,
        style: u32,
        rect: [i32; 4],
        right: bool,
    ) {
        unsafe {
            let hwnd = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | if id != 0 { WS_TABSTOP } else { 0 } | style,
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                self.parent,
                id as HMENU,
                GetModuleHandleW(null()),
                null(),
            );
            self.controls.push(Control {
                hwnd,
                id,
                x: rect[0],
                y: rect[1],
                width: rect[2],
                height: rect[3],
                right,
            });
        }
    }

    fn control(&self, id: usize) -> HWND {
        self.controls
            .iter()
            .find(|c| c.id == id)
            .map_or(null_mut(), |c| c.hwnd)
    }
    fn scale(&self, value: i32) -> i32 {
        (value * self.dpi as i32 + 48) / 96
    }

    unsafe fn item(&self, id: usize, text: &str) {
        unsafe {
            SendMessageW(
                self.control(id),
                CB_ADDSTRING,
                0,
                wide(text).as_ptr() as isize,
            );
        }
    }

    unsafe fn set_font(&mut self, hwnd: HWND) {
        unsafe {
            self.dpi = GetDpiForWindow(hwnd).max(96);
            let font = CreateFontW(
                -self.scale(15),
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                0,
                0,
                CLEARTYPE_QUALITY as u32,
                0,
                wide("Microsoft YaHei UI").as_ptr(),
            );
            for c in &self.controls {
                SendMessageW(c.hwnd, WM_SETFONT, font as usize, 1);
            }
            if !self.font.is_null() {
                DeleteObject(self.font);
            }
            self.font = font;
        }
    }

    unsafe fn layout(&self, hwnd: HWND) {
        unsafe {
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let width = rect.right * 96 / self.dpi as i32;
            let height = rect.bottom * 96 / self.dpi as i32;
            let right_x = (width * 58 / 100).max(530);
            let left_width = right_x - 32;
            let right_width = width - right_x - 20;
            for c in &self.controls {
                let x = if c.right { right_x + c.x } else { c.x };
                let y = if c.id == STATUS { height - 52 } else { c.y };
                let w = if c.right && c.width == 400 {
                    right_width
                } else if [INPUT, RESULT].contains(&c.id) {
                    left_width
                } else if c.id == STATUS {
                    width - 32
                } else {
                    c.width
                };
                let h = if c.id == RESULT {
                    (height - 445).max(150)
                } else {
                    c.height
                };
                MoveWindow(
                    c.hwnd,
                    self.scale(x),
                    self.scale(y),
                    self.scale(w),
                    self.scale(h),
                    1,
                );
            }
        }
    }

    unsafe fn settings_from_controls(&self) -> Result<Settings, TranslationError> {
        unsafe {
            let mut settings = self.controller.settings.clone();
            settings.provider = match SendMessageW(self.control(PROVIDER), CB_GETCURSEL, 0, 0) {
                1 => ProviderChoice::Online,
                2 => ProviderChoice::OfflineModel,
                _ => ProviderChoice::Dictionary,
            };
            let path = read_text(self.control(DICTIONARY), 4096)?;
            settings.dictionary_path = if path.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(path))
            };
            settings.endpoint = read_text(self.control(ENDPOINT), 2048)?;
            settings.model = read_text(self.control(MODEL), 256)?;
            settings.connect_timeout_secs = read_text(self.control(CONNECT), 8)?
                .parse()
                .map_err(|_| TranslationError::InvalidConfig)?;
            settings.total_timeout_secs = read_text(self.control(TOTAL), 8)?
                .parse()
                .map_err(|_| TranslationError::InvalidConfig)?;
            settings.direction = self.controller.direction;
            settings.cache_enabled = self.checked(CACHE);
            settings.network_enabled = self.checked(NETWORK);
            settings.allow_loopback_http = self.checked(LOOPBACK);
            Ok(settings)
        }
    }

    unsafe fn checked(&self, id: usize) -> bool {
        unsafe { SendMessageW(self.control(id), BM_GETCHECK, 0, 0) == BST_CHECKED as isize }
    }

    unsafe fn command(&mut self, hwnd: HWND, wparam: WPARAM) {
        unsafe {
            if self.closing {
                return;
            }
            let id = wparam & 0xffff;
            let notification = (wparam >> 16) as u32;
            if id == INPUT && notification == EN_CHANGE {
                match read_text(self.control(INPUT), 16384) {
                    Ok(text) => self.controller.set_text(text),
                    Err(e) => self.controller.record_error(e),
                }
                return;
            }
            if [DICTIONARY, ENDPOINT, MODEL, CONNECT, TOTAL].contains(&id)
                && notification == EN_CHANGE
            {
                self.controller.mark_settings_dirty();
                return;
            }
            if notification == CBN_SELCHANGE {
                match id {
                    DIRECTION => self
                        .controller
                        .set_direction(selected_direction(SendMessageW(
                            self.control(id),
                            CB_GETCURSEL,
                            0,
                            0,
                        ))),
                    KIND => self.controller.set_kind(
                        match SendMessageW(self.control(id), CB_GETCURSEL, 0, 0) {
                            1 => TranslationKind::Phrase,
                            2 => TranslationKind::Sentence,
                            _ => TranslationKind::Word,
                        },
                    ),
                    PROVIDER => self.controller.mark_settings_dirty(),
                    _ => {}
                }
                return;
            }
            if notification != BN_CLICKED {
                return;
            }
            let result = match id {
                START => self.controller.start(),
                CANCEL => {
                    self.controller.cancel();
                    Ok(())
                }
                CLEAR => {
                    self.controller.set_text(String::new());
                    SetWindowTextW(self.control(INPUT), wide("").as_ptr());
                    Ok(())
                }
                COPY => match self.controller.copy_text() {
                    Ok(text) => {
                        if let Err(e) = copy_unicode(hwnd as isize, &text) {
                            self.controller.platform_error(e);
                        }
                        Ok(())
                    }
                    Err(e) => Err(e),
                },
                DETAILS => {
                    let text = self
                        .controller
                        .error
                        .as_deref()
                        .unwrap_or("没有当前错误。错误信息不会显示密钥或输入正文。");
                    MessageBoxW(
                        hwnd,
                        wide(text).as_ptr(),
                        wide("错误详情").as_ptr(),
                        MB_OK | MB_ICONINFORMATION,
                    );
                    Ok(())
                }
                APPLY => self
                    .settings_from_controls()
                    .and_then(|s| self.controller.apply(s)),
                RESET => self.controller.apply(Settings::default()),
                CACHE | NETWORK | LOOPBACK => {
                    self.controller.mark_settings_dirty();
                    Ok(())
                }
                CONSENT => {
                    self.controller.authorized = self.checked(CONSENT) && !self.controller.dirty;
                    Ok(())
                }
                SAVE_KEY => {
                    let settings = self.settings_from_controls();
                    let value = read_text(self.control(KEY), 8192);
                    SetWindowTextW(self.control(KEY), wide("").as_ptr());
                    SendMessageW(self.control(KEY), EM_EMPTYUNDOBUFFER, 0, 0);
                    match value {
                        Ok(value) => Secret::new(value).and_then(|secret| {
                            settings.and_then(|s| self.controller.save_key(s, secret))
                        }),
                        Err(e) => Err(e),
                    }
                }
                DELETE_KEY => self
                    .settings_from_controls()
                    .and_then(|s| self.controller.delete_key(s)),
                _ => Ok(()),
            };
            if let Err(error) = result {
                self.controller.record_error(error);
            }
        }
    }

    unsafe fn render(&mut self) {
        unsafe {
            if self.serial != self.controller.settings_serial {
                self.serial = self.controller.settings_serial;
                let s = &self.controller.settings;
                SendMessageW(
                    self.control(PROVIDER),
                    CB_SETCURSEL,
                    match s.provider {
                        ProviderChoice::Dictionary => 0,
                        ProviderChoice::Online => 1,
                        ProviderChoice::OfflineModel => 2,
                    },
                    0,
                );
                SetWindowTextW(
                    self.control(DICTIONARY),
                    wide(
                        s.dictionary_path
                            .as_ref()
                            .and_then(|p| p.to_str())
                            .unwrap_or(""),
                    )
                    .as_ptr(),
                );
                SetWindowTextW(self.control(ENDPOINT), wide(&s.endpoint).as_ptr());
                SetWindowTextW(self.control(MODEL), wide(&s.model).as_ptr());
                SetWindowTextW(
                    self.control(CONNECT),
                    wide(&s.connect_timeout_secs.to_string()).as_ptr(),
                );
                SetWindowTextW(
                    self.control(TOTAL),
                    wide(&s.total_timeout_secs.to_string()).as_ptr(),
                );
                for (id, checked) in [
                    (CACHE, s.cache_enabled),
                    (NETWORK, s.network_enabled),
                    (LOOPBACK, s.allow_loopback_http),
                ] {
                    SendMessageW(
                        self.control(id),
                        BM_SETCHECK,
                        if checked { BST_CHECKED } else { BST_UNCHECKED } as usize,
                        0,
                    );
                }
                SendMessageW(self.control(KIND), CB_RESETCONTENT, 0, 0);
                for (kind, label) in [
                    (TranslationKind::Word, "单词"),
                    (TranslationKind::Phrase, "短语"),
                    (TranslationKind::Sentence, "整句"),
                ] {
                    if self.controller.supports_kind(kind) {
                        self.item(KIND, label);
                    }
                }
                if !self.controller.supports_kind(self.controller.kind) {
                    self.controller.set_kind(TranslationKind::Word);
                }
                SendMessageW(
                    self.control(KIND),
                    CB_SETCURSEL,
                    match self.controller.kind {
                        TranslationKind::Word => 0,
                        TranslationKind::Phrase => 1,
                        TranslationKind::Sentence => 2,
                    },
                    0,
                );
            }
            SendMessageW(
                self.control(DIRECTION),
                CB_SETCURSEL,
                match self.controller.direction {
                    DirectionChoice::ChineseEnglish => 0,
                    DirectionChoice::EnglishChinese => 1,
                    DirectionChoice::AutoEnglish => 2,
                    DirectionChoice::AutoChinese => 3,
                },
                0,
            );
            SendMessageW(
                self.control(CONSENT),
                BM_SETCHECK,
                if self.controller.authorized {
                    BST_CHECKED
                } else {
                    BST_UNCHECKED
                } as usize,
                0,
            );
            EnableWindow(
                self.control(KIND),
                if self.controller.settings.provider == ProviderChoice::OfflineModel {
                    0
                } else {
                    1
                },
            );
            EnableWindow(
                self.control(START),
                i32::from(
                    !self.closing
                        && !self.controller.dirty
                        && self.controller.available()
                        && self.controller.supports_kind(self.controller.kind),
                ),
            );
            EnableWindow(
                self.control(COPY),
                i32::from(!self.closing && self.controller.result().is_some()),
            );
            let result = self
                .controller
                .result()
                .map(|r| {
                    let quality = match r.output.quality {
                        QualityStatus::ResearchOnly => "ResearchOnly · 原始研究释义，非正式翻译",
                        QualityStatus::Generated => "Generated · 在线生成，未经独立语言质量验收",
                        QualityStatus::Approved => "Approved · 有向映射",
                        QualityStatus::Pending => "Pending",
                    };
                    format!(
                        "{quality}\r\n来源：{} · {:?}\r\n\r\n{}",
                        r.provider.id,
                        r.output.output_kind,
                        r.output
                            .translated_text
                            .replace("\r\n", "\n")
                            .replace('\r', "\n")
                            .replace('\n', "\r\n")
                    )
                })
                .unwrap_or_default();
            if result != self.last_result {
                SetWindowTextW(self.control(RESULT), wide(&result).as_ptr());
                self.last_result = result;
            }
            let elapsed = self
                .controller
                .elapsed
                .map(|value| format!(" · 实测 {} ms（含UI观察间隔）", value.as_millis()))
                .unwrap_or_default();
            let status = format!(
                "{:?}{elapsed} · {}{}",
                self.controller.status,
                self.controller.notice,
                self.controller
                    .error
                    .as_ref()
                    .map(|e| format!(" · {e}"))
                    .unwrap_or_default()
            );
            if status != self.last_status {
                SetWindowTextW(self.control(STATUS), wide(&status).as_ptr());
                self.last_status = status;
            }
        }
    }
}

fn selected_direction(value: isize) -> DirectionChoice {
    match value {
        1 => DirectionChoice::EnglishChinese,
        2 => DirectionChoice::AutoEnglish,
        3 => DirectionChoice::AutoChinese,
        _ => DirectionChoice::ChineseEnglish,
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
unsafe fn read_text(hwnd: HWND, limit: usize) -> Result<String, TranslationError> {
    unsafe {
        let size = GetWindowTextLengthW(hwnd) as usize;
        if size > limit {
            return Err(TranslationError::InputTooLong);
        }
        let mut bytes = vec![0u16; size + 1];
        let count = GetWindowTextW(hwnd, bytes.as_mut_ptr(), bytes.len() as i32) as usize;
        let value = String::from_utf16(&bytes[..count]).map_err(|_| TranslationError::InvalidInput);
        bytes.zeroize();
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    struct NativeWindow(HWND);
    impl Drop for NativeWindow { fn drop(&mut self) { unsafe { DestroyWindow(self.0); } } }

    #[test]
    fn refresh_preserves_native_selection_until_the_change_notification() {
        let directory = std::env::temp_dir().join(format!("cloudleaf-r12-native-{}",std::process::id()));
        let mut controller = PanelController::new(directory,None,Arc::new(WindowsCredentialStore::default())).unwrap();
        let deadline = Instant::now()+Duration::from_secs(3);
        while controller.settings_serial==0 { controller.tick(); assert!(Instant::now()<deadline); std::thread::yield_now(); }
        unsafe {
            let window=NativeWindow(CreateWindowExW(0,wide("STATIC").as_ptr(),wide("R12 native control regression").as_ptr(),0,0,0,1120,850,null_mut(),null_mut(),GetModuleHandleW(null()),null()));
            assert!(!window.0.is_null());
            let mut state=WindowState{controller,controls:Vec::new(),parent:window.0,font:null_mut(),dpi:96,serial:u64::MAX,last_result:String::new(),last_status:String::new(),closing:false};
            state.create_controls(window.0);state.render();
            SendMessageW(state.control(DIRECTION),CB_SETCURSEL,1,0);
            state.render();
            assert_eq!(SendMessageW(state.control(DIRECTION),CB_GETCURSEL,0,0),1,"刷新不能覆盖用户尚在选择的方向");
            state.command(window.0,((CBN_SELCHANGE as usize)<<16)|DIRECTION);
            assert_eq!(state.controller.direction,DirectionChoice::EnglishChinese);
            state.controller.begin_close();
        }
    }
}
