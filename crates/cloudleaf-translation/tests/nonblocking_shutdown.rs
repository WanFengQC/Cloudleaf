use cloudleaf_translation::*;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

#[test]
fn closing_idle_workers_is_nonblocking_and_eventually_reclaims_them() {
    let mut service =
        TranslationService::new(ProviderRegistry::new(), ServiceLimits::default()).unwrap();
    service.begin_shutdown();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !service.workers_finished() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    service.shutdown().unwrap();
}

struct Blocking {
    started: mpsc::SyncSender<()>,
    release: Arc<(Mutex<bool>, Condvar)>,
    panic: bool,
}
impl TranslationProvider for Blocking {
    fn identity(&self) -> ProviderIdentity {
        ProviderIdentity {
            id: "shutdown-fixture".into(),
            version: "1".into(),
        }
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            directions: vec![TranslationDirection {
                source: Language::Chinese,
                target: Language::English,
            }],
            kinds: vec![TranslationKind::Word],
            auto_source: false,
            network: false,
            available: true,
        }
    }
    fn translate(
        &self,
        _: &TranslationRequest,
        _: &TranslationCancellation,
    ) -> Result<ProviderOutput, TranslationError> {
        self.started.send(()).unwrap();
        if self.panic {
            panic!("intentional provider-exit fixture");
        }
        let (lock, signal) = &*self.release;
        let guard = lock.lock().unwrap();
        drop(signal.wait_while(guard, |released| !*released).unwrap());
        Err(TranslationError::Cancelled)
    }
}
fn request() -> TranslationRequest {
    TranslationRequest {
        text: "机制".into(),
        direction: TranslationDirection {
            source: Language::Chinese,
            target: Language::English,
        },
        kind: TranslationKind::Word,
        context: TranslationContext {
            session: 1,
            epoch: 1,
            input_revision: 1,
        },
        send_authorized_to: None,
        use_cache: false,
    }
}

#[test]
fn noncooperative_worker_keeps_its_budget_until_real_exit() {
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    struct Release(Arc<(Mutex<bool>, Condvar)>);
    impl Drop for Release {
        fn drop(&mut self) {
            *self.0.0.lock().unwrap() = true;
            self.0.1.notify_all();
        }
    }
    let guard = Release(release.clone());
    let (started, receiver) = mpsc::sync_channel(1);
    let mut registry = ProviderRegistry::new();
    registry
        .register(
            Arc::new(Blocking {
                started,
                release,
                panic: false,
            }),
            TranslationProviderConfig::default(),
        )
        .unwrap();
    let mut service = TranslationService::new(
        registry,
        ServiceLimits {
            workers: 1,
            ..ServiceLimits::default()
        },
    )
    .unwrap();
    let id = service.submit("shutdown-fixture", request()).unwrap();
    receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    let start = Instant::now();
    service.begin_shutdown();
    assert!(start.elapsed() < Duration::from_millis(250));
    assert!(!service.workers_finished());
    assert_eq!(service.poll(id).unwrap().state, TaskState::Cancelled);
    assert_eq!(
        service.submit("shutdown-fixture", request()),
        Err(TranslationError::ProviderUnavailable)
    );
    drop(guard);
    let deadline = Instant::now() + Duration::from_secs(3);
    while !service.workers_finished() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    service.shutdown().unwrap();
}

#[test]
fn provider_thread_exit_is_reported_and_shutdown_retains_the_failure() {
    let (started, receiver) = mpsc::sync_channel(1);
    let mut registry = ProviderRegistry::new();
    registry
        .register(
            Arc::new(Blocking {
                started,
                release: Arc::new((Mutex::new(false), Condvar::new())),
                panic: true,
            }),
            TranslationProviderConfig::default(),
        )
        .unwrap();
    let mut service = TranslationService::new(
        registry,
        ServiceLimits {
            workers: 1,
            ..ServiceLimits::default()
        },
    )
    .unwrap();
    let id = service.submit("shutdown-fixture", request()).unwrap();
    receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let result = service.poll(id).unwrap();
        if result.state.is_terminal() {
            assert_eq!(result.error, Some(TranslationError::ProviderUnavailable));
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    service.begin_shutdown();
    while !service.workers_finished() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(service.shutdown(), Err(TranslationError::InternalError));
}
