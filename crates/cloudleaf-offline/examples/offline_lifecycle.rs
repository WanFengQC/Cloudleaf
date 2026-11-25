use cloudleaf_offline::{InferenceStats, MarianProvider};
use cloudleaf_translation::*;
use serde_json::{Value, json};
use std::error::Error;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Outcome<T> = Result<T, Box<dyn Error>>;

// 只观测真实Provider调用，不替换模型、取消或调度逻辑。
struct ObservedProvider {
    model: Arc<MarianProvider>,
    calls: AtomicUsize,
    active: AtomicUsize,
    maximum_active: AtomicUsize,
    records: Mutex<Vec<Value>>,
}

impl ObservedProvider {
    fn new(model: MarianProvider) -> Self {
        Self {
            model: Arc::new(model),
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            maximum_active: AtomicUsize::new(0),
            records: Mutex::new(Vec::new()),
        }
    }
}

impl TranslationProvider for ObservedProvider {
    fn identity(&self) -> ProviderIdentity {
        self.model.identity()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.model.capabilities()
    }

    fn translate(
        &self,
        request: &TranslationRequest,
        cancellation: &TranslationCancellation,
    ) -> Result<ProviderOutput, TranslationError> {
        let sequence = self.calls.fetch_add(1, Ordering::AcqRel) + 1;
        let active = self.active.fetch_add(1, Ordering::AcqRel) + 1;
        self.maximum_active.fetch_max(active, Ordering::AcqRel);
        let started = Instant::now();
        let observed = self.model.translate_observed(request, cancellation);
        let (error, stats): (Option<String>, Option<&InferenceStats>) = match &observed {
            Ok((_, stats)) => (None, Some(stats)),
            Err(error) => (Some(error.to_string()), None),
        };
        let record = json!({
            "call": sequence,
            "session": request.context.session,
            "epoch": request.context.epoch,
            "input_revision": request.context.input_revision,
            "input": request.text,
            "elapsed_ms": started.elapsed().as_millis(),
            "cancellation_flag_at_return": cancellation.is_cancelled(),
            "error": error,
            "stats": stats,
        });
        self.records.lock().expect("观测记录锁已损坏").push(record);
        self.active.fetch_sub(1, Ordering::AcqRel);
        observed.map(|(output, _)| output)
    }
}

fn main() {
    if let Err(error) = run() {
        emit(json!({"event":"failed","error":error.to_string()}));
        std::process::exit(1);
    }
}

fn run() -> Outcome<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let catalog = single_catalog_argument(&args)?;
    let started = Instant::now();
    let provider = Arc::new(ObservedProvider::new(MarianProvider::load(catalog)?));
    emit(
        json!({"event":"loaded","catalog_identity":provider.identity().version,
        "load_ms":started.elapsed().as_millis(),"network":provider.capabilities().network,
        "scope":"internal_engineering_samples_without_independent_gold"}),
    );

    repeated_inference(&provider)?;
    cancellation_and_queue(&provider)?;
    timeout_and_recovery(&provider)?;

    require(
        provider.active.load(Ordering::Acquire) == 0,
        "provider call remains active",
    )?;
    require(
        provider.maximum_active.load(Ordering::Acquire) == 1,
        "single inference budget violated",
    )?;
    let records = provider
        .records
        .lock()
        .map_err(|_| "observation lock poisoned")?;
    for record in records.iter() {
        emit(json!({"event":"provider_call","record":record}));
    }
    emit(
        json!({"event":"passed","provider_calls":provider.calls.load(Ordering::Acquire),
        "maximum_active_provider_calls":provider.maximum_active.load(Ordering::Acquire),
        "active_provider_calls_at_end":provider.active.load(Ordering::Acquire),
        "model_provider_instances":1,"network_provider_registered":false,
        "formal_language_metrics":{"top1":null,"top5":null,"mrr":null}}),
    );
    Ok(())
}

fn single_catalog_argument(args: &[String]) -> Outcome<&Path> {
    if args.len() != 1 {
        return Err("usage: offline_lifecycle CATALOG".into());
    }
    Ok(Path::new(&args[0]))
}

fn repeated_inference(provider: &Arc<ObservedProvider>) -> Outcome<()> {
    let mut service = make_service(provider, Duration::from_secs(30))?;
    let mut baseline = None;
    for index in 0..10 {
        let request = request("I am developing a Chinese input method.", 1, 1, index + 1);
        let started = Instant::now();
        let id = service.submit("offline-model", request.clone())?;
        let snapshot = await_terminal(&mut service, id)?;
        let text = require_success(&snapshot)?.to_owned();
        let same_as_first = baseline.as_ref().is_none_or(|first| first == &text);
        emit(json!({"event":"repeated_inference","index":index,
            "temperature":if index==0 {"first_full_inference_after_load"} else {"warm"},
            "input":request.text,"output":text,"elapsed_ms":started.elapsed().as_millis(),
            "same_as_first":same_as_first,"snapshot":snapshot_json(&snapshot)}));
        require(
            same_as_first,
            "identical uncached greedy request changed output",
        )?;
        if baseline.is_none() {
            baseline = Some(text);
        }
        service.forget(id)?;
    }
    close_service(service, "after_repeated_inference")
}

