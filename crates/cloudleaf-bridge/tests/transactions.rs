use cloudleaf_bridge::{
    BridgeError, LazyMarian, PendingCommit, TranslationBridge, TranslationState,
};
use cloudleaf_input::{InputConfig, InputEngine, PinyinScheme, SchemeConfig, SearchSnapshot};
use cloudleaf_search::Config;
use cloudleaf_translation::{
    Language, OutputKind, ProviderCapabilities, ProviderIdentity, ProviderOutput, QualityStatus,
    TranslationCancellation, TranslationDirection, TranslationError, TranslationKind,
    TranslationProvider, TranslationRequest, TranslationResult, TranslationTaskId,
};
use cloudleaf_types::{CommitOutcome, InputEvent, InputMode, Key};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const FIXTURE: &str = include_str!("fixtures/lexicon.tsv");
const FIXTURE_SHA: &str = "50b020cbe8813bf783a1ac66731e93771439ddff9a6b9a369dde58332c704041";
const WAIT: Duration = Duration::from_secs(5);
const ZH_EN: TranslationDirection = TranslationDirection {
    source: Language::Chinese,
    target: Language::English,
};
const EN_ZH: TranslationDirection = TranslationDirection {
    source: Language::English,
    target: Language::Chinese,
};

struct Gate {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}

struct FixtureProvider {
    version: &'static str,
    network: bool,
    calls: AtomicUsize,
    gate: Option<Gate>,
}

impl FixtureProvider {
    fn new(version: &'static str) -> Self {
        Self {
            version,
            network: false,
            calls: AtomicUsize::new(0),
            gate: None,
        }
    }
}

impl TranslationProvider for FixtureProvider {
    fn identity(&self) -> ProviderIdentity {
        ProviderIdentity {
            id: "translation-fixture".into(),
            version: self.version.into(),
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            directions: vec![ZH_EN, EN_ZH],
            kinds: vec![TranslationKind::Sentence],
            auto_source: false,
            network: self.network,
            available: true,
        }
    }

    fn translate(
        &self,
        request: &TranslationRequest,
        cancellation: &TranslationCancellation,
    ) -> Result<ProviderOutput, TranslationError> {
        cancellation.check()?;
        assert!(request.send_authorized_to.is_none());
        assert!(!request.use_cache);
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            gate.entered.send(()).unwrap();
            let _ = gate.release.lock().unwrap().recv_timeout(WAIT);
        }
        // 放行后仍返回结果，验证任务层会抑制不合作提供者的迟到成功。
        let translated_text = match (request.direction, request.text.as_str()) {
            (ZH_EN, "你好") => "Hello!",
            (EN_ZH, "Hello!") => "你好！",
            _ => return Err(TranslationError::NoMatch),
        };
        Ok(ProviderOutput {
            translated_text: translated_text.into(),
            resolved_source: request.direction.source,
            output_kind: OutputKind::Translation,
            quality: QualityStatus::Generated,
            source_ids: vec!["fixture".into()],
        })
    }
}

fn input() -> InputEngine {
    let snapshot = SearchSnapshot::from_verified_tsv(FIXTURE, FIXTURE_SHA).unwrap();
    InputEngine::with_search(
        Arc::new(snapshot),
        Config::default(),
        InputConfig::default(),
    )
    .unwrap()
}

fn activated(provider: Arc<dyn TranslationProvider>, timeout: Duration) -> TranslationBridge {
    let mut bridge = TranslationBridge::new(input(), 1, provider, timeout).unwrap();
    bridge.apply_input(InputEvent::Activate).unwrap();
    bridge
}

fn standard() -> TranslationBridge {
    activated(Arc::new(FixtureProvider::new("v1")), WAIT)
}

fn held(timeout: Duration) -> (TranslationBridge, mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let mut provider = FixtureProvider::new("held");
    provider.gate = Some(Gate {
        entered,
        release: Mutex::new(release_rx),
    });
    (activated(Arc::new(provider), timeout), entered_rx, release)
}

