use crate::provider::RegisteredProvider;
use crate::{
    Language, OutputKind, ProviderAuthorization, ProviderIdentity, ProviderOutput,
    ProviderRegistry, QualityStatus, RESULT_VERSION, ReadPolicy, TaskSnapshot, TaskState,
    TranslationCancellation, TranslationContext, TranslationDirection, TranslationError,
    TranslationKind, TranslationRequest, TranslationResult, TranslationTaskId,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceLimits {
    pub workers: usize,
    pub queued: usize,
    pub retained_tasks: usize,
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    pub cache_entries: usize,
    pub cache_bytes: usize,
    pub offline_only: bool,
}

impl Default for ServiceLimits {
    fn default() -> Self {
        Self {
            workers: 2,
            queued: 8,
            retained_tasks: 64,
            max_input_bytes: 16 * 1024,
            max_output_bytes: 64 * 1024,
            cache_entries: 64,
            cache_bytes: 256 * 1024,
            offline_only: true,
        }
    }
}

struct Job {
    request: TranslationRequest,
    provider: RegisteredProvider,
    cancellation: TranslationCancellation,
    events: mpsc::SyncSender<WorkerEvent>,
    max_output_bytes: usize,
}

enum WorkerEvent {
    Running,
    Finished(Result<ProviderOutput, TranslationError>),
}

struct TaskRecord {
    request: TranslationRequest,
    provider: RegisteredProvider,
    cancellation: TranslationCancellation,
    receiver: Option<mpsc::Receiver<WorkerEvent>>,
    snapshot: TaskSnapshot,
}

#[derive(Clone, PartialEq, Eq)]
struct CacheKey {
    text: String,
    direction: TranslationDirection,
    kind: TranslationKind,
    provider: ProviderIdentity,
    config_version: u64,
    read_policy: ReadPolicy,
}

struct Cached {
    key: CacheKey,
    output: ProviderOutput,
    bytes: usize,
}

pub struct TranslationService {
    registry: ProviderRegistry,
    limits: ServiceLimits,
    next_id: u64,
    sender: Option<mpsc::SyncSender<Job>>,
    workers: Vec<JoinHandle<()>>,
    tasks: BTreeMap<TranslationTaskId, TaskRecord>,
    cache: VecDeque<Cached>,
    cache_bytes: usize,
}

impl TranslationService {
    pub fn new(
        registry: ProviderRegistry,
        limits: ServiceLimits,
    ) -> Result<Self, TranslationError> {
        if !(1..=4).contains(&limits.workers)
            || !(1..=128).contains(&limits.queued)
            || !(1..=1024).contains(&limits.retained_tasks)
            || !(1..=1024 * 1024).contains(&limits.max_input_bytes)
            || !(1..=1024 * 1024).contains(&limits.max_output_bytes)
            || limits.cache_entries > 1024
            || limits.cache_bytes > 16 * 1024 * 1024
        {
            return Err(TranslationError::InvalidConfig);
        }
        let (sender, receiver) = mpsc::sync_channel::<Job>(limits.queued);
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::new();
        for index in 0..limits.workers {
            let receiver = Arc::clone(&receiver);
            let worker = thread::Builder::new()
                .name(format!("cloudleaf-translation-{index}"))
                .spawn(move || worker_loop(receiver))
                .map_err(|_| TranslationError::ProviderUnavailable)?;
            workers.push(worker);
        }
        Ok(Self {
            registry,
            limits,
            next_id: 1,
            sender: Some(sender),
            workers,
            tasks: BTreeMap::new(),
            cache: VecDeque::new(),
            cache_bytes: 0,
        })
    }

    pub fn submit(
        &mut self,
        provider_id: &str,
        request: TranslationRequest,
    ) -> Result<TranslationTaskId, TranslationError> {
        if self.sender.is_none() {
            return Err(TranslationError::ProviderUnavailable);
        }
        request.validate(self.limits.max_input_bytes)?;
        let provider = self.registry.get(provider_id)?;
        validate_admission(&provider, &request, self.limits.offline_only)?;
        if self.tasks.len() >= self.limits.retained_tasks || self.next_id == u64::MAX {
            return Err(TranslationError::Busy);
        }
        let id = TranslationTaskId(self.next_id);
        let cancellation = TranslationCancellation::new(provider.config.timeout);
        let key = cache_key(&request, &provider);
        let cached = if request.use_cache && provider.config.allow_cache {
            self.take_cached(&key)
        } else {
            None
        };
        let (receiver, snapshot) = if let Some(output) = cached {
            (
                None,
                TaskSnapshot {
                    state: TaskState::Succeeded,
                    result: Some(make_result(id, &request, &provider, output, true)),
                    error: None,
                },
            )
        } else {
            let (events, receiver) = mpsc::sync_channel(2);
            let job = Job {
                request: request.clone(),
                provider: provider.clone(),
                cancellation: cancellation.clone(),
                events,
                max_output_bytes: self.limits.max_output_bytes,
            };
            self.sender
                .as_ref()
                .ok_or(TranslationError::ProviderUnavailable)?
                .try_send(job)
                .map_err(|error| match error {
                    mpsc::TrySendError::Full(_) => TranslationError::Busy,
                    mpsc::TrySendError::Disconnected(_) => TranslationError::ProviderUnavailable,
                })?;
            (
                Some(receiver),
                TaskSnapshot {
                    state: TaskState::Queued,
                    result: None,
                    error: None,
                },
            )
        };
        self.next_id += 1;
        self.tasks.insert(
            id,
            TaskRecord {
                request,
                provider,
                cancellation,
                receiver,
                snapshot,
            },
        );
        Ok(id)
    }

    pub fn poll(&mut self, id: TranslationTaskId) -> Result<TaskSnapshot, TranslationError> {
        let record = self.tasks.get_mut(&id).ok_or(TranslationError::NotFound)?;
        if record.snapshot.state.is_terminal() {
            return Ok(record.snapshot.clone());
        }
        if let Err(error) = record.cancellation.check() {
            finish_error(record, error);
            return Ok(record.snapshot.clone());
        }
        let mut cached = None;
        if let Some(receiver) = &record.receiver {
            loop {
                match receiver.try_recv() {
                    Ok(WorkerEvent::Running) => record.snapshot.state = TaskState::Running,
                    Ok(WorkerEvent::Finished(outcome)) => {
                        // 完成与取消相遇时先核当前token，迟到结果不进入缓存或面板。
                        match record.cancellation.check().and(outcome) {
                            Ok(output) => {
                                if record.request.use_cache && record.provider.config.allow_cache {
                                    cached = Some((
                                        cache_key(&record.request, &record.provider),
                                        output.clone(),
                                    ));
                                }
                                record.snapshot = TaskSnapshot {
                                    state: TaskState::Succeeded,
                                    result: Some(make_result(
                                        id,
                                        &record.request,
                                        &record.provider,
                                        output,
                                        false,
                                    )),
                                    error: None,
                                };
                            }
                            Err(error) => finish_error(record, error),
                        }
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        finish_error(record, TranslationError::ProviderUnavailable);
                        break;
                    }
                }
            }
        }
        if record.snapshot.state.is_terminal() {
            record.receiver = None;
        }
        let snapshot = record.snapshot.clone();
        if let Some((key, output)) = cached {
            self.insert_cached(key, output);
        }
        Ok(snapshot)
    }

    pub fn cancel(&mut self, id: TranslationTaskId) -> Result<(), TranslationError> {
        let record = self.tasks.get_mut(&id).ok_or(TranslationError::NotFound)?;
        if !record.snapshot.state.is_terminal() {
            record.cancellation.cancel();
            finish_error(record, TranslationError::Cancelled);
            record.receiver = None;
        }
        Ok(())
    }

    pub fn expire(&mut self, id: TranslationTaskId) -> Result<(), TranslationError> {
        let record = self.tasks.get_mut(&id).ok_or(TranslationError::NotFound)?;
        if !record.snapshot.state.is_terminal() {
            record.cancellation.expire();
            finish_error(record, TranslationError::Expired);
            record.receiver = None;
        }
        Ok(())
    }

    pub fn forget(&mut self, id: TranslationTaskId) -> Result<(), TranslationError> {
        let record = self.tasks.remove(&id).ok_or(TranslationError::NotFound)?;
        record.cancellation.cancel();
        Ok(())
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.cache_bytes = 0;
    }

    /// 字节数为缓存文本payload，不是allocator或进程峰值内存。
    pub fn cache_usage(&self) -> (usize, usize) {
        (self.cache.len(), self.cache_bytes)
    }

    pub fn retained_tasks(&self) -> usize {
        self.tasks.len()
    }

    pub fn cancellation(
        &self,
        id: TranslationTaskId,
    ) -> Result<TranslationCancellation, TranslationError> {
        self.tasks
            .get(&id)
            .map(|record| record.cancellation.clone())
            .ok_or(TranslationError::NotFound)
    }

    /// 仅发取消和停止接单；窗口线程可以轮询实际worker退出再回收。
    pub fn begin_shutdown(&mut self) {
        for record in self.tasks.values() {
            record.cancellation.cancel();
        }
        self.sender = None;
        self.clear_cache();
    }

    pub fn workers_finished(&self) -> bool {
        self.workers.iter().all(JoinHandle::is_finished)
    }

    /// 仅后台或已证明worker退出后调用；不合作的提供者无法安全强杀。
    pub fn shutdown(mut self) -> Result<(), TranslationError> {
        self.begin_shutdown();
        let mut error = None;
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                error = Some(TranslationError::InternalError);
            }
        }
        error.map_or(Ok(()), Err)
    }

    fn take_cached(&mut self, key: &CacheKey) -> Option<ProviderOutput> {
        let index = self.cache.iter().position(|entry| entry.key == *key)?;
        let value = self.cache.remove(index)?;
        let output = value.output.clone();
        self.cache.push_back(value);
        Some(output)
    }

    fn insert_cached(&mut self, key: CacheKey, output: ProviderOutput) {
        let bytes = key.text.len()
            + key.provider.id.len()
            + key.provider.version.len()
            + output.translated_text.len()
            + output.source_ids.iter().map(String::len).sum::<usize>();
        if self.limits.cache_entries == 0 || bytes > self.limits.cache_bytes {
            return;
        }
        if let Some(index) = self.cache.iter().position(|entry| entry.key == key) {
            self.cache_bytes -= self.cache.remove(index).expect("索引来自同一缓存").bytes;
        }
        while self.cache.len() >= self.limits.cache_entries
            || self.cache_bytes + bytes > self.limits.cache_bytes
        {
            if let Some(old) = self.cache.pop_front() {
                self.cache_bytes -= old.bytes;
            } else {
                break;
            }
        }
        self.cache.push_back(Cached { key, output, bytes });
        self.cache_bytes += bytes;
    }
}

