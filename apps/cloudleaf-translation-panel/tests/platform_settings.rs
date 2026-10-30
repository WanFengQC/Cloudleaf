use cloudleaf_translation::Secret;
use cloudleaf_translation_panel::platform::{
    PlatformError, WindowsCredentialStore, unicode_clipboard_text,
};
use cloudleaf_translation_panel::settings::{
    DirectionChoice, ProviderChoice, Settings, SettingsRepository,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        let name = format!(
            "Cloudleaf-Round12-Settings-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("Cloudleaf-Round12-Settings-")
        );
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn defaults_and_strict_settings_validation_never_enable_network() {
    let settings = Settings::default();
    assert_eq!(settings.schema_version, 2);
    assert_eq!(settings.revision, 1);
    assert_eq!(settings.provider, ProviderChoice::Dictionary);
    assert_eq!(settings.direction, DirectionChoice::ChineseEnglish);
    assert!(!settings.network_enabled && !settings.allow_loopback_http);
    settings.validate().unwrap();
    let mut invalid = settings.clone();
    invalid.provider = ProviderChoice::Online;
    assert!(invalid.validate().is_err());
    invalid.endpoint = "https://example.invalid/v1/chat/completions".into();
    invalid.model = "fixture-model".into();
    invalid.validate().unwrap();
    for endpoint in [
        "http://example.invalid/v1/chat/completions",
        "https://user:secret@example.invalid/api",
        "https://example.invalid/api?key=secret",
        "https://example.invalid/api#fragment",
    ] {
        invalid.endpoint = endpoint.into();
        assert!(invalid.validate().is_err());
    }
    invalid.endpoint = "http://127.0.0.1:9988/v1/chat/completions".into();
    invalid.allow_loopback_http = true;
    invalid.validate().unwrap();
    invalid.connect_timeout_secs = invalid.total_timeout_secs + 1;
    assert!(invalid.validate().is_err());
    invalid.connect_timeout_secs = 1;
    invalid.revision = 0;
    assert!(invalid.validate().is_err());
    let mut value = serde_json::to_value(settings).unwrap();
    value["api_key"] = "fixture-not-real".into();
    assert!(serde_json::from_value::<Settings>(value).is_err());
}

#[test]
fn reference_is_endpoint_bound_and_settings_debug_excludes_content() {
    let first = Settings {
        endpoint: "https://first.invalid/v1/chat/completions".into(),
        model: "private-model-name".into(),
        ..Settings::default()
    };
    let second = Settings {
        endpoint: "https://second.invalid/v1/chat/completions".into(),
        ..first.clone()
    };
    assert_ne!(first.credential_reference(), second.credential_reference());
    assert_eq!(first.credential_reference().len(), 68);
    assert!(!format!("{first:?}").contains("first.invalid"));
    assert!(!format!("{first:?}").contains("private-model-name"));
}

#[test]
fn saved_settings_reload_offline_and_valid_previous_version_is_retained() {
    let directory = TestDirectory::new();
    let repository = SettingsRepository::new(directory.0.clone());
    assert_eq!(repository.load().unwrap().settings, Settings::default());
    let first = Settings {
        model: "first-fixture".into(),
        ..Settings::default()
    };
    repository.save(&first).unwrap();
    let second = Settings {
        revision: 2,
        model: "second-fixture".into(),
        network_enabled: true,
        ..first.clone()
    };
    repository.save(&second).unwrap();
    let loaded = repository.load().unwrap();
    assert_eq!(loaded.settings.model, "second-fixture");
    assert!(!loaded.settings.network_enabled);
    assert!(loaded.notice.is_some());
    let backup: Settings =
        serde_json::from_slice(&std::fs::read(directory.0.join("settings.json.bak")).unwrap())
            .unwrap();
    assert_eq!(backup, first);
    let saved = std::fs::read_to_string(directory.0.join("settings.json")).unwrap();
    assert!(!saved.contains("api_key") && !saved.contains("credential_reference"));
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 2);
}

