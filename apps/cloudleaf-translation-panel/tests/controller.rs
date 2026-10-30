use cloudleaf_translation::*;
use cloudleaf_translation_panel::controller::{PanelController, ViewStatus};
use cloudleaf_translation_panel::platform::{CredentialStore, PlatformError};
use cloudleaf_translation_panel::settings::{DirectionChoice, ProviderChoice, Settings};
use ring::digest::{SHA256, digest};
use serde_json::json;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Default)]
struct FixtureStore(SessionSecretStore);
impl SecretStore for FixtureStore {
    fn get(&self, r: &str) -> Result<Secret, TranslationError> {
        self.0.get(r)
    }
}
impl CredentialStore for FixtureStore {
    fn set(&self, r: &str, s: Secret) -> Result<(), PlatformError> {
        self.0
            .insert(r.into(), s)
            .map_err(|_| PlatformError::Unavailable)
    }
    fn delete(&self, r: &str) -> Result<bool, PlatformError> {
        self.0.remove(r).map_err(|_| PlatformError::Unavailable)
    }
}
static NEXT: AtomicUsize = AtomicUsize::new(1);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "cloudleaf-r12-controller-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn catalog(&self) -> PathBuf {
        let package = self.0.join("dictionary");
        fs::create_dir_all(&package).unwrap();
        let mut hashes = Vec::new();
        for (name, text) in [
            ("LICENSE.txt", "MIT original mechanism fixture"),
            ("NOTICE.txt", "Not product gold"),
            ("SOURCE.json", "{}"),
        ] {
            fs::write(package.join(name), text).unwrap();
            hashes.push(
                digest(&SHA256, text.as_bytes())
                    .as_ref()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
            );
        }
        let value = json!({"format":"cloudleaf-translation-dictionary-v1","schema_version":1,"version":"r12-mechanism",
            "sources":[{"id":"fixture","version":"1","artifact_sha256":"1".repeat(64),"license_id":"MIT","license_file":"LICENSE.txt","license_sha256":hashes[0],"notice_file":"NOTICE.txt","notice_sha256":hashes[1],"source_lock_file":"SOURCE.json","source_lock_sha256":hashes[2],"review_status":"approved_bounded_scope","scope":["raw_gloss"]}],
            "entries":[{"id":"fixture-1","source_id":"fixture","source_record_id":"original-1","headword":"你好🙂","raw_glosses":["mechanism gloss🙂"],"status":"research_only"}],"mappings":[]});
        let path = package.join("catalog.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn wait(panel: &mut PanelController, predicate: impl Fn(&PanelController) -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        panel.tick();
        if predicate(panel) {
            break;
        }
        assert!(
            Instant::now() < end,
            "status {:?} error {:?}",
            panel.status,
            panel.error
        );
        thread::sleep(Duration::from_millis(2));
    }
}

fn start_after_queue_drains(panel: &mut PanelController) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match panel.start() {
            Ok(()) => return,
            Err(TranslationError::Busy) => {
                assert!(Instant::now() < deadline);
                thread::yield_now();
            }
            Err(error) => panic!("unexpected request failure: {error}"),
        }
    }
}

#[test]
fn actual_dictionary_roundtrip_keeps_research_status_original_and_capabilities() {
    let f = Fixture::new();
    let catalog = f.catalog();
    let mut panel = PanelController::new(
        f.0.join("settings"),
        Some(catalog),
        Arc::new(FixtureStore::default()),
    )
    .unwrap();
    wait(&mut panel, |p| p.status == ViewStatus::Idle);
    assert!(!panel.supports_kind(TranslationKind::Sentence));
    assert!(panel.supports_auto());
    panel.set_text("你好🙂".into());
    panel.start().unwrap();
    wait(&mut panel, |p| p.status == ViewStatus::Completed);
    assert_eq!(panel.copy_text().unwrap(), "mechanism gloss🙂");
    assert_eq!(panel.text, "你好🙂");
    assert_eq!(
        panel.result().unwrap().output.quality,
        QualityStatus::ResearchOnly
    );
    panel.set_direction(DirectionChoice::EnglishChinese);
    assert!(panel.result().is_none());
    panel.set_text("mechanism gloss🙂".into());
    panel.start().unwrap();
    wait(&mut panel, |p| p.status == ViewStatus::Completed);
    assert_eq!(panel.copy_text().unwrap(), "你好🙂");
    panel.set_text("中文English\n🙂".repeat(2000));
    assert_eq!(panel.start(), Err(TranslationError::InputTooLong));
    panel.begin_close();
    wait(&mut panel, PanelController::close_ready);
}

