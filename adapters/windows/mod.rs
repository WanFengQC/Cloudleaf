//! 原生 Windows 集成，所有系统依赖和不安全边界限制在此模块树。
#![allow(non_snake_case)]
mod budget;
mod candidate;
mod keys;
mod owner_cleanup;
mod ownership;
mod recovery;
mod registration;
mod service;
mod session;
mod watchdog;
use std::panic::{AssertUnwindSafe, catch_unwind};
use windows::Win32::Foundation::E_UNEXPECTED;
use windows_core::{Error, GUID, Result};
pub const SERVICE_ID: GUID = GUID::from_u128(0xf573e0d6_2c67_4fd8_a75a_7a58278dc1b0);
pub const PROFILE_ID: GUID = GUID::from_u128(0xd845b8e7_f6c2_4d28_9bc1_cac205f9f62b);
pub const LANGUAGE: u16 = 0x0804;
pub(crate) fn boundary<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| Err(Error::from_hresult(E_UNEXPECTED)))
}
pub fn manage(args: Vec<String>) -> Result<()> {
    registration::manage(args)
}
pub use service::{DllCanUnloadNow, DllGetClassObject};