#[test]
fn corruption_recovery_preserves_original_and_does_not_overwrite_valid_backup() {
    let directory = TestDirectory::new();
    let repository = SettingsRepository::new(directory.0.clone());
    let first = Settings {
        model: "backup-fixture".into(),
        ..Settings::default()
    };
    repository.save(&first).unwrap();
    repository
        .save(&Settings {
            revision: 2,
            ..first.clone()
        })
        .unwrap();
    let damaged = b"{ damaged fixture";
    std::fs::write(directory.0.join("settings.json"), damaged).unwrap();
    let backup_before = std::fs::read(directory.0.join("settings.json.bak")).unwrap();
    let loaded = repository.load().unwrap();
    assert_eq!(loaded.settings.model, first.model);
    assert!(loaded.notice.is_some());
    assert_eq!(
        std::fs::read(directory.0.join("settings.json")).unwrap(),
        damaged
    );
    repository.save(&Settings::default()).unwrap();
    assert_eq!(
        std::fs::read(directory.0.join("settings.json.bak")).unwrap(),
        backup_before
    );
    std::fs::write(directory.0.join("settings.json"), b"bad").unwrap();
    std::fs::write(directory.0.join("settings.json.bak"), b"bad").unwrap();
    let loaded = repository.load().unwrap();
    assert_eq!(loaded.settings, Settings::default());
    assert!(loaded.notice.is_some());
}

#[test]
fn migration_unknown_fields_size_and_interrupted_temporary_files_are_safe() {
    let directory = TestDirectory::new();
    let repository = SettingsRepository::new(directory.0.clone());
    let source = serde_json::json!({"schema_version":1,"provider":"online","endpoint":"https://example.invalid/api","model":"fixture","direction":"english_chinese","network_enabled":true,"allow_loopback_http":true});
    let encoded = serde_json::to_vec(&source).unwrap();
    std::fs::write(directory.0.join("settings.json"), &encoded).unwrap();
    let loaded = repository.load().unwrap();
    assert_eq!(loaded.settings.schema_version, 2);
    assert_eq!(loaded.settings.direction, DirectionChoice::EnglishChinese);
    assert!(!loaded.settings.network_enabled && !loaded.settings.allow_loopback_http);
    assert!(loaded.notice.is_some());
    assert_eq!(
        std::fs::read(directory.0.join("settings.json")).unwrap(),
        encoded
    );
    let mut injected = source;
    injected["api_key"] = "fixture-never-allowed".into();
    std::fs::write(
        directory.0.join("settings.json"),
        serde_json::to_vec(&injected).unwrap(),
    )
    .unwrap();
    assert_eq!(repository.load().unwrap().settings, Settings::default());
    std::fs::write(directory.0.join("settings.json"), vec![b' '; 32 * 1024 + 1]).unwrap();
    assert_eq!(repository.load().unwrap().settings, Settings::default());
    repository.save(&Settings::default()).unwrap();
    std::fs::write(
        directory.0.join(".settings-interrupted.tmp"),
        b"partial new fixture",
    )
    .unwrap();
    assert_eq!(repository.load().unwrap().settings, Settings::default());
    assert_eq!(
        std::fs::read(directory.0.join(".settings-interrupted.tmp")).unwrap(),
        b"partial new fixture"
    );
}

#[test]
fn clipboard_encoding_is_unicode_crlf_and_nul_terminated() {
    let encoded = unicode_clipboard_text("中文🙂\nnext\r\nlast\rtail").unwrap();
    assert_eq!(encoded.last(), Some(&0));
    assert_eq!(
        String::from_utf16(&encoded[..encoded.len() - 1]).unwrap(),
        "中文🙂\r\nnext\r\nlast\r\ntail"
    );
    assert_eq!(
        unicode_clipboard_text("bad\0text"),
        Err(PlatformError::InvalidData)
    );
}

#[test]
fn credential_namespaces_are_application_owned_and_debug_redacted() {
    assert!(WindowsCredentialStore::new("Cloudleaf/TranslationPanel").is_ok());
    assert!(WindowsCredentialStore::new("Cloudleaf/TranslationPanel.Test.fixture").is_ok());
    assert!(WindowsCredentialStore::new("OtherApp/secret").is_err());
    assert!(!format!("{:?}", WindowsCredentialStore::default()).contains("api-"));
}