#[test]
fn repeated_actions_changes_and_focus_invalidate_without_losing_input() {
    let f = Fixture::new();
    let catalog = f.catalog();
    let mut p = PanelController::new(
        f.0.join("s"),
        Some(catalog),
        Arc::new(FixtureStore::default()),
    )
    .unwrap();
    wait(&mut p, |p| p.status == ViewStatus::Idle);
    p.set_text("你好🙂".into());
    for _ in 0..30 {
        let result = p.start();
        assert!(result.is_ok() || result == Err(TranslationError::Busy));
    }
    p.cancel();
    for _ in 0..20 {
        p.tick();
        thread::yield_now();
    }
    assert!(p.result().is_none());
    assert_eq!(p.text, "你好🙂");
    start_after_queue_drains(&mut p);
    p.focus(false);
    p.tick();
    assert!(p.result().is_none());
    assert_eq!(p.start(), Err(TranslationError::StaleResult));
    p.focus(true);
    start_after_queue_drains(&mut p);
    p.mark_settings_dirty();
    assert!(p.result().is_none());
    assert_eq!(p.start(), Err(TranslationError::Busy));
    p.begin_close();
    wait(&mut p, PanelController::close_ready);
}

struct Server {
    endpoint: String,
    stop: Arc<AtomicBool>,
    requests: Arc<AtomicUsize>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let (s, r) = (stop.clone(), requests.clone());
        let worker = thread::spawn(move || {
            let threads = Mutex::new(Vec::new());
            while !s.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        socket.set_nonblocking(false).unwrap();
                        socket
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let r = r.clone();
                        threads.lock().unwrap().push(thread::spawn(move||{
            let mut raw=Vec::new();let mut buffer=[0;4096];let body=loop{let n=socket.read(&mut buffer).unwrap_or(0);if n==0{return;}raw.extend_from_slice(&buffer[..n]);if let Some(pos)=raw.windows(4).position(|v|v==b"\r\n\r\n"){let head=String::from_utf8_lossy(&raw[..pos]);let len=head.lines().find_map(|l|l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v|v.trim().parse::<usize>().ok())).unwrap();if raw.len()>=pos+4+len{break String::from_utf8(raw[pos+4..].to_vec()).unwrap();}}};
            r.fetch_add(1,Ordering::AcqRel);let delayed=body.contains("old")||body.contains("timeout");if delayed{thread::sleep(Duration::from_millis(if body.contains("timeout"){1600}else{350}));}
            let (code,body)=if body.contains("old-error"){(401,"{}".to_string())}else{(200,json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"local server generated fixture"}}]}).to_string())};
            let _=write!(socket,"HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
        }));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
            for t in threads.into_inner().unwrap() {
                t.join().unwrap();
            }
        });
        Self {
            endpoint,
            stop,
            requests,
            worker: Some(worker),
        }
    }
    fn await_request(&self, n: usize) {
        let end = Instant::now() + Duration::from_secs(4);
        while self.requests.load(Ordering::Acquire) < n {
            assert!(Instant::now() < end);
            thread::sleep(Duration::from_millis(2));
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn online(f: &Fixture, server: &Server) -> (PanelController, Arc<FixtureStore>) {
    let store = Arc::new(FixtureStore::default());
    let mut p = PanelController::new(f.0.join("s"), None, store.clone()).unwrap();
    wait(&mut p, |p| p.status != ViewStatus::Preparing);
    let settings = Settings {
        provider: ProviderChoice::Online,
        endpoint: server.endpoint.clone(),
        model: "fixture-only".into(),
        network_enabled: true,
        allow_loopback_http: true,
        connect_timeout_secs: 1,
        total_timeout_secs: 1,
        ..Settings::default()
    };
    store
        .set(
            &settings.credential_reference(),
            Secret::new("fixture-not-real".into()).unwrap(),
        )
        .unwrap();
    p.apply(settings).unwrap();
    wait(&mut p, |p| p.status == ViewStatus::Idle);
    (p, store)
}

#[test]
fn online_requires_explicit_authorization_and_late_errors_cannot_replace_new_success() {
    let f = Fixture::new();
    let server = Server::new();
    let (mut p, _) = online(&f, &server);
    p.set_text("old-error".into());
    assert_eq!(p.start(), Err(TranslationError::OnlineNotAuthorized));
    assert_eq!(server.requests.load(Ordering::Acquire), 0);
    p.authorized = true;
    p.start().unwrap();
    assert!(!p.authorized, "单次发送授权在构造请求后消费");
    server.await_request(1);
    p.set_text("new request".into());
    p.authorized = true;
    p.start().unwrap();
    wait(&mut p, |p| p.status == ViewStatus::Completed);
    let id = p.result().unwrap().task_id;
    thread::sleep(Duration::from_millis(400));
    p.tick();
    assert_eq!(p.status, ViewStatus::Completed);
    assert_eq!(p.result().unwrap().task_id, id);
    assert_eq!(p.text, "new request");
    let mut settings = p.settings.clone();
    settings.model = "changed-model".into();
    p.apply(settings).unwrap();
    wait(&mut p, |p| p.status == ViewStatus::Idle);
    assert!(!p.authorized);
    assert!(p.result().is_none());
    assert_eq!(p.start(), Err(TranslationError::OnlineNotAuthorized));
    p.begin_close();
    wait(&mut p, PanelController::close_ready);
}

#[test]
fn cancel_late_success_timeout_offline_and_provider_switch_are_real() {
    let f = Fixture::new();
    let server = Server::new();
    let (mut p, _) = online(&f, &server);
    p.set_text("old-success".into());
    p.authorized = true;
    p.start().unwrap();
    server.await_request(1);
    p.cancel();
    thread::sleep(Duration::from_millis(400));
    p.tick();
    assert_eq!(p.status, ViewStatus::Cancelled);
    assert!(p.result().is_none());
    p.set_text("timeout".into());
    p.authorized = true;
    p.start().unwrap();
    server.await_request(2);
    wait(&mut p, |p| p.status == ViewStatus::TimedOut);
    assert!(p.result().is_none());
    let mut s = p.settings.clone();
    s.network_enabled = false;
    p.apply(s).unwrap();
    wait(&mut p, |p| p.status == ViewStatus::ProviderUnavailable);
    assert!(!p.available());
    p.authorized = true;
    assert_eq!(p.start(), Err(TranslationError::ProviderUnavailable));
    assert_eq!(server.requests.load(Ordering::Acquire), 2);
    let mut s = p.settings.clone();
    s.provider = ProviderChoice::OfflineModel;
    p.apply(s).unwrap();
    wait(&mut p, |p| p.status == ViewStatus::ProviderUnavailable);
    assert!(p.notice.contains("未安装"));
    assert_eq!(p.start(), Err(TranslationError::ProviderUnavailable));
    p.begin_close();
    wait(&mut p, PanelController::close_ready);
}

#[cfg(windows)]
#[test]
fn controller_key_commands_use_real_isolated_windows_credentials() {
    use cloudleaf_translation_panel::platform::WindowsCredentialStore;
    let f = Fixture::new();
    let namespace = format!(
        "Cloudleaf/TranslationPanel.Test.controller-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let store = Arc::new(WindowsCredentialStore::new(&namespace).unwrap());
    let mut p = PanelController::new(f.0.join("s"), None, store.clone()).unwrap();
    wait(&mut p, |p| p.status != ViewStatus::Preparing);
    let settings = Settings {
        endpoint: "https://example.invalid/v1/chat/completions".into(),
        model: "fixture".into(),
        ..Settings::default()
    };
    let reference = settings.credential_reference();
    struct Cleanup(Arc<WindowsCredentialStore>, String);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = self.0.delete(&self.1);
        }
    }
    let _cleanup = Cleanup(store.clone(), reference.clone());
    p.save_key(
        settings.clone(),
        Secret::new("isolated-controller-fixture".into()).unwrap(),
    )
    .unwrap();
    wait(&mut p, |p| p.notice.contains("密钥已保存"));
    assert_eq!(
        store.read(&reference).unwrap().expose(),
        "isolated-controller-fixture"
    );
    assert!(
        !fs::read_to_string(f.0.join("s/settings.json"))
            .unwrap_or_default()
            .contains("isolated-controller-fixture")
    );
    p.delete_key(settings).unwrap();
    wait(&mut p, |p| p.notice.contains("已删除"));
    assert_eq!(store.read(&reference).unwrap_err(), PlatformError::NotFound);
    p.begin_close();
    wait(&mut p, PanelController::close_ready);
}
