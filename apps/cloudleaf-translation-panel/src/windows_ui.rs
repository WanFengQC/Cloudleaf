#![allow(unsafe_code)]

use crate::center::UserAction;
use crate::controller::PanelController;
use crate::definitions::DefinitionDirection;
use crate::platform::{WindowsCredentialStore, copy_unicode};
use crate::resources::ResourceSlot;
use crate::settings::{DirectionChoice, ProviderChoice, Settings, WorkerPriority, WorkerThreads};
use cloudleaf_settings::{
    DictionaryIdentity, InputFuzzyPair, InputPreferences, InputScheme, ResourceAction,
    ResourceSlot as DictionarySlot,
};
use cloudleaf_translation::{QualityStatus, Secret, TranslationError, TranslationKind};
use std::cell::RefCell;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::time::{Duration, Instant};
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
const OFFLINE_CATALOG: usize = 216;
const THREADS: usize = 217;
const PRIORITY: usize = 218;
const OFFLINE_GLOSSARY: usize = 219;
const TEST_CONNECTION: usize = 220;
const PAGE_TRANSLATION: usize = 230;
const PAGE_INPUT: usize = 231;
const PAGE_USER: usize = 232;
const PAGE_RESOURCES: usize = 233;
const PAGE_DEFINITIONS: usize = 234;
const INPUT_SCHEME: usize = 301;
const INPUT_CORRECTION: usize = 302;
const INPUT_MIXED: usize = 303;
const INPUT_PAGE_SIZE: usize = 304;
const INPUT_MAXIMUM: usize = 305;
const INPUT_SAVE: usize = 306;
const INPUT_LOAD: usize = 307;
const INPUT_NOTE: usize = 308;
const INPUT_FUZZY: usize = 310;
const USER_DIRECTORY: usize = 401;
const USER_ID: usize = 402;
const USER_WORD: usize = 403;
const USER_PINYIN: usize = 404;
const USER_TRANSFER: usize = 405;
const USER_BACKUP: usize = 406;
const USER_OPEN: usize = 407;
const USER_ADD: usize = 408;
const USER_EDIT: usize = 409;
const USER_REMOVE: usize = 410;
const USER_IMPORT: usize = 411;
const USER_EXPORT: usize = 412;
const USER_RECOVER: usize = 413;
const USER_LEARNING: usize = 414;
const USER_CLEAR_LEARNING: usize = 415;
const USER_CONTENT: usize = 416;
const USER_NOTE: usize = 417;
const RESOURCES: usize = 501;
const RESOURCE_PATH: usize = 502;
const RESOURCE_HASH: usize = 503;
const RESOURCE_BASE: usize = 504;
const RESOURCE_SUPPLEMENT: usize = 505;
const RESOURCE_SELECT_BASE: usize = 506;
const RESOURCE_SELECT_SUPPLEMENT: usize = 507;
const RESOURCE_ROLLBACK_BASE: usize = 508;
const RESOURCE_ROLLBACK_SUPPLEMENT: usize = 509;
const RESOURCE_ENABLE_SUPPLEMENT: usize = 510;
const DEFINITION_INDEX: usize = 601;
const DEFINITION_HASH: usize = 602;
const DEFINITION_MODE: usize = 603;
const DEFINITION_QUERY: usize = 604;
const DEFINITION_STATUS: usize = 605;
const DEFINITION_CONTENT: usize = 606;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsPage {
    Translation,
    Input,
    User,
    Resources,
    Definitions,
}

