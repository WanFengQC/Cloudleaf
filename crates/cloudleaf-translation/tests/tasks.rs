use cloudleaf_translation::{
    Language, OfflineModelDescriptor, OfflineModelProvider, OutputKind, ProviderAuthorization,
    ProviderCapabilities, ProviderIdentity, ProviderOutput, ProviderRegistry, QualityStatus,
    ReadPolicy, ServiceLimits, TaskSnapshot, TaskState, TranslationCancellation,
    TranslationContext, TranslationDirection, TranslationError, TranslationKind,
    TranslationProvider, TranslationProviderConfig, TranslationRequest, TranslationService,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[test]
fn request_and_cancellation_are_explicit_without_an_input_engine_dependency() {
    let request = TranslationRequest {
        text: "你好🙂\nworld".into(),
        direction: TranslationDirection {
            source: Language::Chinese,
            target: Language::English,
        },
        kind: TranslationKind::Phrase,
        context: TranslationContext {
            session: 1,
            epoch: 2,
            input_revision: 3,
        },
        send_authorized_to: None,
        use_cache: false,
    };
    assert!(request.validate(1024).is_ok());
    let token = TranslationCancellation::new(Duration::from_secs(1));
    token.cancel();
    assert_eq!(token.check(), Err(TranslationError::Cancelled));
}

fn request(text: &str) -> TranslationRequest {
    TranslationRequest {
        text: text.into(),
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

#[derive(Default)]
struct Gate {
    state: Mutex<(usize, bool)>,
    changed: Condvar,
}

impl Gate {
    fn enter(&self) {
        let mut state = self.state.lock().unwrap();
        state.0 += 1;
        self.changed.notify_all();
        while !state.1 {
            state = self.changed.wait(state).unwrap();
        }
    }

    fn started(&self) {
        self.started_count(1);
    }

    fn started_count(&self, count: usize) {
        let state = self.state.lock().unwrap();
        let (state, _) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(3), |value| value.0 < count)
            .unwrap();
        assert!(state.0 >= count, "受控provider未同时启动");
    }

    fn release(&self) {
        self.state.lock().unwrap().1 = true;
        self.changed.notify_all();
    }
}

struct ReleaseOnDrop(Arc<Gate>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}

struct Provider {
    id: &'static str,
    calls: Arc<AtomicUsize>,
    gate: Option<Arc<Gate>>,
    network: bool,
    quality: QualityStatus,
}

impl TranslationProvider for Provider {
    fn identity(&self) -> ProviderIdentity {
        ProviderIdentity {
            id: self.id.into(),
            version: "fixture-v1".into(),
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            directions: vec![
                TranslationDirection {
                    source: Language::Chinese,
                    target: Language::English,
                },
                TranslationDirection {
                    source: Language::English,
                    target: Language::Chinese,
                },
            ],
            kinds: vec![
                TranslationKind::Word,
                TranslationKind::Phrase,
                TranslationKind::Sentence,
            ],
            auto_source: true,
            network: self.network,
            available: true,
        }
    }

    fn translate(
        &self,
        request: &TranslationRequest,
        _: &TranslationCancellation,
    ) -> Result<ProviderOutput, TranslationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            // 该fixture故意迟到，用真实线程验证服务不会重新接纳取消结果。
            gate.enter();
        }
        Ok(ProviderOutput {
            translated_text: format!("{}译", request.text),
            resolved_source: if request.direction.source == Language::Auto {
                match request.direction.target {
                    Language::Chinese => Language::English,
                    _ => Language::Chinese,
                }
            } else {
                request.direction.source
            },
            output_kind: OutputKind::Translation,
            quality: self.quality,
            source_ids: vec!["fixture:not-language-quality".into()],
        })
    }
}

fn fixture_provider(id: &'static str) -> (Provider, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    (
        Provider {
            id,
            calls: Arc::clone(&calls),
            gate: None,
            network: false,
            quality: QualityStatus::Approved,
        },
        calls,
    )
}

fn registry(provider: Provider, config: TranslationProviderConfig) -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(provider), config).unwrap();
    registry
}

