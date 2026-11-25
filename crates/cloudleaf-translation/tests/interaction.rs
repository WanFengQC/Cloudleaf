use cloudleaf_translation::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn request() -> TranslationRequest {
    TranslationRequest {
        text: "原创机制样例🙂\n原文保留".into(),
        direction: TranslationDirection {
            source: Language::Chinese,
            target: Language::English,
        },
        kind: TranslationKind::Sentence,
        context: TranslationContext {
            session: 1,
            epoch: 2,
            input_revision: 3,
        },
        send_authorized_to: None,
        use_cache: false,
    }
}

fn provider() -> ProviderIdentity {
    ProviderIdentity {
        id: "fixture-panel".into(),
        version: "mechanism-only".into(),
    }
}

fn result(id: u64, request: &TranslationRequest) -> TranslationResult {
    TranslationResult {
        task_id: TranslationTaskId(id),
        context: request.context,
        result_version: RESULT_VERSION,
        provider: provider(),
        direction: request.direction,
        kind: request.kind,
        output: ProviderOutput {
            translated_text: "fixture output 🙂\nnot language gold".into(),
            resolved_source: Language::Chinese,
            output_kind: OutputKind::Translation,
            quality: QualityStatus::Generated,
            source_ids: vec!["mechanism-fixture".into()],
        },
        from_cache: false,
    }
}

fn ready(panel: &mut TranslationPanel, value: TranslationResult) -> Result<(), TranslationError> {
    panel.update(
        value.task_id,
        value.context,
        TaskSnapshot {
            state: TaskState::Succeeded,
            result: Some(value),
            error: None,
        },
    )
}

#[test]
fn confirmation_is_explicit_current_and_single_use_without_an_automatic_commit() {
    let request = request();
    let mut panel = TranslationPanel::new(request.context);
    panel
        .start(TranslationTaskId(1), &request, provider())
        .unwrap();
    assert_eq!(panel.state(), TranslationPanelState::Waiting);
    assert_eq!(
        panel
            .confirm(TranslationTaskId(1), request.context, RESULT_VERSION)
            .unwrap_err(),
        TranslationError::StaleResult
    );
    ready(&mut panel, result(1, &request)).unwrap();
    assert_eq!(panel.state(), TranslationPanelState::Ready);
    assert_eq!(panel.original(), request.text);
    let copy = panel
        .copy(TranslationTaskId(1), request.context, RESULT_VERSION)
        .unwrap();
    assert!(copy.text.contains("fixture output"));
    assert_eq!(panel.state(), TranslationPanelState::Ready);
    assert_eq!(
        panel
            .confirm(TranslationTaskId(1), request.context, 99)
            .unwrap_err(),
        TranslationError::StaleResult
    );
    let intent = panel
        .confirm(TranslationTaskId(1), request.context, RESULT_VERSION)
        .unwrap();
    assert_eq!(intent.text, copy.text);
    assert_eq!(panel.state(), TranslationPanelState::Confirmed);
    assert_eq!(
        panel
            .confirm(TranslationTaskId(1), request.context, RESULT_VERSION)
            .unwrap_err(),
        TranslationError::StaleResult
    );
    assert_eq!(panel.original(), request.text);
    assert!(!format!("{panel:?} {intent:?}").contains("fixture output"));
}

#[test]
fn replacement_focus_and_every_result_identity_reject_stale_outputs() {
    let request = request();
    let mut panel = TranslationPanel::new(request.context);
    panel
        .start(TranslationTaskId(1), &request, provider())
        .unwrap();
    assert_eq!(
        panel
            .start(TranslationTaskId(2), &request, provider())
            .unwrap(),
        Some(TranslationTaskId(1))
    );
    assert_eq!(
        ready(&mut panel, result(1, &request)),
        Err(TranslationError::StaleResult)
    );
    for changed in 0..6 {
        let mut old = result(2, &request);
        match changed {
            0 => old.context.epoch += 1,
            1 => old.context.session += 1,
            2 => old.context.input_revision += 1,
            3 => old.provider.version.push_str("-old"),
            4 => old.direction.target = Language::Chinese,
            _ => old.kind = TranslationKind::Word,
        }
        assert_eq!(ready(&mut panel, old), Err(TranslationError::StaleResult));
        assert_eq!(panel.state(), TranslationPanelState::Waiting);
    }
    assert_eq!(panel.focus_lost(), Some(TranslationTaskId(2)));
    assert_eq!(
        ready(&mut panel, result(2, &request)),
        Err(TranslationError::StaleResult)
    );
    assert_eq!(panel.original(), request.text);
    let next = TranslationContext {
        epoch: 3,
        ..request.context
    };
    panel.focus(next);
    assert_eq!(
        panel.start(TranslationTaskId(3), &request, provider()),
        Err(TranslationError::StaleResult)
    );
}