fn candidate(bridge: &mut TranslationBridge) -> TranslationTaskId {
    for ch in "nihao".chars() {
        let update = bridge.apply_input(InputEvent::Key(Key::Char(ch))).unwrap();
        assert!(update.error.is_none());
    }
    let frame = bridge.input().frame();
    let candidate = frame.candidates.iter().find(|c| c.text == "你好").unwrap();
    bridge
        .request_candidate(frame.revision, &candidate.id)
        .unwrap()
}

fn english(bridge: &mut TranslationBridge) -> TranslationTaskId {
    bridge
        .apply_input(InputEvent::SetMode(InputMode::English))
        .unwrap();
    bridge.replace_english("Hello!").unwrap();
    bridge.request_english().unwrap()
}

fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !predicate() {
        assert!(Instant::now() < deadline, "后台状态超出预期等待时间");
        thread::sleep(Duration::from_millis(1));
    }
}

fn finished(bridge: &mut TranslationBridge) -> TranslationState {
    until(|| {
        !matches!(
            bridge.poll(),
            TranslationState::Requested | TranslationState::Running
        )
    });
    bridge.state()
}

fn ready(bridge: &mut TranslationBridge) -> TranslationResult {
    assert_eq!(finished(bridge), TranslationState::Ready);
    bridge.result().unwrap().clone()
}

fn confirm(
    bridge: &mut TranslationBridge,
    result: &TranslationResult,
) -> Result<PendingCommit, BridgeError> {
    bridge.confirm(result.context, result.task_id, result.result_version)
}

#[test]
fn real_candidate_is_translated_then_explicitly_applied_without_learning() {
    let mut bridge = standard();
    let task = candidate(&mut bridge);
    assert_eq!(bridge.state(), TranslationState::Requested);
    let source = bridge.source_identity().unwrap();
    assert_eq!(source.lexicon.as_deref(), Some(FIXTURE_SHA));
    assert!(source.candidate_id.is_some());
    assert_eq!(bridge.original(), Some("你好"));
    assert_eq!(bridge.original_preedit(), Some("nihao"));
    let result = ready(&mut bridge);
    assert_eq!(result.task_id, task);
    assert_eq!(result.direction, ZH_EN);
    assert_eq!(result.provider.version, "v1");
    assert_eq!(bridge.input().frame().preedit, "nihao");
    assert_eq!(
        bridge.confirm(result.context, result.task_id, result.result_version + 1),
        Err(BridgeError::StaleConfirmation)
    );
    let pending = confirm(&mut bridge, &result).unwrap();
    assert_eq!(pending.request.text, "Hello!");
    assert_eq!(pending.identity.task, task);
    assert_eq!(pending.identity.commit_id, pending.request.id);
    assert_eq!(bridge.state(), TranslationState::CommitPending);
    assert!(bridge.input().pending_selection().is_none());
    assert!(
        bridge
            .apply_input(InputEvent::CommitSucceeded {
                id: pending.request.id
            })
            .is_err()
    );

    let mut document = String::new();
    document.push_str(&pending.request.text);
    bridge
        .acknowledge(pending.identity, CommitOutcome::Applied)
        .unwrap();
    assert_eq!(document, "Hello!");
    assert_eq!(bridge.state(), TranslationState::Completed);
    assert!(bridge.input().frame().preedit.is_empty());
    assert!(bridge.input().pending_selection().is_none());
    assert_eq!(
        bridge.acknowledge(pending.identity, CommitOutcome::Applied),
        Err(BridgeError::StaleReceipt)
    );
    assert_eq!(
        confirm(&mut bridge, &result),
        Err(BridgeError::StaleConfirmation)
    );
    bridge.shutdown().unwrap();
}

#[test]
fn english_source_survives_until_the_confirmed_chinese_commit_is_applied() {
    let mut bridge = standard();
    english(&mut bridge);
    let result = ready(&mut bridge);
    assert_eq!(result.direction, EN_ZH);
    assert_eq!(bridge.english_text(), "Hello!");
    let pending = confirm(&mut bridge, &result).unwrap();
    assert_eq!(pending.request.text, "你好！");
    assert_eq!(bridge.english_text(), "Hello!");
    bridge
        .acknowledge(pending.identity, CommitOutcome::Applied)
        .unwrap();
    assert!(bridge.english_text().is_empty());
    assert!(bridge.input().pending_selection().is_none());
    bridge.shutdown().unwrap();
}

