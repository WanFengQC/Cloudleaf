use cloudleaf_translation::{Secret, SecretStore, TranslationError};
use std::fmt;

const PRODUCTION_NAMESPACE: &str = "Cloudleaf/TranslationPanel";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformError {
    Unsupported,
    AccessDenied,
    NotFound,
    InvalidData,
    TooLong,
    Unavailable,
    ClipboardBusy,
    ClipboardFailed,
}

impl fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unsupported => "当前平台不支持此系统功能",
            Self::AccessDenied => "系统拒绝访问",
            Self::NotFound => "尚未保存密钥",
            Self::InvalidData => "系统数据或引用无效",
            Self::TooLong => "内容超过系统存储限制",
            Self::Unavailable => "系统功能当前不可用",
            Self::ClipboardBusy => "剪贴板正被其他程序占用，请稍后重试",
            Self::ClipboardFailed => "复制失败，译文仍保留在面板中",
        })
    }
}

impl std::error::Error for PlatformError {}

pub trait CredentialStore: SecretStore {
    fn set(&self, reference: &str, secret: Secret) -> Result<(), PlatformError>;
    fn delete(&self, reference: &str) -> Result<bool, PlatformError>;
}

pub struct WindowsCredentialStore {
    namespace: String,
}

impl WindowsCredentialStore {
    pub fn new(namespace: &str) -> Result<Self, PlatformError> {
        let test_suffix = namespace.strip_prefix("Cloudleaf/TranslationPanel.Test.");
        if namespace != PRODUCTION_NAMESPACE
            && !test_suffix.is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix.len() <= 128
                    && suffix
                        .bytes()
                        .all(|value| value.is_ascii_alphanumeric() || b"-_.".contains(&value))
            })
        {
            return Err(PlatformError::InvalidData);
        }
        Ok(Self {
            namespace: namespace.to_owned(),
        })
    }

    pub fn set(&self, reference: &str, secret: Secret) -> Result<(), PlatformError> {
        let target = self.target(reference)?;
        if secret.expose().len() > 2560 {
            return Err(PlatformError::TooLong);
        }
        system::write(&target, secret)
    }

    pub fn read(&self, reference: &str) -> Result<Secret, PlatformError> {
        system::read(&self.target(reference)?)
    }

    pub fn delete(&self, reference: &str) -> Result<bool, PlatformError> {
        system::delete(&self.target(reference)?)
    }

    fn target(&self, reference: &str) -> Result<Vec<u16>, PlatformError> {
        if reference.is_empty()
            || reference.len() > 128
            || !reference
                .bytes()
                .all(|value| value.is_ascii_alphanumeric() || b"-_.:".contains(&value))
        {
            return Err(PlatformError::InvalidData);
        }
        Ok(format!("{}/{}", self.namespace, reference)
            .encode_utf16()
            .chain(Some(0))
            .collect())
    }
}

impl Default for WindowsCredentialStore {
    fn default() -> Self {
        Self {
            namespace: PRODUCTION_NAMESPACE.to_owned(),
        }
    }
}

impl fmt::Debug for WindowsCredentialStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WindowsCredentialStore([REDACTED])")
    }
}

impl CredentialStore for WindowsCredentialStore {
    fn set(&self, reference: &str, secret: Secret) -> Result<(), PlatformError> {
        WindowsCredentialStore::set(self, reference, secret)
    }
    fn delete(&self, reference: &str) -> Result<bool, PlatformError> {
        WindowsCredentialStore::delete(self, reference)
    }
}

impl SecretStore for WindowsCredentialStore {
    fn get(&self, reference: &str) -> Result<Secret, TranslationError> {
        self.read(reference).map_err(|error| match error {
            PlatformError::NotFound => TranslationError::MissingSecret,
            _ => TranslationError::SecretStoreUnavailable,
        })
    }
}

pub fn unicode_clipboard_text(text: &str) -> Result<Vec<u16>, PlatformError> {
    if text.contains('\0') {
        return Err(PlatformError::InvalidData);
    }
    if text.len() > 64 * 1024 {
        return Err(PlatformError::TooLong);
    }
    Ok(text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
        .encode_utf16()
        .chain(Some(0))
        .collect())
}

pub fn copy_unicode(hwnd: isize, text: &str) -> Result<(), PlatformError> {
    system::copy(hwnd, &unicode_clipboard_text(text)?)
}