impl Drop for TranslationService {
    fn drop(&mut self) {
        for record in self.tasks.values() {
            record.cancellation.cancel();
        }
        self.sender = None;
        // Drop只请求取消；显式后台shutdown负责join，按键线程不等待provider。
    }
}

fn validate_admission(
    provider: &RegisteredProvider,
    request: &TranslationRequest,
    offline_only: bool,
) -> Result<(), TranslationError> {
    if !provider.config.enabled || !provider.capabilities.available {
        return Err(TranslationError::ProviderUnavailable);
    }
    if !provider.capabilities.kinds.contains(&request.kind) {
        return Err(TranslationError::UnsupportedTaskKind);
    }
    let supported = if request.direction.source == Language::Auto {
        provider.capabilities.auto_source
            && provider
                .capabilities
                .directions
                .iter()
                .any(|pair| pair.target == request.direction.target)
    } else {
        provider
            .capabilities
            .directions
            .contains(&request.direction)
    };
    if !supported {
        return Err(TranslationError::UnsupportedLanguagePair);
    }
    if provider.capabilities.network {
        let expected = ProviderAuthorization {
            provider: provider.identity.clone(),
            config_version: provider.config.config_version,
        };
        if offline_only
            || !provider.config.allow_network
            || request.send_authorized_to.as_ref() != Some(&expected)
        {
            return Err(TranslationError::OnlineNotAuthorized);
        }
    }
    Ok(())
}

