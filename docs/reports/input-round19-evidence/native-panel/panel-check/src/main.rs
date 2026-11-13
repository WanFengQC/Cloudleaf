#![allow(dead_code)]
mod isolated_input {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    use windows::Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    };
    use windows_core::{Error, PCWSTR, Result, w};

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain([0]).collect()
    }
    fn boundary<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
        f()
    }

    #[path = "C:/Users/EDY/.codex/tmp/r19-host-review/user_panel.rs"]
    mod user_panel;
}
fn main() {}