fn cancellation_and_queue(provider: &Arc<ObservedProvider>) -> Outcome<()> {
    let mut service = make_service(provider, Duration::from_secs(30))?;
    let old_request = request(
        &"I am developing a Chinese input method. ".repeat(12),
        2,
        1,
        1,
    );
    let old_id = service.submit("offline-model", old_request.clone())?;
    let mut panel = TranslationPanel::new(old_request.context);
    panel.start(old_id, &old_request, provider.identity())?;
    await_running_call(&mut service, old_id, provider)?;
    // 留给真实编码器计算时间，再用Busy探测确认同一模型锁仍被占用。
    std::thread::sleep(Duration::from_millis(100));
    let before_cancel = service.poll(old_id)?;
    require(
        before_cancel.state == TaskState::Running,
        "long request finished before cancellation window",
    )?;
    let probe = provider.model.translate_observed(
        &request("Hello.", 2, 1, 2),
        &TranslationCancellation::new(Duration::from_millis(1)),
    );
    emit(
        json!({"event":"before_running_cancellation","snapshot":snapshot_json(&before_cancel),
        "active_calls":provider.active.load(Ordering::Acquire),
        "lock_probe_error":probe.as_ref().err().map(|error|error.to_string())}),
    );
    require(
        matches!(probe, Err(TranslationError::Busy)),
        "model lock not occupied during cancellation probe",
    )?;

    let next_request = request("The result should preserve the original meaning.", 2, 2, 3);
    let next_id = service.submit("offline-model", next_request.clone())?;
    let mut busy = 0;
    let mut extra = Vec::new();
    for index in 0..8 {
        match service.submit("offline-model", request("Hello.", 2, 2, index + 10)) {
            Ok(id) => extra.push(id),
            Err(TranslationError::Busy) => busy += 1,
            Err(error) => return Err(error.into()),
        }
    }
    for id in &extra {
        service.cancel(*id)?;
    }
    emit(json!({"event":"queue_budget","workers":1,"queued_budget":1,
        "burst_attempts":8,"busy":busy,"accepted_extra":extra.len(),"real_model_lock_probe":"Busy"}));
    require(busy > 0, "burst did not observe bounded queue rejection")?;

    let cancelled_at = Instant::now();
    service.cancel(old_id)?;
    panel.cancel();
    let cancelled = service.poll(old_id)?;
    require(
        cancelled.state == TaskState::Cancelled && cancelled.result.is_none(),
        "cancelled task exposed a result",
    )?;
    panel.focus(next_request.context);
    panel.start(next_id, &next_request, provider.identity())?;
    let next_snapshot = await_terminal(&mut service, next_id)?;
    require_success(&next_snapshot)?;
    panel.update(next_id, next_request.context, next_snapshot.clone())?;
    let kept_output = panel
        .result()
        .ok_or("new panel result missing")?
        .output
        .translated_text
        .clone();
    let stale = panel.update(old_id, old_request.context, service.poll(old_id)?);
    require(
        stale == Err(TranslationError::StaleResult),
        "old cancelled context was accepted",
    )?;
    require(
        panel.original() == next_request.text,
        "old context changed new original",
    )?;
    require(
        panel
            .result()
            .is_some_and(|result| result.output.translated_text == kept_output),
        "old context changed new result",
    )?;
    emit(
        json!({"event":"cancel_then_recover","cancel_to_next_success_ms":cancelled_at.elapsed().as_millis(),
        "cancelled":snapshot_json(&cancelled),"next_input":next_request.text,
        "next":snapshot_json(&next_snapshot),"old_context_rejected":true,"original_preserved":true,
        "backend_note":"Busy proves provider lock ownership; no instruction-level encoder cancellation hook"}),
    );
    close_service(service, "after_cancel_and_queue")
}