#[test]
fn editing_pinyin_during_real_worker_execution_makes_the_result_stale() {
    let (mut bridge, entered, release) = held(WAIT);
    candidate(&mut bridge);
    entered.recv_timeout(WAIT).unwrap();
    assert_eq!(bridge.poll(), TranslationState::Running);
    bridge.apply_input(InputEvent::Key(Key::Backspace)).unwrap();
    assert_eq!(bridge.input().frame().preedit, "niha");
    assert_eq!(bridge.state(), TranslationState::Stale);
    assert_eq!(bridge.original_preedit(), Some("nihao"));
    release.send(()).unwrap();
    assert_eq!(finished(&mut bridge), TranslationState::Stale);
    assert!(bridge.result().is_none());
    bridge.shutdown().unwrap();
}

#[test]
fn explicit_cancel_drops_the_running_result_and_preserves_source() {
    let (mut bridge, entered, release) = held(WAIT);
    english(&mut bridge);
    entered.recv_timeout(WAIT).unwrap();
    bridge.cancel().unwrap();
    assert_eq!(bridge.english_text(), "Hello!");
    release.send(()).unwrap();
    assert_eq!(finished(&mut bridge), TranslationState::Cancelled);
    assert!(bridge.result().is_none());
    assert!(bridge.input().failed_commit().is_none());
    bridge.shutdown().unwrap();
}

#[test]
fn focus_loss_rejects_even_a_ready_result() {
    let mut bridge = standard();
    candidate(&mut bridge);
    let result = ready(&mut bridge);
    bridge.apply_input(InputEvent::FocusLost).unwrap();
    assert_eq!(bridge.state(), TranslationState::Stale);
    assert_eq!(bridge.original(), Some("你好"));
    assert_eq!(bridge.original_preedit(), Some("nihao"));
    assert_eq!(
        confirm(&mut bridge, &result),
        Err(BridgeError::StaleConfirmation)
    );
    bridge.apply_input(InputEvent::Activate).unwrap();
    assert_eq!(
        confirm(&mut bridge, &result),
        Err(BridgeError::StaleConfirmation)
    );
    bridge.shutdown().unwrap();
}

#[test]
fn not_applied_preserves_translation_and_explicit_retry_gets_new_commit_id() {
    let mut bridge = standard();
    candidate(&mut bridge);
    let result = ready(&mut bridge);
    let first = confirm(&mut bridge, &result).unwrap();
    bridge
        .acknowledge(first.identity, CommitOutcome::NotApplied)
        .unwrap();
    assert_eq!(bridge.state(), TranslationState::RetryReady);
    assert_eq!(bridge.input().frame().preedit, "nihao");
    assert_eq!(bridge.result(), Some(&result));

    let retry = bridge.retry(first.identity).unwrap();
    assert_ne!(retry.identity.commit_id, first.identity.commit_id);
    assert_eq!(retry.identity.task, first.identity.task);
    assert_eq!(retry.request.text, first.request.text);
    assert_eq!(bridge.retry(first.identity), Err(BridgeError::InvalidState));
    assert_eq!(
        bridge.acknowledge(first.identity, CommitOutcome::Applied),
        Err(BridgeError::StaleReceipt)
    );
    let mut document = String::new();
    document.push_str(&retry.request.text);
    bridge
        .acknowledge(retry.identity, CommitOutcome::Applied)
        .unwrap();
    assert_eq!(document, "Hello!");
    assert_eq!(bridge.state(), TranslationState::Completed);
    assert!(bridge.input().pending_selection().is_none());
    bridge.shutdown().unwrap();
}