struct Control {
    hwnd: HWND,
    id: usize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    right: bool,
    page: Option<SettingsPage>,
}
struct WindowState {
    controller: PanelController,
    controls: Vec<Control>,
    parent: HWND,
    font: HFONT,
    dpi: u32,
    serial: u64,
    input_serial: u64,
    user_serial: u64,
    page: SettingsPage,
    control_page: Option<SettingsPage>,
    last_result: String,
    last_status: String,
    last_resources: String,
    runtime_refresh: Instant,
    last_definitions: String,
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
    let state = Box::new(RefCell::new(WindowState::new(controller, null_mut())));
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
            wide("云笺 Cloudleaf · 翻译与设置").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1120,
            920,
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
            920 * dpi / 96,
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
                            y: state.scale(880),
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
                        state.clear_password();
                        state.render();
                        return 0;
                    }
                    WM_TIMER => {
                        if state.runtime_refresh.elapsed() >= Duration::from_secs(1) {
                            state.controller.refresh_runtime_settings();
                            state.runtime_refresh = Instant::now();
                        }
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
    fn new(controller: PanelController, parent: HWND) -> Self {
        Self {
            controller,
            controls: Vec::new(),
            parent,
            font: null_mut(),
            dpi: 96,
            serial: u64::MAX,
            input_serial: u64::MAX,
            user_serial: u64::MAX,
            page: SettingsPage::Translation,
            control_page: None,
            last_result: String::new(),
            last_status: String::new(),
            last_resources: String::new(),
            runtime_refresh: Instant::now(),
            last_definitions: String::new(),
            closing: false,
        }
    }

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
            SendMessageW(self.control(RESULT), EM_SETLIMITTEXT, 128 * 1024, 0);
            self.add(
                STATUS,
                "STATIC",
                "正在初始化…",
                0,
                [16, 733, 1000, 46],
                false,
            );
            for (id, label, x) in [
                (PAGE_TRANSLATION, "翻译", 0),
                (PAGE_INPUT, "输入方案", 78),
                (PAGE_USER, "用户词库", 156),
                (PAGE_RESOURCES, "资源诊断", 234),
                (PAGE_DEFINITIONS, "词语释义", 312),
            ] {
                self.add(
                    id,
                    "BUTTON",
                    label,
                    BS_PUSHBUTTON as u32,
                    [x, 13, 74, 32],
                    true,
                );
            }
            self.control_page = Some(SettingsPage::Translation);
            self.add(0, "STATIC", "翻译设置", 0, [0, 16, 400, 28], true);
            self.add(
                PROVIDER,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32,
                [0, 54, 400, 140],
                true,
            );
            for text in [
                "本地研究词典",
                "用户自定义在线 API",
                "离线模型（本地 OPUS-MT）",
            ] {
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
                "离线模型 catalog.json 路径（先用模型工具导入）",
                0,
                [0, 161, 400, 22],
                true,
            );
            self.add(
                OFFLINE_CATALOG,
                "EDIT",
                "",
                ES_AUTOHSCROLL as u32 | WS_BORDER,
                [0, 187, 400, 27],
                true,
            );
            self.add(
                0,
                "STATIC",
                "API 完整端点（HTTPS；不提供默认服务）",
                0,
                [0, 226, 400, 22],
                true,
            );
            self.add(
                ENDPOINT,
                "EDIT",
                "",
                ES_AUTOHSCROLL as u32 | WS_BORDER,
                [0, 252, 400, 27],
                true,
            );
            self.add(0, "STATIC", "模型名称", 0, [0, 291, 400, 22], true);
            self.add(
                MODEL,
                "EDIT",
                "",
                ES_AUTOHSCROLL as u32 | WS_BORDER,
                [0, 317, 400, 27],
                true,
            );
            self.add(
                0,
                "STATIC",
                "连接超时(秒)                 翻译总超时(秒)",
                0,
                [0, 356, 400, 22],
                true,
            );
            self.add(
                CONNECT,
                "EDIT",
                "5",
                ES_NUMBER as u32 | WS_BORDER,
                [0, 382, 90, 27],
                true,
            );
            self.add(
                TOTAL,
                "EDIT",
                "15",
                ES_NUMBER as u32 | WS_BORDER,
                [210, 382, 90, 27],
                true,
            );
            for (id, text, y) in [
                (CACHE, "启用内存缓存（不写输入历史）", 425),
                (NETWORK, "启用联网（重启后恢复离线）", 459),
                (LOOPBACK, "允许本机 HTTP 服务（仅 loopback）", 493),
                (CONSENT, "允许本次发送到已应用的上方 API 端点", 527),
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
                [0, 570, 400, 22],
                true,
            );
            self.add(
                KEY,
                "EDIT",
                "",
                ES_PASSWORD as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER,
                [0, 596, 400, 27],
                true,
            );
            SendMessageW(self.control(KEY), EM_SETLIMITTEXT, 8192, 0);
            self.add(
                SAVE_KEY,
                "BUTTON",
                "保存 / 更新密钥",
                BS_PUSHBUTTON as u32,
                [0, 635, 128, 32],
                true,
            );
            self.add(
                DELETE_KEY,
                "BUTTON",
                "删除该端点密钥",
                BS_PUSHBUTTON as u32,
                [137, 635, 119, 32],
                true,
            );
            self.add(
                TEST_CONNECTION,
                "BUTTON",
                "测试 API",
                BS_PUSHBUTTON as u32,
                [265, 635, 113, 32],
                true,
            );
            self.add(
                APPLY,
                "BUTTON",
                "应用并保存设置",
                BS_PUSHBUTTON as u32,
                [0, 689, 180, 34],
                true,
            );
            self.add(
                RESET,
                "BUTTON",
                "恢复默认设置",
                BS_PUSHBUTTON as u32,
                [195, 689, 180, 34],
                true,
            );
            self.add(
                0,
                "STATIC",
                "离线 Worker 线程及调度（低优先级为研究档）",
                0,
                [0, 735, 400, 20],
                true,
            );
            self.add(
                THREADS,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32,
                [0, 759, 145, 150],
                true,
            );
            for label in ["单线程（默认）", "2 线程", "4 线程", "自动"] {
                self.item(THREADS, label);
            }
            self.add(
                PRIORITY,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32,
                [155, 759, 175, 115],
                true,
            );
            for label in ["正常", "低优先级 · 研究"] {
                self.item(PRIORITY, label);
            }
            self.settings_field(OFFLINE_GLOSSARY, "离线翻译术语表 TSV（可选）", "", 0, 800);
            self.create_input_settings();
            self.create_user_settings();
            self.control_page = Some(SettingsPage::Resources);
            self.settings_field(RESOURCE_PATH, "只读词典绝对路径", "", 0, 16);
            self.settings_field(RESOURCE_HASH, "词典 SHA-256", "", 0, 83);
            for (id, label, x) in [
                (RESOURCE_BASE, "核验基础词典", 0),
                (RESOURCE_SUPPLEMENT, "核验补充词典", 170),
            ] {
                self.add(
                    id,
                    "BUTTON",
                    label,
                    BS_PUSHBUTTON as u32,
                    [x, 148, 160, 30],
                    true,
                );
            }
            for (id, label, x, y) in [
                (RESOURCE_SELECT_BASE, "选用基础词典", 0, 189),
                (RESOURCE_SELECT_SUPPLEMENT, "选用补充词典", 170, 189),
                (RESOURCE_ROLLBACK_BASE, "回退基础词典", 0, 230),
                (RESOURCE_ROLLBACK_SUPPLEMENT, "回退补充词典", 170, 230),
            ] {
                self.add(
                    id,
                    "BUTTON",
                    label,
                    BS_PUSHBUTTON as u32,
                    [x, y, 160, 30],
                    true,
                );
            }
            self.add(
                RESOURCE_ENABLE_SUPPLEMENT,
                "BUTTON",
                "启用已选补充词典（研究）",
                BS_AUTOCHECKBOX as u32,
                [0, 271, 340, 26],
                true,
            );
            self.add(
                RESOURCES,
                "EDIT",
                "",
                ES_MULTILINE as u32 | ES_READONLY as u32 | WS_VSCROLL | WS_BORDER,
                [0, 310, 400, 415],
                true,
            );
            self.control_page = Some(SettingsPage::Definitions);
            self.settings_field(DEFINITION_INDEX, "语义索引绝对路径", "", 0, 16);
            self.settings_field(DEFINITION_HASH, "索引 SHA-256", "", 0, 83);
            self.add(
                DEFINITION_MODE,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32,
                [0, 148, 205, 120],
                true,
            );
            self.item(DEFINITION_MODE, "中文词语或短语");
            self.item(DEFINITION_MODE, "精确反查英文释义");
            SendMessageW(self.control(DEFINITION_MODE), CB_SETCURSEL, 0, 0);
            self.add(
                DEFINITION_QUERY,
                "BUTTON",
                "查询左侧原文",
                BS_PUSHBUTTON as u32,
                [219, 148, 128, 30],
                true,
            );
            self.add(DEFINITION_STATUS, "STATIC", "", 0, [0, 193, 400, 35], true);
            self.add(
                DEFINITION_CONTENT,
                "EDIT",
                "",
                ES_MULTILINE as u32 | ES_READONLY as u32 | WS_VSCROLL | WS_BORDER,
                [0, 234, 400, 410],
                true,
            );
            SendMessageW(
                self.control(DEFINITION_CONTENT),
                EM_SETLIMITTEXT,
                128 * 1024,
                0,
            );
            self.control_page = None;
        }
    }

    unsafe fn settings_field(&mut self, id: usize, label: &str, initial: &str, flags: u32, y: i32) {
        unsafe {
            self.add(0, "STATIC", label, 0, [0, y, 400, 24], true);
            self.add(
                id,
                "EDIT",
                initial,
                ES_AUTOHSCROLL as u32 | WS_BORDER | flags,
                [0, y + 26, 400, 28],
                true,
            );
        }
    }

    unsafe fn create_input_settings(&mut self) {
        self.control_page = Some(SettingsPage::Input);
        unsafe {
            self.add(0, "STATIC", "拼音输入方案", 0, [0, 16, 400, 24], true);
            self.add(
                INPUT_SCHEME,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32,
                [0, 47, 400, 150],
                true,
            );
            for item in ["标准全拼", "简拼", "小鹤双拼"] {
                self.item(INPUT_SCHEME, item);
            }
            self.add(0, "STATIC", "模糊音（双向）", 0, [0, 90, 400, 22], true);
            for (index, pair) in InputFuzzyPair::ALL.into_iter().enumerate() {
                let x = if index % 2 == 0 { 0 } else { 176 };
                let y = 119 + (index / 2) as i32 * 31;
                self.add(
                    INPUT_FUZZY + index,
                    "BUTTON",
                    pair.label(),
                    BS_AUTOCHECKBOX as u32,
                    [x, y, 170, 28],
                    true,
                );
            }
            self.add(
                INPUT_CORRECTION,
                "BUTTON",
                "启用拼音纠错（全拼、简拼）",
                BS_AUTOCHECKBOX as u32,
                [0, 257, 400, 29],
                true,
            );
            self.add(
                INPUT_MIXED,
                "BUTTON",
                "启用自动中英混排",
                BS_AUTOCHECKBOX as u32,
                [0, 292, 400, 29],
                true,
            );
            self.settings_field(
                INPUT_PAGE_SIZE,
                "每页候选（1–9）",
                "",
                ES_NUMBER as u32,
                339,
            );
            self.settings_field(INPUT_MAXIMUM, "最多候选（1–50）", "", ES_NUMBER as u32, 411);
            self.add(
                INPUT_SAVE,
                "BUTTON",
                "保存输入设置",
                BS_PUSHBUTTON as u32,
                [0, 491, 163, 34],
                true,
            );
            self.add(
                INPUT_LOAD,
                "BUTTON",
                "重新读取",
                BS_PUSHBUTTON as u32,
                [177, 491, 136, 34],
                true,
            );
            self.add(INPUT_NOTE, "STATIC", "", 0, [0, 540, 400, 65], true);
        }
    }

    unsafe fn create_user_settings(&mut self) {
        self.control_page = Some(SettingsPage::User);
        let directory = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("Cloudleaf/user-dictionary"))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        unsafe {
            self.settings_field(USER_DIRECTORY, "用户词库绝对目录", &directory, 0, 16);
            self.settings_field(USER_ID, "词条 ID（修改或删除时填写）", "", 0, 83);
            self.settings_field(USER_WORD, "词语", "", 0, 148);
            self.settings_field(USER_PINYIN, "拼音", "", 0, 213);
            for (id, label, x) in [
                (USER_OPEN, "读取", 0),
                (USER_ADD, "添加", 88),
                (USER_EDIT, "修改", 176),
                (USER_REMOVE, "删除", 264),
            ] {
                self.add(
                    id,
                    "BUTTON",
                    label,
                    BS_PUSHBUTTON as u32,
                    [x, 282, 80, 34],
                    true,
                );
            }
            self.settings_field(USER_TRANSFER, "导入文件或导出新文件的绝对路径", "", 0, 332);
            self.add(
                USER_IMPORT,
                "BUTTON",
                "导入并合并",
                BS_PUSHBUTTON as u32,
                [0, 390, 128, 34],
                true,
            );
            self.add(
                USER_EXPORT,
                "BUTTON",
                "导出备份",
                BS_PUSHBUTTON as u32,
                [142, 390, 128, 34],
                true,
            );
            self.settings_field(
                USER_BACKUP,
                "恢复版本号（见下方当前状态）",
                "",
                ES_NUMBER as u32,
                438,
            );
            self.add(
                USER_RECOVER,
                "BUTTON",
                "恢复指定版本",
                BS_PUSHBUTTON as u32,
                [0, 497, 128, 34],
                true,
            );
            self.add(
                USER_LEARNING,
                "BUTTON",
                "启用学习",
                BS_AUTOCHECKBOX as u32,
                [142, 497, 112, 34],
                true,
            );
            self.add(
                USER_CLEAR_LEARNING,
                "BUTTON",
                "清理学习",
                BS_PUSHBUTTON as u32,
                [264, 497, 105, 34],
                true,
            );
            self.add(USER_NOTE, "STATIC", "", 0, [0, 543, 400, 56], true);
            self.add(
                USER_CONTENT,
                "EDIT",
                "",
                ES_MULTILINE as u32 | ES_READONLY as u32 | WS_VSCROLL | WS_BORDER,
                [0, 604, 400, 218],
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
                page: self.control_page,
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
                let y = if c.id == STATUS {
                    height - 52
                } else if c.page.is_some() {
                    52 + c.y * 83 / 100
                } else {
                    c.y
                };
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
                ShowWindow(
                    c.hwnd,
                    if c.page.is_none_or(|page| page == self.page) {
                        SW_SHOWNA
                    } else {
                        SW_HIDE
                    },
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
            let catalog = read_text(self.control(OFFLINE_CATALOG), 4096)?;
            settings.offline_catalog = if catalog.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(catalog))
            };
            let glossary = read_text(self.control(OFFLINE_GLOSSARY), 4096)?;
            settings.offline_glossary =
                (!glossary.trim().is_empty()).then(|| PathBuf::from(glossary));
            settings.worker_threads = match SendMessageW(self.control(THREADS), CB_GETCURSEL, 0, 0)
            {
                1 => WorkerThreads::Two,
                2 => WorkerThreads::Four,
                3 => WorkerThreads::Auto,
                _ => WorkerThreads::One,
            };
            settings.worker_priority =
                if SendMessageW(self.control(PRIORITY), CB_GETCURSEL, 0, 0) == 1 {
                    WorkerPriority::BelowNormal
                } else {
                    WorkerPriority::Normal
                };
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

    unsafe fn input_settings_from_controls(&self) -> Result<InputPreferences, TranslationError> {
        unsafe {
            let old = &self.controller.input_settings.settings;
            Ok(InputPreferences {
                scheme: match SendMessageW(self.control(INPUT_SCHEME), CB_GETCURSEL, 0, 0) {
                    1 => InputScheme::Abbreviated,
                    2 => InputScheme::Xiaohe,
                    _ => InputScheme::Full,
                },
                fuzzy_pairs: InputFuzzyPair::ALL
                    .into_iter()
                    .enumerate()
                    .filter_map(|(i, pair)| self.checked(INPUT_FUZZY + i).then_some(pair))
                    .collect(),
                correction: self.checked(INPUT_CORRECTION),
                mixed_input: self.checked(INPUT_MIXED),
                page_size: read_text(self.control(INPUT_PAGE_SIZE), 8)?
                    .parse()
                    .map_err(|_| TranslationError::InvalidConfig)?,
                max_candidates: read_text(self.control(INPUT_MAXIMUM), 8)?
                    .parse()
                    .map_err(|_| TranslationError::InvalidConfig)?,
                ..old.clone()
            })
        }
    }

    unsafe fn user_command(&mut self, hwnd: HWND, id: usize) -> Result<(), TranslationError> {
        unsafe {
            let directory = PathBuf::from(read_text(self.control(USER_DIRECTORY), 4096)?);
            if matches!(id, USER_REMOVE | USER_RECOVER | USER_CLEAR_LEARNING)
                && MessageBoxW(
                    hwnd,
                    wide(match id {
                        USER_REMOVE => "删除填写 ID 所指的词条？",
                        USER_RECOVER => "使用指定的历史版本恢复损坏的当前词库？",
                        _ => "清除学习统计并保留用户词条？",
                    })
                    .as_ptr(),
                    wide("确认用户词库操作").as_ptr(),
                    MB_YESNO | MB_DEFBUTTON2 | MB_ICONWARNING,
                ) != IDYES
            {
                return Ok(());
            }
            let word = || read_text(self.control(USER_WORD), 4096);
            let pinyin = || read_text(self.control(USER_PINYIN), 512);
            let entry = || read_text(self.control(USER_ID), 128);
            let transfer = || read_text(self.control(USER_TRANSFER), 4096).map(PathBuf::from);
            let action = match id {
                USER_OPEN => UserAction::Inspect,
                USER_ADD => UserAction::Add {
                    text: word()?,
                    pinyin: pinyin()?,
                },
                USER_EDIT => UserAction::Update {
                    id: entry()?,
                    text: word()?,
                    pinyin: pinyin()?,
                },
                USER_REMOVE => UserAction::Remove(entry()?),
                USER_IMPORT => UserAction::Import(transfer()?),
                USER_EXPORT => UserAction::Export(transfer()?),
                USER_RECOVER => UserAction::Recover(
                    read_text(self.control(USER_BACKUP), 32)?
                        .parse()
                        .map_err(|_| TranslationError::InvalidConfig)?,
                ),
                USER_LEARNING => UserAction::ChangeLearning(self.checked(USER_LEARNING)),
                USER_CLEAR_LEARNING => UserAction::ClearLearning,
                _ => unreachable!("只处理用户词库按钮"),
            };
            self.controller.manage_user_dictionary(directory, action)
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
            if notification == EN_CHANGE && [DEFINITION_INDEX, DEFINITION_HASH].contains(&id)
                || id == DEFINITION_MODE && notification == CBN_SELCHANGE
            {
                self.controller.clear_definitions();
                return;
            }
            if [
                DICTIONARY,
                ENDPOINT,
                MODEL,
                CONNECT,
                TOTAL,
                OFFLINE_CATALOG,
                OFFLINE_GLOSSARY,
            ]
            .contains(&id)
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
                    PROVIDER | THREADS | PRIORITY => self.controller.mark_settings_dirty(),
                    _ => {}
                }
                return;
            }
            if notification != BN_CLICKED {
                return;
            }
            let result = match id {
                PAGE_TRANSLATION | PAGE_INPUT | PAGE_USER | PAGE_RESOURCES | PAGE_DEFINITIONS => {
                    self.controller.clear_definitions();
                    self.page = match id {
                        PAGE_INPUT => SettingsPage::Input,
                        PAGE_USER => SettingsPage::User,
                        PAGE_RESOURCES => SettingsPage::Resources,
                        PAGE_DEFINITIONS => SettingsPage::Definitions,
                        _ => SettingsPage::Translation,
                    };
                    if matches!(self.page, SettingsPage::Resources | SettingsPage::Input) {
                        self.controller.refresh_runtime_settings();
                        self.runtime_refresh = Instant::now();
                    }
                    self.layout(hwnd);
                    Ok(())
                }
                INPUT_SAVE => self
                    .input_settings_from_controls()
                    .and_then(|settings| self.controller.save_input_settings(settings)),
                INPUT_LOAD => self.controller.reload_input_settings(),
                DEFINITION_QUERY => {
                    let result = read_text(self.control(DEFINITION_INDEX), 4096)
                        .and_then(|path| {
                            read_text(self.control(DEFINITION_HASH), 64).map(|hash| (path, hash))
                        })
                        .and_then(|(path, hash)| {
                            self.controller.lookup_definition(
                                PathBuf::from(path),
                                hash,
                                if SendMessageW(self.control(DEFINITION_MODE), CB_GETCURSEL, 0, 0)
                                    == 1
                                {
                                    DefinitionDirection::EnglishGloss
                                } else {
                                    DefinitionDirection::ChineseHeadword
                                },
                            )
                        });
                    if result.is_err() {
                        self.controller.definitions.notice =
                            "无法开始查询，请核对原文、绝对路径和 SHA-256".into();
                    }
                    Ok(())
                }
                RESOURCE_BASE
                | RESOURCE_SUPPLEMENT
                | RESOURCE_SELECT_BASE
                | RESOURCE_SELECT_SUPPLEMENT => {
                    let slot = if matches!(id, RESOURCE_BASE | RESOURCE_SELECT_BASE) {
                        ResourceSlot::Base
                    } else {
                        ResourceSlot::Supplement
                    };
                    let path = read_text(self.control(RESOURCE_PATH), 4096).unwrap_or_default();
                    let hash = read_text(self.control(RESOURCE_HASH), 64).unwrap_or_default();
                    if matches!(id, RESOURCE_BASE | RESOURCE_SUPPLEMENT) {
                        self.controller
                            .inspect_resource(slot, PathBuf::from(path), hash);
                    } else {
                        self.controller
                            .select_dictionary(slot, PathBuf::from(path), hash);
                    }
                    Ok(())
                }
                RESOURCE_ROLLBACK_BASE | RESOURCE_ROLLBACK_SUPPLEMENT => {
                    let slot = if id == RESOURCE_ROLLBACK_BASE {
                        DictionarySlot::Base
                    } else {
                        DictionarySlot::Supplement
                    };
                    self.controller
                        .manage_dictionary(ResourceAction::Rollback(slot));
                    Ok(())
                }
                RESOURCE_ENABLE_SUPPLEMENT => {
                    let enable =
                        SendMessageW(self.control(id), BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
                    self.controller
                        .manage_dictionary(ResourceAction::SetSupplementEnabled(enable));
                    Ok(())
                }
                USER_OPEN..=USER_CLEAR_LEARNING => self.user_command(hwnd, id),
                TEST_CONNECTION => self.controller.test_connection(),
                START => self.controller.start(),
                CANCEL => {
                    self.clear_password();
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
                RESET => {
                    self.clear_password();
                    self.controller.apply(Settings::default())
                }
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
                    self.clear_password();
                    match value {
                        Ok(value) => Secret::new(value).and_then(|secret| {
                            settings.and_then(|s| self.controller.save_key(s, secret))
                        }),
                        Err(e) => Err(e),
                    }
                }
                DELETE_KEY => {
                    self.clear_password();
                    self.settings_from_controls()
                        .and_then(|s| self.controller.delete_key(s))
                }
                _ => Ok(()),
            };
            if let Err(error) = result {
                self.controller.record_error(error);
            }
        }
    }

    unsafe fn clear_password(&self) {
        unsafe {
            SetWindowTextW(self.control(KEY), wide("").as_ptr());
            SendMessageW(self.control(KEY), EM_EMPTYUNDOBUFFER, 0, 0);
        }
    }

    unsafe fn render(&mut self) {
        unsafe {
            let editable =
                !self.closing && self.controller.status != crate::controller::ViewStatus::Preparing;
            for id in PROVIDER..=OFFLINE_GLOSSARY {
                EnableWindow(self.control(id), i32::from(editable));
            }
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
                    self.control(OFFLINE_CATALOG),
                    wide(
                        s.offline_catalog
                            .as_ref()
                            .and_then(|path| path.to_str())
                            .unwrap_or(""),
                    )
                    .as_ptr(),
                );
                SetWindowTextW(
                    self.control(OFFLINE_GLOSSARY),
                    wide(
                        s.offline_glossary
                            .as_deref()
                            .and_then(|path| path.to_str())
                            .unwrap_or(""),
                    )
                    .as_ptr(),
                );
                SendMessageW(
                    self.control(THREADS),
                    CB_SETCURSEL,
                    match s.worker_threads {
                        WorkerThreads::One => 0,
                        WorkerThreads::Two => 1,
                        WorkerThreads::Four => 2,
                        WorkerThreads::Auto => 3,
                    },
                    0,
                );
                SendMessageW(
                    self.control(PRIORITY),
                    CB_SETCURSEL,
                    usize::from(s.worker_priority == WorkerPriority::BelowNormal),
                    0,
                );
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
                SendMessageW(self.control(DIRECTION), CB_RESETCONTENT, 0, 0);
                for label in ["中文 → 英文", "英文 → 中文"] {
                    self.item(DIRECTION, label);
                }
                if self.controller.supports_auto() {
                    self.item(DIRECTION, "自动检测 → 英文");
                    self.item(DIRECTION, "自动检测 → 中文");
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
            }
            self.render_settings_center();
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
                i32::from(!self.closing && self.controller.available()),
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
            EnableWindow(
                self.control(TEST_CONNECTION),
                i32::from(
                    editable
                        && self.controller.available()
                        && !self.controller.dirty
                        && self.controller.settings.provider == ProviderChoice::Online
                        && self.controller.authorized,
                ),
            );
            let result = self
                .controller
                .result()
                .map(|r| {
                    let quality = match r.output.quality {
                        QualityStatus::ResearchOnly => "ResearchOnly · 原始研究释义，非正式翻译",
                        QualityStatus::Generated => "Generated · 模型生成，未经独立语言质量验收",
                        QualityStatus::Approved => "Approved · 有向映射",
                        QualityStatus::Pending => "Pending",
                    };
                    format!(
                        "{quality}\r\n提供者：{} · {:?}\r\n数据来源：{}{}\r\n\r\n{}",
                        r.provider.id,
                        r.output.output_kind,
                        r.output.source_ids.join(", "),
                        self.controller
                            .integrity_report()
                            .map(integrity_summary)
                            .unwrap_or_default(),
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
            let worker = self
                .controller
                .worker_snapshot()
                .map(|snapshot| format!(" · 工作进程{}", worker_stage(snapshot.stage)))
                .unwrap_or_default();
            let status = format!(
                "{:?}{elapsed}{worker} · {}{}",
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

    unsafe fn render_settings_center(&mut self) {
        unsafe {
            let definitions = &self.controller.definitions;
            EnableWindow(
                self.control(DEFINITION_QUERY),
                i32::from(!definitions.pending && !self.closing),
            );
            SetWindowTextW(
                self.control(DEFINITION_STATUS),
                wide(&definitions.notice).as_ptr(),
            );
            if self.last_definitions != definitions.contents {
                let display = definitions.contents.replace('\n', "\r\n");
                SetWindowTextW(self.control(DEFINITION_CONTENT), wide(&display).as_ptr());
                self.last_definitions.clone_from(&definitions.contents);
            }
            let input = &self.controller.input_settings;
            for id in (INPUT_SCHEME..=INPUT_LOAD)
                .chain(INPUT_FUZZY..INPUT_FUZZY + InputFuzzyPair::ALL.len())
            {
                EnableWindow(self.control(id), i32::from(!input.working && !self.closing));
            }
            let input_note = if input.working {
                "正在处理输入设置…"
            } else {
                &input.notice
            };
            SetWindowTextW(self.control(INPUT_NOTE), wide(input_note).as_ptr());
            if self.input_serial != input.serial {
                self.input_serial = input.serial;
                let settings = &input.settings;
                SendMessageW(
                    self.control(INPUT_SCHEME),
                    CB_SETCURSEL,
                    match settings.scheme {
                        InputScheme::Full => 0,
                        InputScheme::Abbreviated => 1,
                        InputScheme::Xiaohe => 2,
                    },
                    0,
                );
                for (id, value) in [
                    (INPUT_PAGE_SIZE, settings.page_size),
                    (INPUT_MAXIMUM, settings.max_candidates),
                ] {
                    SetWindowTextW(self.control(id), wide(&value.to_string()).as_ptr());
                }
                for (id, selected) in [
                    (INPUT_CORRECTION, settings.correction),
                    (INPUT_MIXED, settings.mixed_input),
                ]
                .into_iter()
                .chain(
                    InputFuzzyPair::ALL
                        .into_iter()
                        .enumerate()
                        .map(|(index, pair)| {
                            (INPUT_FUZZY + index, settings.fuzzy_pairs.contains(&pair))
                        }),
                ) {
                    SendMessageW(
                        self.control(id),
                        BM_SETCHECK,
                        if selected { BST_CHECKED } else { BST_UNCHECKED } as usize,
                        0,
                    );
                }
            }

            let user = &self.controller.user_settings;
            for id in USER_OPEN..=USER_CLEAR_LEARNING {
                let enabled =
                    !user.working && !self.closing && (!user.needs_refresh || id == USER_OPEN);
                EnableWindow(self.control(id), i32::from(enabled));
            }
            let note = if user.working {
                "正在后台处理用户词库…"
            } else {
                &user.notice
            };
            SetWindowTextW(self.control(USER_NOTE), wide(note).as_ptr());
            if self.user_serial != user.serial {
                self.user_serial = user.serial;
                let dictionary = &user.dictionary;
                SendMessageW(
                    self.control(USER_LEARNING),
                    BM_SETCHECK,
                    if dictionary.learning {
                        BST_CHECKED
                    } else {
                        BST_UNCHECKED
                    } as usize,
                    0,
                );
                let generation = dictionary
                    .generation
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "—".into());
                let mut listing = format!(
                    "当前版本 {generation} · 共 {} 条\r\n可用版本 {:?}\r\n\r\n",
                    dictionary.entries.len(),
                    dictionary.backups
                );
                for entry in dictionary.entries.iter().take(128) {
                    use std::fmt::Write as _;
                    let state = if entry.enabled { "启用" } else { "停用" };
                    let _ = write!(
                        listing,
                        "{} · {} · {} · {state}\r\n",
                        entry.id, entry.text, entry.pinyin
                    );
                }
                if dictionary.entries.len() > 128 {
                    listing.push_str("\r\n列表显示前 128 条。导出可查看完整词库。");
                }
                SetWindowTextW(self.control(USER_CONTENT), wide(&listing).as_ptr());
            }

            if self.page == SettingsPage::Resources {
                let controller = &self.controller;
                let editable =
                    controller.dictionary_config.is_some() && !controller.dictionary_saving;
                for id in RESOURCE_SELECT_BASE..=RESOURCE_ENABLE_SUPPLEMENT {
                    EnableWindow(self.control(id), i32::from(editable));
                }
                let supplement_enabled = controller
                    .dictionary_config
                    .as_ref()
                    .is_some_and(|config| config.supplement_enabled);
                if !controller.dictionary_saving {
                    SendMessageW(
                        self.control(RESOURCE_ENABLE_SUPPLEMENT),
                        BM_SETCHECK,
                        if supplement_enabled {
                            BST_CHECKED
                        } else {
                            BST_UNCHECKED
                        } as usize,
                        0,
                    );
                }
                let summary = self.resource_text();
                if summary != self.last_resources {
                    SetWindowTextW(self.control(RESOURCES), wide(&summary).as_ptr());
                    self.last_resources = summary;
                }
            }
        }
    }

    fn resource_text(&self) -> String {
        use std::fmt::Write as _;
        let mut summary = String::from("输入词典运行时\r\n");
        let _ = writeln!(summary, "{}\r", self.controller.dictionary_notice);
        if let Some(config) = &self.controller.dictionary_config {
            let digest = |artifact: Option<&cloudleaf_settings::DictionaryArtifact>| {
                artifact
                    .map(|file| file.sha256().to_owned())
                    .unwrap_or_else(|| "未记录".into())
            };
            let _ = write!(
                summary,
                "已保存修订：{}\r\n已选基础 SHA-256：{}\r\n基础回退 SHA-256：{}\r\n已选补充 SHA-256：{}\r\n补充回退 SHA-256：{}\r\n补充开关：{}\r\n",
                config.revision,
                digest(config.base.as_ref()),
                digest(config.previous_base.as_ref()),
                digest(config.supplement.as_ref()),
                digest(config.previous_supplement.as_ref()),
                if config.supplement_enabled {
                    "开"
                } else {
                    "关"
                }
            );
        }
        if self.controller.dictionary_hosts.is_empty() {
            summary.push_str("未收到在线宿主的词典状态\r\n");
        }
        for host in &self.controller.dictionary_hosts {
            let status = host.state.label();
            let _ = write!(
                summary,
                "\r\n宿主 {} · PID {} · {}\r\n希望修订 {} · 生效修订 {} · 快照代次 {}\r\n实际快照 SHA-256 {}\r\n输入模型 {}\r\n用户词库代次 {}\r\n{}\r\n",
                host.host_id,
                host.process_id,
                status,
                host.desired_revision,
                host.active_revision,
                host.generation,
                host.active.snapshot_sha256,
                host.active.model_sha256.as_deref().unwrap_or("结构排序"),
                host.user_generation
                    .map_or_else(|| "未发布用户层".into(), |n| n.to_string()),
                host.error.as_deref().unwrap_or("")
            );
            render_dictionary_identity(&mut summary, "基础", &host.active.base);
            if let Some(supplement) = &host.active.supplement {
                render_dictionary_identity(&mut summary, "补充", supplement);
            } else {
                summary.push_str("补充未生效\r\n");
            }
        }
        summary.push_str("\r\n翻译提供者\r\n");
        if let Some(identity) = self.controller.provider_identity() {
            let _ = write!(
                summary,
                "标识：{}\r\n版本或校验：{}\r\n",
                identity.id, identity.version
            );
        } else {
            summary.push_str("当前没有可用的翻译提供者\r\n");
        }
        if let Some(snapshot) = self.controller.worker_snapshot() {
            let _ = write!(summary, "\r\nWorker {}\r\n", worker_stage(snapshot.stage));
            if let Some(model) = snapshot.model_identity {
                let _ = write!(summary, "已观测模型：{model}\r\n");
            }
            if let Some(metrics) = snapshot.metrics {
                let _ = write!(
                    summary,
                    "实际线程：{}\r\n载入：{} ms\r\n推理：{} ms\r\n",
                    metrics.compute_threads,
                    metrics.load_ns / 1_000_000,
                    metrics.inference_ns / 1_000_000
                );
            }
            if let Some(pid) = snapshot.child_pid {
                let _ = write!(summary, "进程：{pid}\r\n");
            }
        }
        if let Some(error) = self.controller.error.as_ref() {
            let _ = write!(summary, "\r\n当前错误：{error}\r\n");
        }
        summary.push_str("\r\n输入设置状态\r\n");
        summary.push_str(&self.controller.input_settings.notice);
        summary.push_str("\r\n");
        summary.push_str(self.controller.runtime_notice);
        for receipt in &self.controller.runtime_settings {
            let config = &receipt.active;
            let scheme = match config.scheme {
                InputScheme::Full => "全拼",
                InputScheme::Abbreviated => "简拼",
                InputScheme::Xiaohe => "小鹤双拼",
            };
            let fuzzy = config
                .fuzzy_pairs
                .iter()
                .map(|pair| pair.label())
                .collect::<Vec<_>>()
                .join("、");
            let _ = write!(
                summary,
                "\r\n宿主 {} · PID {} · {}\r\n生效 {} · 代次 {} · 待应用 {}\r\n{scheme} · 模糊音 {} · 纠错 {} · 混排 {} · 每页 {} / 上限 {}\r\n快照 {}\r\n{}\r\n",
                receipt.host_id,
                receipt.process_id,
                receipt.status.label(),
                config.revision,
                receipt.generation,
                receipt
                    .pending_revision
                    .map_or_else(|| "—".into(), |revision| revision.to_string()),
                if fuzzy.is_empty() { "关" } else { &fuzzy },
                if config.correction { "开" } else { "关" },
                if config.mixed_input { "开" } else { "关" },
                config.page_size,
                config.max_candidates,
                receipt.dictionary_version.as_deref().unwrap_or("未报告"),
                receipt.reason.map_or("", |reason| reason.label())
            );
        }
        for (i, role) in ["基础词典", "补充词典"].into_iter().enumerate() {
            let _ = write!(
                summary,
                "\r\n{role} · 只读 · {}\r\n",
                if self.controller.resource_notice[i].is_empty() {
                    "尚未核验"
                } else {
                    self.controller.resource_notice[i]
                }
            );
            if let Some(identity) = &self.controller.resources[i] {
                let _ = write!(
                    summary,
                    "最近通过的版本：{} · {} 条 · {} 字节\r\n内容版本 SHA-256 {}\r\n声明来源 {}\r\n查看不会切换宿主已加载词典\r\n",
                    identity.name,
                    identity.entries,
                    identity.bytes,
                    identity.sha256,
                    identity.sources.join("、")
                );
            }
        }
        let user = &self.controller.user_settings;
        let _ = write!(
            summary,
            "\r\n用户词库 · 可编辑 · 本地用户数据\r\n版本 {} · {} 条\r\n{}\r\n",
            user.dictionary
                .generation
                .map_or_else(|| "未打开".into(), |generation| generation.to_string()),
            user.dictionary.entries.len(),
            user.notice
        );
        summary
    }
}

fn render_dictionary_identity(summary: &mut String, role: &str, identity: &DictionaryIdentity) {
    use std::fmt::Write as _;
    let _ = write!(
        summary,
        "{role} {} · 来源 {} · 许可 {} · {}\r\nRuntime SHA-256 {}\r\n来源原件 SHA-256 {}\r\n来源锁 SHA-256 {}\r\n许可证 SHA-256 {}\r\n",
        identity.version,
        identity.source_id,
        identity.license_id,
        identity.admission.label(),
        identity.runtime_sha256,
        identity.source_sha256.as_deref().unwrap_or("未报告"),
        identity.source_lock_sha256.as_deref().unwrap_or("未报告"),
        identity.license_sha256.as_deref().unwrap_or("未报告"),
    );
}

fn selected_direction(value: isize) -> DirectionChoice {
    match value {
        1 => DirectionChoice::EnglishChinese,
        2 => DirectionChoice::AutoEnglish,
        3 => DirectionChoice::AutoChinese,
        _ => DirectionChoice::ChineseEnglish,
    }
}

fn worker_stage(stage: cloudleaf_translation::WorkerStage) -> &'static str {
    use cloudleaf_translation::WorkerStage::*;
    match stage {
        NotStarted => "尚未启动",
        Starting => "正在启动",
        Verifying => "正在验证模型资源",
        Loading => "正在加载模型",
        Ready => "已就绪",
        Translating => "正在翻译",
        Cancelling => "正在取消请求",
        Failed => "发生错误",
        Closing => "正在关闭",
        Stopped => "已停止",
    }
}

fn integrity_summary(report: &cloudleaf_translation_integrity::IntegrityReport) -> String {
    let mut labels: Vec<_> = report
        .findings
        .iter()
        .filter(|finding| finding.status.requires_acknowledgement())
        .map(|finding| (finding.kind.label(), finding.status.label()))
        .collect();
    labels.sort_unstable();
    labels.dedup();
    if labels.is_empty() {
        return String::new();
    }
    let details = labels
        .into_iter()
        .map(|(kind, status)| format!("{kind}（{status}）"))
        .collect::<Vec<_>>()
        .join("、");
    format!("\r\n完整性提示：{details}。请核对原文。")
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
    impl Drop for NativeWindow {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }

    #[test]
    fn native_kind_control_uses_capabilities_instead_of_provider_label() {
        let directory =
            std::env::temp_dir().join(format!("cloudleaf-r13-mode-{}", std::process::id()));
        let mut controller =
            PanelController::new(directory, None, Arc::new(WindowsCredentialStore::default()))
                .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while controller.settings_serial == 0 {
            controller.tick();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        controller
            .apply(Settings {
                provider: ProviderChoice::Online,
                endpoint: "https://example.invalid/v1/chat/completions".into(),
                model: "mechanism-no-network-request".into(),
                network_enabled: true,
                ..Settings::default()
            })
            .unwrap();
        while !controller.available() {
            controller.tick();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        // 此处只核渲染与能力契约；没有提交请求，不冒充真实离线推理。
        controller.settings.provider = ProviderChoice::OfflineModel;
        unsafe {
            let window = NativeWindow(CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("R13 capability fixture").as_ptr(),
                0,
                0,
                0,
                1120,
                920,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            ));
            assert!(!window.0.is_null());
            let mut state = WindowState::new(controller, window.0);
            state.create_controls(window.0);
            state.render();
            assert_eq!(SendMessageW(state.control(KIND), CB_GETCOUNT, 0, 0), 3);
            assert_ne!(
                windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(
                    state.control(KIND)
                ),
                0,
                "具备三种任务能力时应允许选择整句模式"
            );
            state.controller.begin_close();
        }
    }

    #[test]
    fn rendering_unavailable_provider_preserves_status_and_error() {
        let directory =
            std::env::temp_dir().join(format!("cloudleaf-r12-unavailable-{}", std::process::id()));
        let mut controller =
            PanelController::new(directory, None, Arc::new(WindowsCredentialStore::default()))
                .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while controller.settings_serial == 0 {
            controller.tick();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        controller.kind = TranslationKind::Phrase;
        controller.status = crate::controller::ViewStatus::ProviderUnavailable;
        let error = controller.error.clone();
        unsafe {
            let window = NativeWindow(CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("R12 unavailable state fixture").as_ptr(),
                0,
                0,
                0,
                1120,
                850,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            ));
            assert!(!window.0.is_null());
            let mut state = WindowState::new(controller, window.0);
            state.create_controls(window.0);
            state.render();
            assert_eq!(
                state.controller.status,
                crate::controller::ViewStatus::ProviderUnavailable
            );
            assert_eq!(state.controller.error, error, "渲染不能清除提供者错误");
            state.controller.begin_close();
        }
    }

    #[test]
    fn cancel_and_delete_clear_unsaved_password_text() {
        let directory =
            std::env::temp_dir().join(format!("cloudleaf-r12-password-{}", std::process::id()));
        let mut controller =
            PanelController::new(directory, None, Arc::new(WindowsCredentialStore::default()))
                .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while controller.settings_serial == 0 {
            controller.tick();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        unsafe {
            let window = NativeWindow(CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("R12 password lifecycle fixture").as_ptr(),
                0,
                0,
                0,
                1120,
                850,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            ));
            assert!(!window.0.is_null());
            let mut state = WindowState::new(controller, window.0);
            state.create_controls(window.0);
            state.render();
            // endpoint为空，删除仅验证UI清理，不会访问任何生产凭据。
            for action in [CANCEL, DELETE_KEY, RESET] {
                SetWindowTextW(
                    state.control(KEY),
                    wide("unsaved-mechanism-secret").as_ptr(),
                );
                state.command(window.0, action);
                assert_eq!(
                    GetWindowTextLengthW(state.control(KEY)),
                    0,
                    "取消、删除、重置不得保留未保存密钥"
                );
            }
            state.controller.begin_close();
        }
    }

    #[test]
    fn refresh_preserves_native_selection_until_the_change_notification() {
        let directory =
            std::env::temp_dir().join(format!("cloudleaf-r12-native-{}", std::process::id()));
        let mut controller =
            PanelController::new(directory, None, Arc::new(WindowsCredentialStore::default()))
                .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while controller.settings_serial == 0 {
            controller.tick();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        unsafe {
            let window = NativeWindow(CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("R12 native control regression").as_ptr(),
                0,
                0,
                0,
                1120,
                850,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            ));
            assert!(!window.0.is_null());
            let mut state = WindowState::new(controller, window.0);
            state.create_controls(window.0);
            state.render();
            SendMessageW(state.control(DIRECTION), CB_SETCURSEL, 1, 0);
            state.render();
            assert_eq!(
                SendMessageW(state.control(DIRECTION), CB_GETCURSEL, 0, 0),
                1,
                "刷新不能覆盖用户尚在选择的方向"
            );
            state.command(window.0, ((CBN_SELCHANGE as usize) << 16) | DIRECTION);
            assert_eq!(state.controller.direction, DirectionChoice::EnglishChinese);
            state.controller.begin_close();
        }
    }
}
