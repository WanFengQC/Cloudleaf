use crate::{
    Language, OutputKind, ProviderCapabilities, ProviderIdentity, ProviderOutput, QualityStatus,
    TranslationCancellation, TranslationDirection, TranslationError, TranslationKind,
    TranslationProvider, TranslationRequest, TranslationResult,
};
use cloudleaf_translation_ipc::{
    Direction, IpcError, MAX_TIMEOUT_MS, PROVIDER_ID, ProgressStage, RESULT_VERSION, RequestKey,
    SourceIdentity, WorkerClient, WorkerConfig, WorkerEvent, WorkerMetrics, WorkerState,
};
use ring::digest::{SHA256, digest};
use std::{
    fs::File,
    io::Read,
    path::Path,
    sync::{Arc, Mutex, TryLockError},
    thread,
    time::{Duration, Instant},
};

const CATALOG_LIMIT: u64 = 64 * 1024;
const POLL: Duration = Duration::from_millis(5);
const CANCEL_WAIT: Duration = Duration::from_millis(250);
const SHUTDOWN_WAIT: Duration = Duration::from_secs(3);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStage {
    #[default]
    NotStarted,
    Starting,
    Verifying,
    Loading,
    Ready,
    Translating,
    Cancelling,
    Failed,
    Closing,
    Stopped,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WorkerSnapshot {
    pub stage: WorkerStage,
    pub child_pid: Option<u32>,
    pub request: Option<RequestKey>,
    pub task_id: Option<u64>,
    pub source_sha256: Option<[u8; 32]>,
    pub catalog_sha256: Option<String>,
    pub direction: Option<TranslationDirection>,
    pub output_sha256: Option<[u8; 32]>,
    pub model_identity: Option<String>,
    pub metrics: Option<WorkerMetrics>,
    pub error: Option<TranslationError>,
}

impl WorkerSnapshot {
    pub fn matches_result(&self, source: &str, result: &TranslationResult) -> bool {
        self.request.is_some_and(|key| {
            key.source.session == result.context.session
                && key.source.epoch == result.context.epoch
                && key.source.input_revision == result.context.input_revision
        }) && result.provider.id == PROVIDER_ID
            && self.catalog_sha256.as_deref() == Some(result.provider.version.as_str())
            && self.direction == Some(result.direction)
            && result.result_version == RESULT_VERSION
            && self.source_sha256 == Some(hash(source.as_bytes()))
            && self.output_sha256 == Some(hash(result.output.translated_text.as_bytes()))
            && self
                .model_identity
                .as_deref()
                .is_some_and(|model| result.output.source_ids.as_slice() == [model])
    }
}

/// 观察句柄不持有进程，也不会等待发译或回收。
#[derive(Clone)]
pub struct WorkerObserver(Arc<Mutex<WorkerSnapshot>>);

impl WorkerObserver {
    pub fn snapshot(&self) -> Option<WorkerSnapshot> {
        self.0.try_lock().ok().map(|snapshot| snapshot.clone())
    }
}

pub struct WorkerProvider {
    config: WorkerConfig,
    client: Mutex<WorkerClient>,
    observed: WorkerObserver,
}

impl WorkerProvider {
    pub fn new(config: WorkerConfig) -> Self {
        Self {
            client: Mutex::new(WorkerClient::new(config.clone())),
            config,
            observed: WorkerObserver(Arc::new(Mutex::new(WorkerSnapshot::default()))),
        }
    }

    pub fn observer(&self) -> WorkerObserver {
        self.observed.clone()
    }

    fn publish(&self, snapshot: &WorkerSnapshot) {
        if let Ok(mut observed) = self.observed.0.lock() {
            *observed = snapshot.clone();
        }
    }

    fn translate_with_client(
        &self,
        client: &mut WorkerClient,
        request: &TranslationRequest,
        cancellation: &TranslationCancellation,
        snapshot: &mut WorkerSnapshot,
    ) -> Result<ProviderOutput, TranslationError> {
        let direction = match (request.direction.source, request.direction.target) {
            (Language::Chinese, Language::English) => Direction::ZhEn,
            (Language::English, Language::Chinese) => Direction::EnZh,
            _ => return Err(TranslationError::UnsupportedLanguagePair),
        };
        cancellation.check()?;
        // 目录元数据身份有独立错误；模型清单和权重仍只由工作进程验证。
        if worker_catalog_sha256(self.config.catalog_path())? != self.config.catalog_sha256() {
            return Err(TranslationError::OfflineCatalogMismatch);
        }
        cancellation.check()?;
        let timeout = cancellation
            .remaining()
            .min(Duration::from_millis(MAX_TIMEOUT_MS));
        if timeout < Duration::from_millis(1) {
            return Err(TranslationError::Timeout);
        }

        let context = request.context;
        let key = client
            .submit(
                SourceIdentity {
                    session: context.session,
                    epoch: context.epoch,
                    input_revision: context.input_revision,
                },
                direction,
                request.text.clone(),
                timeout,
            )
            .map_err(worker_error)?;
        snapshot.request = Some(key);
        snapshot.stage = if client.state() == WorkerState::Starting {
            WorkerStage::Starting
        } else {
            WorkerStage::Ready
        };
        self.publish(snapshot);
        let mut stopped = None;
        loop {
            if stopped.is_none()
                && let Err(error) = cancellation.check()
            {
                client.cancel(key);
                stopped = Some((error, Instant::now() + CANCEL_WAIT));
                snapshot.stage = WorkerStage::Cancelling;
                self.publish(snapshot);
            }

            if let Some(event) = client.try_event() {
                snapshot.child_pid = client.child_pid();
                match event {
                    WorkerEvent::State(state) if stopped.is_none() => {
                        snapshot.stage = match state {
                            WorkerState::NotStarted => WorkerStage::NotStarted,
                            WorkerState::Starting => WorkerStage::Starting,
                            WorkerState::Ready => WorkerStage::Ready,
                            // 请求被发送与张量开始计算不同，由 Progress 报告计算阶段。
                            WorkerState::Busy => snapshot.stage,
                            WorkerState::Stopping => WorkerStage::Closing,
                            WorkerState::Stopped => WorkerStage::Stopped,
                            WorkerState::Failed => WorkerStage::Failed,
                        };
                        self.publish(snapshot);
                    }
                    WorkerEvent::Accepted {
                        key: active,
                        task_id,
                    } if active == key && stopped.is_none() => {
                        snapshot.task_id = Some(task_id);
                        self.publish(snapshot);
                    }
                    WorkerEvent::Progress {
                        key: active,
                        task_id,
                        stage,
                    } if active == key
                        && stopped.is_none()
                        && Some(task_id) == snapshot.task_id =>
                    {
                        snapshot.stage = match stage {
                            ProgressStage::Verifying => WorkerStage::Verifying,
                            ProgressStage::Loading => WorkerStage::Loading,
                            ProgressStage::Inferring => WorkerStage::Translating,
                        };
                        self.publish(snapshot);
                    }
                    WorkerEvent::Completed {
                        key: active,
                        task_id,
                        result_version,
                        output,
                    } if active == key => {
                        if let Some((error, _)) = stopped {
                            return Err(error);
                        }
                        cancellation.check()?;
                        if Some(task_id) != snapshot.task_id
                            || result_version != RESULT_VERSION
                            || output.provider_identity != PROVIDER_ID
                            || output.direction != direction
                            || output.catalog_sha256 != self.config.catalog_sha256()
                        {
                            return Err(TranslationError::OfflineWorker(IpcError::InvalidIdentity));
                        }
                        if output.metrics.output_truncated {
                            return Err(TranslationError::ResponseTooLarge);
                        }
                        snapshot.output_sha256 = Some(hash(output.raw_translation.as_bytes()));
                        snapshot.model_identity = Some(output.model_identity.clone());
                        snapshot.metrics = Some(*output.metrics);
                        return Ok(ProviderOutput {
                            translated_text: output.raw_translation,
                            resolved_source: request.direction.source,
                            output_kind: OutputKind::Translation,
                            quality: QualityStatus::Generated,
                            source_ids: vec![output.model_identity],
                        });
                    }
                    WorkerEvent::Failed { key: active, error } if active == key => {
                        return Err(stopped.map_or_else(|| worker_error(error), |(error, _)| error));
                    }
                    WorkerEvent::Cancelled { key: active } if active == key => {
                        return Err(stopped.map_or(TranslationError::Cancelled, |(error, _)| error));
                    }
                    _ => {}
                }
                continue;
            }
            if let Some((error, deadline)) = stopped
                && Instant::now() >= deadline
            {
                // 取消状态无法收割时隔离旧实例，只有实际退出后才能重新接单。
                if client.wait_for_shutdown(SHUTDOWN_WAIT) {
                    *client = WorkerClient::new(self.config.clone());
                    return Err(error);
                }
                return Err(TranslationError::OfflineWorker(IpcError::WorkerReapFailed));
            }
            thread::sleep(POLL);
        }
    }
}

impl TranslationProvider for WorkerProvider {
    fn identity(&self) -> ProviderIdentity {
        ProviderIdentity {
            id: PROVIDER_ID.to_owned(),
            version: self.config.catalog_sha256().to_owned(),
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
            auto_source: false,
            network: false,
            available: true,
        }
    }

    fn translate(
        &self,
        request: &TranslationRequest,
        cancellation: &TranslationCancellation,
    ) -> Result<ProviderOutput, TranslationError> {
        let mut client = self.client.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => TranslationError::Busy,
            TryLockError::Poisoned(_) => TranslationError::InternalError,
        })?;
        let mut snapshot = WorkerSnapshot {
            source_sha256: Some(hash(request.text.as_bytes())),
            catalog_sha256: Some(self.config.catalog_sha256().to_owned()),
            direction: Some(request.direction),
            child_pid: client.child_pid(),
            ..WorkerSnapshot::default()
        };
        let mut result =
            self.translate_with_client(&mut client, request, cancellation, &mut snapshot);
        if let Err(TranslationError::OfflineWorker(error)) = result
            && matches!(
                error,
                IpcError::WorkerExited
                    | IpcError::BrokenPipe
                    | IpcError::InvalidProtocolVersion
                    | IpcError::HandshakeTimeout
                    | IpcError::InvalidResponse
                    | IpcError::InvalidIdentity
                    | IpcError::InvalidMessage
                    | IpcError::FrameTooLarge
                    | IpcError::TruncatedFrame
                    | IpcError::InvalidUtf8
            )
        {
            // 运输终止后只清理旧代次。失败的请求原样返回，重启由下一次显式请求触发。
            snapshot.stage = WorkerStage::Closing;
            self.publish(&snapshot);
            if client.wait_for_shutdown(SHUTDOWN_WAIT) {
                *client = WorkerClient::new(self.config.clone());
            } else {
                result = Err(TranslationError::OfflineWorker(IpcError::WorkerReapFailed));
            }
        }
        snapshot.child_pid = client.child_pid();
        snapshot.error = result.as_ref().err().copied();
        snapshot.stage = match &result {
            Ok(_) => WorkerStage::Ready,
            Err(
                TranslationError::Cancelled | TranslationError::Expired | TranslationError::Timeout,
            ) if snapshot.child_pid.is_some() => WorkerStage::Ready,
            Err(_) => WorkerStage::Failed,
        };
        self.publish(&snapshot);
        result
    }

    fn shutdown(&self) -> Result<(), TranslationError> {
        let mut client = self
            .client
            .lock()
            .map_err(|_| TranslationError::InternalError)?;
        self.publish(&WorkerSnapshot {
            stage: WorkerStage::Closing,
            child_pid: client.child_pid(),
            ..WorkerSnapshot::default()
        });
        let reaped = client.wait_for_shutdown(SHUTDOWN_WAIT);
        let result = if reaped {
            Ok(())
        } else {
            Err(TranslationError::OfflineWorker(IpcError::WorkerReapFailed))
        };
        self.publish(&WorkerSnapshot {
            stage: if reaped {
                WorkerStage::Stopped
            } else {
                WorkerStage::Failed
            },
            child_pid: client.child_pid(),
            error: result.err(),
            ..WorkerSnapshot::default()
        });
        result
    }
}