#[test]
fn unknown_commit_is_quarantined_until_an_explicitly_reconciled_new_session() {
    let mut bridge = standard();
    english(&mut bridge);
    let result = ready(&mut bridge);
    let pending = confirm(&mut bridge, &result).unwrap();
    bridge
        .acknowledge(pending.identity, CommitOutcome::Unknown)
        .unwrap();
    assert_eq!(bridge.state(), TranslationState::CommitUnknown);
    assert_eq!(bridge.english_text(), "Hello!");
    assert_eq!(bridge.result(), Some(&result));
    assert!(bridge.input().pending_selection().is_none());
    assert_eq!(
        bridge.retry(pending.identity),
        Err(BridgeError::OutcomeUnknown)
    );
    assert_eq!(
        bridge.apply_input(InputEvent::Key(Key::Escape)),
        Err(BridgeError::OutcomeUnknown)
    );
    assert_eq!(
        bridge.reset_after_unknown(pending.identity, 1),
        Err(BridgeError::StaleReceipt)
    );
    bridge.reset_after_unknown(pending.identity, 2).unwrap();
    assert_eq!(bridge.state(), TranslationState::Idle);
    bridge.apply_input(InputEvent::Activate).unwrap();
    english(&mut bridge);
    let current = ready(&mut bridge);
    assert_eq!(current.context.session, 2);
    let current = confirm(&mut bridge, &current).unwrap();
    assert_eq!(
        bridge.acknowledge(pending.identity, CommitOutcome::Applied),
        Err(BridgeError::StaleReceipt)
    );
    bridge
        .acknowledge(current.identity, CommitOutcome::Applied)
        .unwrap();
    bridge.shutdown().unwrap();
}

#[test]
fn focus_loss_with_an_outstanding_commit_is_unknown() {
    let mut bridge = standard();
    english(&mut bridge);
    let result = ready(&mut bridge);
    let pending = confirm(&mut bridge, &result).unwrap();
    assert_eq!(
        bridge.apply_input(InputEvent::FocusLost),
        Err(BridgeError::OutcomeUnknown)
    );
    assert_eq!(bridge.state(), TranslationState::CommitUnknown);
    assert_eq!(
        bridge.retry(pending.identity),
        Err(BridgeError::OutcomeUnknown)
    );
    bridge.shutdown().unwrap();
}

#[test]
fn unavailable_or_corrupt_catalog_does_not_block_normal_chinese_input() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (name, error) in [
        ("missing-catalog.json", TranslationError::ModelNotFound),
        ("lexicon.tsv", TranslationError::ModelLoadFailed),
    ] {
        let provider = LazyMarian::new(fixtures.join(name), "0".repeat(64)).unwrap();
        let mut bridge = activated(Arc::new(provider), WAIT);
        candidate(&mut bridge);
        assert_eq!(finished(&mut bridge), TranslationState::Failed);
        assert_eq!(bridge.failure(), Some(error));
        assert_eq!(bridge.input().frame().preedit, "nihao");
        let update = bridge.apply_input(InputEvent::Key(Key::Space)).unwrap();
        let normal = update.commit.unwrap();
        assert!(bridge.input().pending_selection().is_some());
        bridge
            .apply_input(InputEvent::CommitSucceeded { id: normal.id })
            .unwrap();
        assert!(bridge.input().frame().preedit.is_empty());
        bridge.shutdown().unwrap();
    }
}