#[cfg(not(windows))]
mod system {
    use super::{PlatformError, Secret};
    pub(super) fn write(_: &[u16], _: Secret) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }
    pub(super) fn read(_: &[u16]) -> Result<Secret, PlatformError> {
        Err(PlatformError::Unsupported)
    }
    pub(super) fn delete(_: &[u16]) -> Result<bool, PlatformError> {
        Err(PlatformError::Unsupported)
    }
    pub(super) fn copy(_: isize, _: &[u16]) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod system {
    use super::{PlatformError, Secret};
    use std::ptr;
    use windows_sys::Win32::Foundation::{
        ERROR_ACCESS_DENIED, ERROR_NOT_FOUND, ERROR_SUCCESS, GetLastError, GlobalFree, HGLOBAL,
        HWND, SetLastError,
    };
    use windows_sys::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
        CredReadW, CredWriteW,
    };
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
    };
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
    use zeroize::Zeroize;

    fn error(code: u32) -> PlatformError {
        match code {
            ERROR_ACCESS_DENIED => PlatformError::AccessDenied,
            ERROR_NOT_FOUND => PlatformError::NotFound,
            _ => PlatformError::Unavailable,
        }
    }

    pub(super) fn write(target: &[u16], secret: Secret) -> Result<(), PlatformError> {
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_ptr().cast_mut(),
            CredentialBlobSize: secret.expose().len() as u32,
            CredentialBlob: secret.expose().as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..CREDENTIALW::default()
        };
        // Win32在返回前复制blob，owned Secret随后清零释放。
        if unsafe { CredWriteW(&credential, 0) } == 0 {
            return Err(error(unsafe { GetLastError() }));
        }
        Ok(())
    }

    struct ReadCredential(*mut CREDENTIALW);
    impl Drop for ReadCredential {
        fn drop(&mut self) {
            unsafe {
                let credential = &mut *self.0;
                if !credential.CredentialBlob.is_null() && credential.CredentialBlobSize <= 2560 {
                    std::slice::from_raw_parts_mut(
                        credential.CredentialBlob,
                        credential.CredentialBlobSize as usize,
                    )
                    .zeroize();
                }
                CredFree(self.0.cast());
            }
        }
    }

    pub(super) fn read(target: &[u16]) -> Result<Secret, PlatformError> {
        let mut pointer = ptr::null_mut();
        if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pointer) } == 0 {
            return Err(error(unsafe { GetLastError() }));
        }
        if pointer.is_null() {
            return Err(PlatformError::InvalidData);
        }
        let guard = ReadCredential(pointer);
        let credential = unsafe { &*guard.0 };
        if credential.Type != CRED_TYPE_GENERIC
            || credential.CredentialBlobSize == 0
            || credential.CredentialBlobSize > 2560
            || credential.CredentialBlob.is_null()
        {
            return Err(PlatformError::InvalidData);
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
        };
        let text = std::str::from_utf8(bytes).map_err(|_| PlatformError::InvalidData)?;
        Secret::new(text.to_owned()).map_err(|_| PlatformError::InvalidData)
    }

    pub(super) fn delete(target: &[u16]) -> Result<bool, PlatformError> {
        if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } != 0 {
            return Ok(true);
        }
        let code = unsafe { GetLastError() };
        if code == ERROR_NOT_FOUND {
            Ok(false)
        } else {
            Err(error(code))
        }
    }

    struct GlobalMemory(HGLOBAL);
    impl Drop for GlobalMemory {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    GlobalFree(self.0);
                }
            }
        }
    }
    struct Clipboard;
    impl Drop for Clipboard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }

    pub(super) fn copy(hwnd: isize, text: &[u16]) -> Result<(), PlatformError> {
        if hwnd == 0 {
            return Err(PlatformError::ClipboardFailed);
        }
        let mut memory =
            GlobalMemory(unsafe { GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of_val(text)) });
        if memory.0.is_null() {
            return Err(PlatformError::ClipboardFailed);
        }
        let buffer = unsafe { GlobalLock(memory.0) };
        if buffer.is_null() {
            return Err(PlatformError::ClipboardFailed);
        }
        unsafe {
            ptr::copy_nonoverlapping(text.as_ptr(), buffer.cast::<u16>(), text.len());
            SetLastError(ERROR_SUCCESS);
            if GlobalUnlock(memory.0) == 0 && GetLastError() != ERROR_SUCCESS {
                return Err(PlatformError::ClipboardFailed);
            }
        }
        if unsafe { OpenClipboard(hwnd as HWND) } == 0 {
            return Err(PlatformError::ClipboardBusy);
        }
        let _clipboard = Clipboard;
        if unsafe { EmptyClipboard() } == 0 {
            return Err(PlatformError::ClipboardFailed);
        }
        if unsafe { SetClipboardData(CF_UNICODETEXT as u32, memory.0) }.is_null() {
            return Err(PlatformError::ClipboardFailed);
        }
        // 仅成功SetClipboardData后移交所有权，其他路径由guard释放。
        memory.0 = ptr::null_mut();
        Ok(())
    }
}