#[test]
fn research_gloss_and_reverse_lookup_can_be_copied_but_never_confirmed_as_translation() {
    let request = request();
    for kind in [
        OutputKind::DictionaryGloss,
        OutputKind::ReverseLookup,
        OutputKind::Translation,
    ] {
        let mut panel = TranslationPanel::new(request.context);
        panel
            .start(TranslationTaskId(1), &request, provider())
            .unwrap();
        let mut value = result(1, &request);
        value.output.quality = QualityStatus::ResearchOnly;
        value.output.output_kind = kind;
        ready(&mut panel, value).unwrap();
        assert!(
            panel
                .copy(TranslationTaskId(1), request.context, RESULT_VERSION)
                .is_ok()
        );
        assert_eq!(
            panel
                .confirm(TranslationTaskId(1), request.context, RESULT_VERSION)
                .unwrap_err(),
            TranslationError::DataNotApproved
        );
        assert_eq!(panel.cancel(), Some(TranslationTaskId(1)));
        assert_eq!(panel.original(), request.text);
    }
}

#[test]
fn actual_missing_secret_provider_failure_and_panel_cancellation_preserve_input_engine() {
    use cloudleaf_dictionary::Dictionary;
    use cloudleaf_input::InputEngine;
    use cloudleaf_types::{InputEvent, Key};
    let mut input = InputEngine::new(Dictionary::empty());
    input.handle(InputEvent::Activate);
    for c in "nihao".chars() {
        input.handle(InputEvent::Key(Key::Char(c)));
    }
    let before = input.frame();
    let backend = OpenAiCompatibleProvider::new(
        OpenAiCompatibleConfig {
            endpoint: "http://127.0.0.1:9/v1/chat/completions".into(),
            model: "fixture-no-network".into(),
            credential_reference: "missing".into(),
            enabled: true,
            allow_loopback_http: true,
            ..OpenAiCompatibleConfig::default()
        },
        Arc::new(SessionSecretStore::new()),
    )
    .unwrap();
    let mut request = request();
    request.send_authorized_to = Some(backend.authorization());
    let identity = backend.identity();
    let mut registry = ProviderRegistry::new();
    registry
        .register(
            Arc::new(backend),
            TranslationProviderConfig {
                allow_network: true,
                ..TranslationProviderConfig::default()
            },
        )
        .unwrap();
    let mut service = TranslationService::new(
        registry,
        ServiceLimits {
            offline_only: false,
            ..ServiceLimits::default()
        },
    )
    .unwrap();
    let id = service.submit(&identity.id, request.clone()).unwrap();
    let mut panel = TranslationPanel::new(request.context);
    panel.start(id, &request, identity).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let failed = loop {
        let snapshot = service.poll(id).unwrap();
        if snapshot.state.is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };
    assert_eq!(failed.error, Some(TranslationError::MissingSecret));
    panel.update(id, request.context, failed).unwrap();
    assert_eq!(panel.state(), TranslationPanelState::Failed);
    panel.cancel();
    assert_eq!(panel.original(), request.text);
    assert_eq!(input.frame(), before);
    service.shutdown().unwrap();
}

#[test]
fn failure_from_an_old_service_cannot_replace_a_new_epoch_with_the_same_task_id() {
    let old = request();
    let mut panel = TranslationPanel::new(old.context);
    panel.start(TranslationTaskId(1), &old, provider()).unwrap();
    let mut next = old.clone();
    next.context.epoch += 1;
    panel.focus(next.context);
    panel
        .start(TranslationTaskId(1), &next, provider())
        .unwrap();
    let late = TaskSnapshot {
        state: TaskState::Failed,
        result: None,
        error: Some(TranslationError::ProviderUnavailable),
    };
    assert_eq!(
        panel.update(TranslationTaskId(1), old.context, late),
        Err(TranslationError::StaleResult)
    );
    assert_eq!(panel.state(), TranslationPanelState::Waiting);
    ready(&mut panel, result(1, &next)).unwrap();
}