#[cfg(windows)]
#[test]
fn isolated_real_windows_credential_roundtrip_update_delete_and_limit() {
    let namespace = format!(
        "Cloudleaf/TranslationPanel.Test.{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let store = WindowsCredentialStore::new(&namespace).unwrap();
    struct Cleanup<'a>(&'a WindowsCredentialStore);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.delete("fixture");
        }
    }
    let _cleanup = Cleanup(&store);
    assert_eq!(store.read("fixture").unwrap_err(), PlatformError::NotFound);
    store
        .set(
            "fixture",
            Secret::new("fixture-only-中文🙂".into()).unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.read("fixture").unwrap().expose(),
        "fixture-only-中文🙂"
    );
    store
        .set("fixture", Secret::new("fixture-updated".into()).unwrap())
        .unwrap();
    assert_eq!(store.read("fixture").unwrap().expose(), "fixture-updated");
    assert_eq!(
        store.set("fixture", Secret::new("x".repeat(2561)).unwrap()),
        Err(PlatformError::TooLong)
    );
    assert_eq!(store.read("fixture").unwrap().expose(), "fixture-updated");
    assert!(store.delete("fixture").unwrap());
    store
        .set("fixture", Secret::new("x".repeat(2560)).unwrap())
        .unwrap();
    assert_eq!(store.read("fixture").unwrap().expose().len(), 2560);
    assert!(store.delete("fixture").unwrap());
    assert!(!store.delete("fixture").unwrap());
    assert_eq!(store.read("fixture").unwrap_err(), PlatformError::NotFound);
    assert_eq!(
        store.read("invalid/reference").unwrap_err(),
        PlatformError::InvalidData
    );
}

#[cfg(windows)]
#[test]
#[allow(unsafe_code)]
fn corrupt_real_credential_is_rejected_and_isolated_fixture_is_removed() {
    use windows_sys::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredWriteW,
    };
    let namespace = format!(
        "Cloudleaf/TranslationPanel.Test.corrupt-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let store = WindowsCredentialStore::new(&namespace).unwrap();
    struct Cleanup<'a>(&'a WindowsCredentialStore);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.delete("fixture");
        }
    }
    let _cleanup = Cleanup(&store);
    let mut target: Vec<u16> = format!("{namespace}/fixture")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut bytes = [0xff, 0x80];
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_mut_ptr(),
        CredentialBlob: bytes.as_mut_ptr(),
        CredentialBlobSize: bytes.len() as u32,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..CREDENTIALW::default()
    };
    assert_ne!(unsafe { CredWriteW(&credential, 0) }, 0);
    assert_eq!(
        store.read("fixture").unwrap_err(),
        PlatformError::InvalidData
    );
    assert!(store.delete("fixture").unwrap());
}

#[cfg(windows)]
#[test]
#[allow(unsafe_code)]
fn busy_clipboard_is_reported_without_reading_or_replacing_existing_content() {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, OpenClipboard};
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow};
    struct Window(HWND);
    impl Window {
        fn create() -> Self {
            let class: Vec<u16> = "STATIC".encode_utf16().chain(Some(0)).collect();
            let window = unsafe {
                CreateWindowExW(
                    0,
                    class.as_ptr(),
                    std::ptr::null(),
                    0,
                    0,
                    0,
                    20,
                    20,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                )
            };
            assert!(!window.is_null());
            Self(window)
        }
    }
    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let owner = thread::spawn(move || {
        let window = Window::create();
        let opened = unsafe { OpenClipboard(window.0) } != 0;
        ready_tx.send(opened).unwrap();
        if opened {
            let _ = release_rx.recv_timeout(Duration::from_secs(10));
            unsafe {
                CloseClipboard();
            }
        }
    });
    let opened = ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let window = Window::create();
    let result =
        cloudleaf_translation_panel::platform::copy_unicode(window.0 as isize, "原创测试文本🙂");
    let _ = release_tx.send(());
    owner.join().unwrap();
    assert!(opened, "无法取得隔离剪贴板占用fixture的系统访问权");
    assert_eq!(result, Err(PlatformError::ClipboardBusy));
}

#[cfg(not(windows))]
#[test]
fn unsupported_platform_never_falls_back_to_a_plaintext_store() {
    let store = WindowsCredentialStore::default();
    assert_eq!(
        store.read("fixture").unwrap_err(),
        PlatformError::Unsupported
    );
    assert_eq!(
        store.set("fixture", Secret::new("fixture".into()).unwrap()),
        Err(PlatformError::Unsupported)
    );
    assert_eq!(store.delete("fixture"), Err(PlatformError::Unsupported));
}