fn timeout_and_recovery(provider: &Arc<ObservedProvider>) -> Outcome<()> {
    let mut service = make_service(provider, Duration::from_millis(1))?;
    let calls_before = provider.calls.load(Ordering::Acquire);
    let timed_request = request(
        &"The result should preserve the original meaning. ".repeat(12),
        3,
        1,
        1,
    );
    let id = service.submit("offline-model", timed_request.clone())?;
    let snapshot = await_terminal(&mut service, id)?;
    let provider_entered = provider.calls.load(Ordering::Acquire) > calls_before;
    emit(
        json!({"event":"one_millisecond_timeout","input":timed_request.text,"provider_entered":provider_entered,
        "snapshot":snapshot_json(&snapshot),"active_calls_at_logical_timeout":provider.active.load(Ordering::Acquire)}),
    );
    require(
        snapshot.state == TaskState::Expired && snapshot.result.is_none(),
        "one millisecond task did not expire",
    )?;
    require(
        snapshot.error == Some(TranslationError::Timeout),
        "unexpected timeout error category",
    )?;
    close_service(service, "after_one_millisecond_timeout")?;

    let mut recovered = make_service(provider, Duration::from_secs(30))?;
    let id = recovered.submit("offline-model", request("Hello.", 4, 1, 1))?;
    let snapshot = await_terminal(&mut recovered, id)?;
    require_success(&snapshot)?;
    emit(json!({"event":"post_timeout_recovery","snapshot":snapshot_json(&snapshot)}));
    close_service(recovered, "after_post_timeout_recovery")
}

fn make_service(
    provider: &Arc<ObservedProvider>,
    timeout: Duration,
) -> Outcome<TranslationService> {
    require(
        provider.active.load(Ordering::Acquire) == 0,
        "previous provider work not finished before service replacement",
    )?;
    let mut registry = ProviderRegistry::new();
    registry.register(
        provider.clone(),
        TranslationProviderConfig {
            timeout,
            allow_network: false,
            allow_cache: false,
            read_policy: ReadPolicy::Research,
            ..TranslationProviderConfig::default()
        },
    )?;
    Ok(TranslationService::new(
        registry,
        ServiceLimits {
            workers: 1,
            queued: 1,
            offline_only: true,
            cache_entries: 0,
            cache_bytes: 0,
            ..ServiceLimits::default()
        },
    )?)
}

fn request(text: &str, session: u64, epoch: u64, revision: u64) -> TranslationRequest {
    TranslationRequest {
        text: text.to_owned(),
        direction: TranslationDirection {
            source: Language::English,
            target: Language::Chinese,
        },
        kind: TranslationKind::Sentence,
        context: TranslationContext {
            session,
            epoch,
            input_revision: revision,
        },
        send_authorized_to: None,
        use_cache: false,
    }
}

fn await_terminal(
    service: &mut TranslationService,
    id: TranslationTaskId,
) -> Outcome<TaskSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let snapshot = service.poll(id)?;
        if snapshot.state.is_terminal() {
            return Ok(snapshot);
        }
        require(
            Instant::now() < deadline,
            "terminal snapshot deadline exceeded",
        )?;
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn await_running_call(
    service: &mut TranslationService,
    id: TranslationTaskId,
    provider: &ObservedProvider,
) -> Outcome<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let snapshot = service.poll(id)?;
        if snapshot.state == TaskState::Running && provider.active.load(Ordering::Acquire) == 1 {
            return Ok(());
        }
        require(
            !snapshot.state.is_terminal(),
            "request finished before running-call observation",
        )?;
        require(
            Instant::now() < deadline,
            "running-call observation deadline exceeded",
        )?;
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn require_success(snapshot: &TaskSnapshot) -> Outcome<&str> {
    require(
        snapshot.state == TaskState::Succeeded,
        "real inference did not succeed",
    )?;
    let result = snapshot
        .result
        .as_ref()
        .ok_or("successful task result missing")?;
    require(
        !result.from_cache
            && result.output.quality == QualityStatus::Generated
            && result.output.output_kind == OutputKind::Translation,
        "unexpected result origin or quality",
    )?;
    require(
        !result.output.translated_text.trim().is_empty(),
        "empty model output",
    )?;
    Ok(&result.output.translated_text)
}

fn close_service(mut service: TranslationService, phase: &str) -> Outcome<()> {
    let started = Instant::now();
    service.begin_shutdown();
    let signal_ms = started.elapsed().as_millis();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !service.workers_finished() {
        require(
            Instant::now() < deadline,
            "worker exit deadline exceeded; no replacement service started",
        )?;
        std::thread::sleep(Duration::from_millis(2));
    }
    let exit_ms = started.elapsed().as_millis();
    service.shutdown()?;
    emit(
        json!({"event":"service_closed","phase":phase,"begin_shutdown_ms":signal_ms,
        "workers_finished":true,"wait_exit_ms":exit_ms,"join":"success"}),
    );
    Ok(())
}

fn snapshot_json(snapshot: &TaskSnapshot) -> Value {
    json!({"state":format!("{:?}",snapshot.state),"error":snapshot.error.map(|error|error.to_string()),
        "output":snapshot.result.as_ref().map(|result|result.output.translated_text.as_str()),
        "from_cache":snapshot.result.as_ref().map(|result|result.from_cache)})
}

fn require(condition: bool, error: &'static str) -> Outcome<()> {
    if condition { Ok(()) } else { Err(error.into()) }
}

fn emit(record: Value) {
    println!("{record}");
}