fn worker_loop(receiver: Arc<Mutex<mpsc::Receiver<Job>>>) {
    loop {
        let job = match receiver.lock() {
            Ok(receiver) => receiver.recv(),
            Err(_) => return,
        };
        let Ok(job) = job else { return };
        let _ = job.events.try_send(WorkerEvent::Running);
        let outcome = job
            .cancellation
            .check()
            .and_then(|()| {
                job.provider
                    .provider
                    .translate(&job.request, &job.cancellation)
            })
            .and_then(|output| {
                validate_output(output, &job.request, &job.provider, job.max_output_bytes)
            });
        let _ = job.events.try_send(WorkerEvent::Finished(outcome));
    }
}

fn validate_output(
    output: ProviderOutput,
    request: &TranslationRequest,
    provider: &RegisteredProvider,
    max_bytes: usize,
) -> Result<ProviderOutput, TranslationError> {
    if output.translated_text.len() > max_bytes {
        return Err(TranslationError::ResponseTooLarge);
    }
    if output.translated_text.trim().is_empty()
        || output.translated_text.contains('\0')
        || output.source_ids.len() > 64
        || output
            .source_ids
            .iter()
            .any(|value| value.is_empty() || value.len() > 256)
        || (request.direction.source != Language::Auto
            && output.resolved_source != request.direction.source)
        || (output.resolved_source == Language::Auto && !provider.capabilities.network)
        || output.resolved_source == request.direction.target
        || (output.quality == QualityStatus::Generated
            && output.output_kind != OutputKind::Translation)
    {
        return Err(TranslationError::InvalidResponse);
    }
    if provider.config.read_policy == ReadPolicy::Production
        && matches!(
            output.quality,
            QualityStatus::Pending | QualityStatus::ResearchOnly
        )
    {
        return Err(TranslationError::DataNotApproved);
    }
    Ok(output)
}

fn finish_error(record: &mut TaskRecord, error: TranslationError) {
    let state = match error {
        TranslationError::Cancelled => TaskState::Cancelled,
        TranslationError::Expired | TranslationError::Timeout => TaskState::Expired,
        _ => TaskState::Failed,
    };
    record.snapshot = TaskSnapshot {
        state,
        result: None,
        error: Some(error),
    };
}

fn cache_key(request: &TranslationRequest, provider: &RegisteredProvider) -> CacheKey {
    CacheKey {
        text: request.text.clone(),
        direction: request.direction,
        kind: request.kind,
        provider: provider.identity.clone(),
        config_version: provider.config.config_version,
        read_policy: provider.config.read_policy,
    }
}

fn make_result(
    id: TranslationTaskId,
    request: &TranslationRequest,
    provider: &RegisteredProvider,
    output: ProviderOutput,
    from_cache: bool,
) -> TranslationResult {
    TranslationResult {
        task_id: id,
        context: TranslationContext { ..request.context },
        result_version: RESULT_VERSION,
        provider: provider.identity.clone(),
        direction: request.direction,
        kind: request.kind,
        output,
        from_cache,
    }
}