#[test]
fn offline_only_controller_never_calls_network_provider() {
    let provider = Arc::new(FixtureProvider {
        network: true,
        ..FixtureProvider::new("online")
    });
    let bridge = TranslationBridge::new(input(), 1, provider.clone(), WAIT);
    assert!(matches!(
        bridge,
        Err(BridgeError::Translation(
            TranslationError::OnlineNotAuthorized
        ))
    ));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn user_layer_and_scheme_switches_stale_result_and_reject_old_candidate() {
    let mut bridge = standard();
    candidate(&mut bridge);
    let old = ready(&mut bridge);
    let identity = bridge.source_identity().unwrap().clone();
    let (base, _) = bridge.input().search_resources().unwrap();
    let users = concat!(
        "# cloudleaf-lexicon-v2\n",
        "id\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n",
        "user:1\tni hao\t你号\t\tunknown\t\tuser\tuser:1\tLicenseRef-User-Private\n"
    );
    let layer = Arc::new(base.with_user_layer(users, &BTreeMap::new()).unwrap());
    assert_ne!(layer.version(), FIXTURE_SHA);
    bridge
        .replace_search_resources(layer.clone(), None)
        .unwrap();
    assert_eq!(bridge.state(), TranslationState::Stale);
    assert_eq!(
        confirm(&mut bridge, &old),
        Err(BridgeError::StaleConfirmation)
    );
    assert_eq!(
        bridge.request_candidate(
            old.context.input_revision,
            identity.candidate_id.as_deref().unwrap()
        ),
        Err(BridgeError::StaleConfirmation)
    );

    let frame = bridge.input().frame();
    let new_id = &frame
        .candidates
        .iter()
        .find(|row| row.text == "你好")
        .unwrap()
        .id;
    bridge.request_candidate(frame.revision, new_id).unwrap();
    assert_eq!(
        bridge.source_identity().unwrap().lexicon.as_deref(),
        Some(layer.version())
    );
    let old_scheme = ready(&mut bridge);
    bridge
        .reconfigure_scheme(SchemeConfig {
            scheme: PinyinScheme::AbbreviatedPinyin,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(bridge.state(), TranslationState::Stale);
    assert_eq!(
        confirm(&mut bridge, &old_scheme),
        Err(BridgeError::StaleConfirmation)
    );
    bridge.shutdown().unwrap();
}

#[test]
fn model_replacement_waits_for_worker_stop_and_stales_previous_result() {
    let mut bridge = standard();
    english(&mut bridge);
    let old = ready(&mut bridge);
    bridge.begin_backend_change().unwrap();
    assert_eq!(bridge.state(), TranslationState::Stale);
    until(|| bridge.backend_stopped());
    bridge
        .finish_backend_change(Arc::new(FixtureProvider::new("v2")), WAIT)
        .unwrap();
    bridge.request_english().unwrap();
    let new = ready(&mut bridge);
    assert_ne!(new.context.epoch, old.context.epoch);
    assert_eq!(new.provider.version, "v2");
    assert_eq!(
        confirm(&mut bridge, &old),
        Err(BridgeError::StaleConfirmation)
    );
    bridge.shutdown().unwrap();
}

#[test]
fn model_replacement_refuses_to_start_another_worker_while_old_work_is_running() {
    let (mut bridge, entered, release) = held(WAIT);
    english(&mut bridge);
    entered.recv_timeout(WAIT).unwrap();
    bridge.begin_backend_change().unwrap();
    assert!(!bridge.backend_stopped());
    assert_eq!(
        bridge.finish_backend_change(Arc::new(FixtureProvider::new("v2")), WAIT),
        Err(BridgeError::InvalidState)
    );
    assert_eq!(bridge.english_text(), "Hello!");
    let worker_input = bridge.apply_input(InputEvent::SetMode(InputMode::Chinese));
    assert!(worker_input.unwrap().error.is_none());
    release.send(()).unwrap();
    until(|| bridge.backend_stopped());
    bridge
        .finish_backend_change(Arc::new(FixtureProvider::new("v2")), WAIT)
        .unwrap();
    english(&mut bridge);
    assert_eq!(ready(&mut bridge).provider.version, "v2");
    bridge.shutdown().unwrap();
}

#[test]
fn saturated_queue_returns_busy_while_the_input_engine_continues() {
    let (mut bridge, entered, release) = held(WAIT);
    english(&mut bridge);
    entered.recv_timeout(WAIT).unwrap();
    bridge.request_english().unwrap();
    bridge.request_english().unwrap();
    assert_eq!(
        bridge.request_english(),
        Err(BridgeError::Translation(TranslationError::Busy))
    );
    bridge
        .apply_input(InputEvent::SetMode(InputMode::Chinese))
        .unwrap();
    let update = bridge.apply_input(InputEvent::Key(Key::Char('n'))).unwrap();
    assert!(update.error.is_none());
    assert_eq!(update.frame.preedit, "n");
    release.send(()).unwrap();
    bridge.shutdown().unwrap();
}

#[test]
fn timeout_is_reported_as_failure_not_cancellation() {
    let (mut bridge, entered, release) = held(Duration::from_millis(200));
    english(&mut bridge);
    entered.recv_timeout(WAIT).unwrap();
    assert_eq!(finished(&mut bridge), TranslationState::Failed);
    assert_eq!(bridge.failure(), Some(TranslationError::Timeout));
    assert_eq!(bridge.english_text(), "Hello!");
    release.send(()).unwrap();
    bridge.shutdown().unwrap();
}