/// 只读取有界 catalog，并仅用于本地身份发现。资源批准由工作进程独立执行。
pub fn worker_catalog_sha256(path: &Path) -> Result<String, TranslationError> {
    let file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            TranslationError::ModelNotFound
        } else {
            TranslationError::ModelLoadFailed
        }
    })?;
    let mut bytes = Vec::new();
    file.take(CATALOG_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| TranslationError::ModelLoadFailed)?;
    if bytes.len() as u64 > CATALOG_LIMIT {
        return Err(TranslationError::InvalidConfig);
    }
    Ok(hash(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    digest(&SHA256, bytes)
        .as_ref()
        .try_into()
        .expect("SHA-256 长度固定")
}

fn worker_error(error: IpcError) -> TranslationError {
    match error {
        IpcError::InvalidConfiguration => TranslationError::InvalidConfig,
        IpcError::InvalidInput => TranslationError::InvalidInput,
        IpcError::InputTooLong => TranslationError::InputTooLong,
        IpcError::Timeout => TranslationError::Timeout,
        IpcError::Busy => TranslationError::Busy,
        IpcError::Cancelled => TranslationError::Cancelled,
        IpcError::ModelNotFound => TranslationError::ModelNotFound,
        IpcError::ModelLoadFailed => TranslationError::ModelLoadFailed,
        IpcError::UnsupportedDirection => TranslationError::UnsupportedLanguagePair,
        IpcError::OutputTruncated => TranslationError::ResponseTooLarge,
        error => TranslationError::OfflineWorker(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_current_result_can_use_worker_measurements() {
        let direction = TranslationDirection {
            source: Language::English,
            target: Language::Chinese,
        };
        let context = crate::TranslationContext {
            session: 1,
            epoch: 2,
            input_revision: 3,
        };
        let snapshot = WorkerSnapshot {
            request: Some(RequestKey {
                generation: 1,
                source: SourceIdentity {
                    session: context.session,
                    epoch: context.epoch,
                    input_revision: context.input_revision,
                },
                request_id: 1,
            }),
            catalog_sha256: Some("1".repeat(64)),
            direction: Some(direction),
            source_sha256: Some(hash(b"Hello")),
            output_sha256: Some(hash("你好".as_bytes())),
            model_identity: Some("opus-mt-en-zh@fixture".to_owned()),
            ..WorkerSnapshot::default()
        };
        let result = TranslationResult {
            task_id: crate::TranslationTaskId(1),
            context,
            result_version: RESULT_VERSION,
            provider: ProviderIdentity {
                id: PROVIDER_ID.to_owned(),
                version: "1".repeat(64),
            },
            direction,
            kind: TranslationKind::Sentence,
            output: ProviderOutput {
                translated_text: "你好".to_owned(),
                resolved_source: Language::English,
                output_kind: OutputKind::Translation,
                quality: QualityStatus::Generated,
                source_ids: vec!["opus-mt-en-zh@fixture".to_owned()],
            },
            from_cache: false,
        };

        assert!(snapshot.matches_result("Hello", &result));
        assert!(!snapshot.matches_result("Hello!", &result));

        let mut other = result.clone();
        other.context.input_revision += 1;
        assert!(!snapshot.matches_result("Hello", &other));
        other = result.clone();
        other.provider.version = "2".repeat(64);
        assert!(!snapshot.matches_result("Hello", &other));
        other = result.clone();
        other.direction.source = Language::Chinese;
        assert!(!snapshot.matches_result("Hello", &other));
        other = result;
        other.output.translated_text.push('!');
        assert!(!snapshot.matches_result("Hello", &other));
    }
}