fn completed(
    service: &mut TranslationService,
    id: cloudleaf_translation::TranslationTaskId,
) -> TaskSnapshot {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let value = service.poll(id).unwrap();
        if value.state.is_terminal() {
            return value;
        }
        assert!(Instant::now() < deadline, "受控任务未结束");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn inputs_directions_and_debug_preserve_unicode_without_disclosing_text() {
    let value = request("秘密🙂\nmail\t42");
    assert!(value.validate(1024).is_ok());
    assert!(!format!("{value:?}").contains("秘密"));
    for text in ["", " \n\t", "a\0b"] {
        assert_eq!(
            request(text).validate(1024),
            Err(TranslationError::InvalidInput)
        );
    }
    assert_eq!(
        request("🙂").validate(3),
        Err(TranslationError::InputTooLong)
    );
    let mut bad = request("x");
    bad.direction.target = Language::Auto;
    assert_eq!(
        bad.validate(1024),
        Err(TranslationError::UnsupportedLanguagePair)
    );
    bad.direction = TranslationDirection {
        source: Language::English,
        target: Language::English,
    };
    assert_eq!(
        bad.validate(1024),
        Err(TranslationError::UnsupportedLanguagePair)
    );
}

#[test]
fn bidirectional_auto_and_missing_provider_are_explicit() {
    let (provider, _) = fixture_provider("local");
    let mut service = TranslationService::new(
        registry(provider, TranslationProviderConfig::default()),
        ServiceLimits::default(),
    )
    .unwrap();
    for direction in [
        TranslationDirection {
            source: Language::Chinese,
            target: Language::English,
        },
        TranslationDirection {
            source: Language::English,
            target: Language::Chinese,
        },
        TranslationDirection {
            source: Language::Auto,
            target: Language::Chinese,
        },
    ] {
        let mut value = request("词🙂");
        value.direction = direction;
        let id = service.submit("local", value).unwrap();
        let result = completed(&mut service, id).result.unwrap();
        assert_ne!(result.output.resolved_source, Language::Auto);
        assert!(!format!("{result:?}").contains("词🙂"));
        service.forget(id).unwrap();
    }
    assert_eq!(
        service.submit("missing", request("x")),
        Err(TranslationError::ProviderUnavailable)
    );
    service.shutdown().unwrap();
}

#[test]
fn queue_and_retained_table_reject_without_waiting_for_worker() {
    let gate = Arc::new(Gate::default());
    let _release = ReleaseOnDrop(Arc::clone(&gate));
    let (mut provider, _) = fixture_provider("blocked");
    provider.gate = Some(Arc::clone(&gate));
    let limits = ServiceLimits {
        workers: 1,
        queued: 1,
        retained_tasks: 2,
        ..ServiceLimits::default()
    };
    let mut service = TranslationService::new(
        registry(provider, TranslationProviderConfig::default()),
        limits,
    )
    .unwrap();
    let first = service.submit("blocked", request("一")).unwrap();
    gate.started();
    let second = service.submit("blocked", request("二")).unwrap();
    assert_eq!(
        service.submit("blocked", request("三")),
        Err(TranslationError::Busy)
    );
    service.forget(second).unwrap();
    // 表已腾空但真实有界队列还满，不能借forget制造无限排队。
    assert_eq!(
        service.submit("blocked", request("三")),
        Err(TranslationError::Busy)
    );
    gate.release();
    assert_eq!(completed(&mut service, first).state, TaskState::Succeeded);
    service.shutdown().unwrap();
}

#[test]
fn cancelled_expired_and_forgotten_results_never_return_late() {
    let gate = Arc::new(Gate::default());
    let _release = ReleaseOnDrop(Arc::clone(&gate));
    let (mut provider, _) = fixture_provider("blocked");
    provider.gate = Some(Arc::clone(&gate));
    let mut service = TranslationService::new(
        registry(provider, TranslationProviderConfig::default()),
        ServiceLimits {
            workers: 1,
            ..ServiceLimits::default()
        },
    )
    .unwrap();
    let first = service.submit("blocked", request("旧原文")).unwrap();
    gate.started();
    service.cancel(first).unwrap();
    assert_eq!(service.poll(first).unwrap().state, TaskState::Cancelled);
    let expired = service.submit("blocked", request("过期")).unwrap();
    service.expire(expired).unwrap();
    assert_eq!(
        service.poll(expired).unwrap().error,
        Some(TranslationError::Expired)
    );
    let forgotten = service.submit("blocked", request("已关闭")).unwrap();
    service.forget(forgotten).unwrap();
    gate.release();
    assert_eq!(
        service.poll(first).unwrap().error,
        Some(TranslationError::Cancelled)
    );
    assert_eq!(service.poll(forgotten), Err(TranslationError::NotFound));
    assert_eq!(service.cache_usage(), (0, 0));
    service.shutdown().unwrap();
}

#[test]
fn timeout_includes_queue_time_and_does_not_accept_provider_completion() {
    let gate = Arc::new(Gate::default());
    let _release = ReleaseOnDrop(Arc::clone(&gate));
    let (mut provider, _) = fixture_provider("blocked");
    provider.gate = Some(Arc::clone(&gate));
    let config = TranslationProviderConfig {
        timeout: Duration::from_secs(1),
        ..TranslationProviderConfig::default()
    };
    let mut service = TranslationService::new(
        registry(provider, config),
        ServiceLimits {
            workers: 1,
            ..ServiceLimits::default()
        },
    )
    .unwrap();
    let first = service.submit("blocked", request("在运行")).unwrap();
    gate.started();
    let queued = service.submit("blocked", request("在排队")).unwrap();
    std::thread::sleep(
        service.cancellation(queued).unwrap().remaining() + Duration::from_millis(5),
    );
    assert_eq!(
        service.poll(first).unwrap().error,
        Some(TranslationError::Timeout)
    );
    assert_eq!(
        service.poll(queued).unwrap().error,
        Some(TranslationError::Timeout)
    );
    gate.release();
    assert_eq!(service.poll(first).unwrap().state, TaskState::Expired);
    service.shutdown().unwrap();
}

#[test]
fn concurrent_contexts_and_cache_hits_keep_new_task_identity() {
    let (provider, calls) = fixture_provider("local");
    let config = TranslationProviderConfig {
        allow_cache: true,
        ..TranslationProviderConfig::default()
    };
    let mut service = TranslationService::new(
        registry(provider, config),
        ServiceLimits {
            cache_entries: 1,
            cache_bytes: 1024,
            ..ServiceLimits::default()
        },
    )
    .unwrap();
    let mut old = request("你好");
    old.use_cache = true;
    let first = service.submit("local", old.clone()).unwrap();
    let first_result = completed(&mut service, first).result.unwrap();
    assert!(!first_result.from_cache);
    old.context = TranslationContext {
        session: 2,
        epoch: 4,
        input_revision: 99,
    };
    let next = service.submit("local", old.clone()).unwrap();
    let cached = service.poll(next).unwrap().result.unwrap();
    assert!(cached.from_cache);
    assert_eq!(cached.context, old.context);
    assert_eq!(cached.task_id, next);
    assert_ne!(cached.task_id, first_result.task_id);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let mut other = request("再见");
    other.use_cache = true;
    let third = service.submit("local", other).unwrap();
    completed(&mut service, third);
    assert_eq!(service.cache_usage().0, 1);
    let fourth = service.submit("local", old).unwrap();
    assert!(!completed(&mut service, fourth).result.unwrap().from_cache);
    service.clear_cache();
    assert_eq!(service.cache_usage(), (0, 0));
    service.shutdown().unwrap();
}

#[test]
fn two_workers_execute_independent_contexts_before_either_finishes() {
    let gate = Arc::new(Gate::default());
    let _release = ReleaseOnDrop(Arc::clone(&gate));
    let (mut provider, calls) = fixture_provider("parallel");
    provider.gate = Some(Arc::clone(&gate));
    let mut service = TranslationService::new(
        registry(provider, TranslationProviderConfig::default()),
        ServiceLimits::default(),
    )
    .unwrap();
    let first = service.submit("parallel", request("第一句")).unwrap();
    let mut second_request = request("第二句");
    second_request.context = TranslationContext {
        session: 88,
        epoch: 22,
        input_revision: 33,
    };
    let expected_context = second_request.context;
    let second = service.submit("parallel", second_request).unwrap();
    gate.started_count(2);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(service.poll(first).unwrap().state, TaskState::Running);
    assert_eq!(service.poll(second).unwrap().state, TaskState::Running);
    gate.release();
    let first = completed(&mut service, first).result.unwrap();
    let second = completed(&mut service, second).result.unwrap();
    assert_eq!(first.output.translated_text, "第一句译");
    assert_eq!(second.output.translated_text, "第二句译");
    assert_eq!(second.context, expected_context);
    assert_ne!(first.context, second.context);
    assert_ne!(first.task_id, second.task_id);
    service.shutdown().unwrap();
}

#[test]
fn cache_byte_limit_evicts_payload_before_entry_limit_is_reached() {
    let (provider, calls) = fixture_provider("local");
    let config = TranslationProviderConfig {
        allow_cache: true,
        ..TranslationProviderConfig::default()
    };
    let mut service = TranslationService::new(
        registry(provider, config),
        ServiceLimits {
            cache_entries: 8,
            cache_bytes: 128,
            ..ServiceLimits::default()
        },
    )
    .unwrap();
    let mut first_request = request("aaaaaaaaaaaaaaaaaaaaaaaaa");
    first_request.use_cache = true;
    let first = service.submit("local", first_request.clone()).unwrap();
    completed(&mut service, first);
    let mut second_request = request("bbbbbbbbbbbbbbbbbbbbbbbbb");
    second_request.use_cache = true;
    let second = service.submit("local", second_request).unwrap();
    completed(&mut service, second);
    assert_eq!(service.cache_usage().0, 1);
    assert!(service.cache_usage().1 <= 128);
    let repeated = service.submit("local", first_request).unwrap();
    assert!(!completed(&mut service, repeated).result.unwrap().from_cache);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    service.shutdown().unwrap();
}

#[test]
fn cache_bytes_and_result_quality_are_real_boundaries() {
    let (provider, _) = fixture_provider("local");
    let config = TranslationProviderConfig {
        allow_cache: true,
        ..TranslationProviderConfig::default()
    };
    let mut service = TranslationService::new(
        registry(provider, config),
        ServiceLimits {
            cache_bytes: 1,
            ..ServiceLimits::default()
        },
    )
    .unwrap();
    let mut value = request("词");
    value.use_cache = true;
    let id = service.submit("local", value).unwrap();
    completed(&mut service, id);
    assert_eq!(service.cache_usage(), (0, 0));
    service.shutdown().unwrap();

    let (mut provider, _) = fixture_provider("research");
    provider.quality = QualityStatus::ResearchOnly;
    let mut service = TranslationService::new(
        registry(provider, TranslationProviderConfig::default()),
        ServiceLimits::default(),
    )
    .unwrap();
    let id = service.submit("research", request("词")).unwrap();
    assert_eq!(
        completed(&mut service, id).error,
        Some(TranslationError::DataNotApproved)
    );
    service.shutdown().unwrap();
}

#[test]
fn online_needs_offline_policy_config_and_exact_per_request_consent() {
    let (mut provider, calls) = fixture_provider("online");
    provider.network = true;
    provider.quality = QualityStatus::Generated;
    let identity = provider.identity();
    let config = TranslationProviderConfig {
        allow_network: true,
        config_version: 4,
        ..TranslationProviderConfig::default()
    };
    let mut service =
        TranslationService::new(registry(provider, config.clone()), ServiceLimits::default())
            .unwrap();
    let mut value = request("不能偷发");
    value.send_authorized_to = Some(ProviderAuthorization {
        provider: identity.clone(),
        config_version: 4,
    });
    assert_eq!(
        service.submit("online", value.clone()),
        Err(TranslationError::OnlineNotAuthorized)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    service.shutdown().unwrap();

    let (mut provider, _) = fixture_provider("online");
    provider.network = true;
    provider.quality = QualityStatus::Generated;
    let mut service = TranslationService::new(
        registry(provider, config),
        ServiceLimits {
            offline_only: false,
            ..ServiceLimits::default()
        },
    )
    .unwrap();
    value.send_authorized_to = None;
    assert_eq!(
        service.submit("online", value.clone()),
        Err(TranslationError::OnlineNotAuthorized)
    );
    value.send_authorized_to = Some(ProviderAuthorization {
        provider: identity.clone(),
        config_version: 3,
    });
    assert_eq!(
        service.submit("online", value.clone()),
        Err(TranslationError::OnlineNotAuthorized)
    );
    value.send_authorized_to = Some(ProviderAuthorization {
        provider: identity,
        config_version: 4,
    });
    let id = service.submit("online", value).unwrap();
    assert_eq!(
        completed(&mut service, id).result.unwrap().output.quality,
        QualityStatus::Generated
    );
    service.shutdown().unwrap();
}

#[test]
fn registry_config_model_and_production_policy_are_not_inferred() {
    let (provider, _) = fixture_provider("local");
    let mut registry = ProviderRegistry::new();
    let bad = TranslationProviderConfig {
        timeout: Duration::ZERO,
        ..TranslationProviderConfig::default()
    };
    assert_eq!(
        registry.register(Arc::new(provider), bad),
        Err(TranslationError::InvalidConfig)
    );
    assert!(registry.capabilities("missing").is_none());
    let model = OfflineModelProvider::unavailable(OfflineModelDescriptor {
        identity: ProviderIdentity {
            id: "offline".into(),
            version: "unloaded-v1".into(),
        },
        model_path: "not-a-shipped-model".into(),
        model_version: "unavailable".into(),
        license_id: None,
    })
    .unwrap();
    assert!(!model.capabilities().available);
    assert_eq!(
        model.translate(
            &request("整句"),
            &TranslationCancellation::new(Duration::from_secs(1))
        ),
        Err(TranslationError::ModelNotFound)
    );
    let config = TranslationProviderConfig {
        read_policy: ReadPolicy::Research,
        ..TranslationProviderConfig::default()
    };
    assert_eq!(config.read_policy, ReadPolicy::Research);
}
