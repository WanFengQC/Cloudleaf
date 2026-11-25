use cloudleaf_offline::{MarianProvider, ModelState};
use cloudleaf_translation::{
    Language, ProviderCapabilities, ProviderIdentity, ProviderOutput, TranslationCancellation,
    TranslationDirection, TranslationError, TranslationKind, TranslationProvider,
    TranslationRequest,
};
use cloudleaf_translation_ipc::{Direction, IpcError, PROVIDER_ID, ProgressStage, WorkerMetrics};
use std::path::PathBuf;
use std::sync::{Mutex, mpsc::SyncSender};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(crate) struct Observation {
    pub(crate) started: Instant,
    pub(crate) model_identity: String,
    pub(crate) metrics: WorkerMetrics,
}

pub(crate) enum Notice {
    Progress {
        request_id: u64,
        stage: ProgressStage,
    },
    Observed {
        request_id: u64,
        result: Result<Observation, IpcError>,
    },
}

impl Notice {
    pub(crate) fn request_id(&self) -> u64 {
        match self {
            Self::Progress { request_id, .. } | Self::Observed { request_id, .. } => *request_id,
        }
    }
}

pub(crate) struct LocalMarian {
    catalog: PathBuf,
    sha256: String,
    threads: usize,
    notices: SyncSender<Notice>,
    loaded: Mutex<Option<(TranslationDirection, MarianProvider)>>,
}

impl LocalMarian {
    pub(crate) fn new(
        catalog: PathBuf,
        sha256: String,
        threads: usize,
        notices: SyncSender<Notice>,
    ) -> Self {
        Self {
            catalog,
            sha256,
            threads,
            notices,
            loaded: Mutex::new(None),
        }
    }

    fn send_progress(&self, request_id: u64, stage: ProgressStage) {
        let _ = self
            .notices
            .try_send(Notice::Progress { request_id, stage });
    }

    fn run(
        &self,
        request: &TranslationRequest,
        cancellation: &TranslationCancellation,
    ) -> Result<(ProviderOutput, Observation), (TranslationError, IpcError)> {
        let started = Instant::now();
        let request_id = request.context.input_revision;
        let pair = request.direction;
        let mut loaded = self
            .loaded
            .lock()
            .map_err(|_| (TranslationError::ProviderUnavailable, IpcError::Internal))?;
        let mut verification_started_unix_ns = 0;
        let mut loading_started_unix_ns = 0;
        let mut verification_ns = 0;
        let mut load_ns = 0;
        if loaded
            .as_ref()
            .is_none_or(|(direction, _)| *direction != pair)
        {
            // 只驻留一个方向。切换失败后可显式请求原方向并重新校验加载。
            drop(loaded.take());
            cancellation.check().map_err(both_errors)?;
            verification_started_unix_ns = unix_ns();
            self.send_progress(request_id, ProgressStage::Verifying);
            let verify_started = Instant::now();
            let mut load_started = None;
            let provider = MarianProvider::load_direction(
                &self.catalog,
                &self.sha256,
                pair,
                self.threads,
                cancellation,
                |state| {
                    if state == ModelState::Ready {
                        verification_ns = verify_started.elapsed().as_nanos() as u64;
                        loading_started_unix_ns = unix_ns();
                        load_started = Some(Instant::now());
                        self.send_progress(request_id, ProgressStage::Loading);
                    }
                },
            )
            .map_err(|error| {
                if error == TranslationError::ModelLoadFailed && load_started.is_none() {
                    (error, IpcError::ModelInvalid)
                } else {
                    both_errors(error)
                }
            })?;
            load_ns = load_started.expect("成功加载已开始").elapsed().as_nanos() as u64;
            *loaded = Some((pair, provider));
        }
        cancellation.check().map_err(both_errors)?;
        let provider = &loaded.as_ref().expect("同方向模型已加载").1;
        self.send_progress(request_id, ProgressStage::Inferring);
        let inference_started_unix_ns = unix_ns();
        let result = provider.translate_observed(request, cancellation);
        let inference_finished_unix_ns = unix_ns();
        let (output, stats) = result.map_err(both_errors)?;
        Ok((
            output,
            Observation {
                started,
                model_identity: format!("{}@{}", stats.model_id, stats.revision),
                metrics: WorkerMetrics {
                    compute_threads: stats.compute_threads,
                    resident_models: stats.resident_models,
                    verification_started_unix_ns,
                    loading_started_unix_ns,
                    inference_started_unix_ns,
                    inference_finished_unix_ns,
                    verification_ns,
                    load_ns,
                    inference_ns: stats.inference_ns,
                    encoder_ns: stats.encoder_ns,
                    decoder_ns: stats.decoder_ns,
                    generated_steps: stats.generated_steps,
                    queue_wait_ns: 0,
                    output_truncated: stats.output_truncated,
                },
            },
        ))
    }
}

impl TranslationProvider for LocalMarian {
    fn identity(&self) -> ProviderIdentity {
        ProviderIdentity {
            id: PROVIDER_ID.to_owned(),
            version: self.sha256.clone(),
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            directions: [Direction::ZhEn, Direction::EnZh]
                .map(translation_direction)
                .to_vec(),
            kinds: vec![TranslationKind::Sentence],
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
        let result = self.run(request, cancellation);
        let request_id = request.context.input_revision;
        let (output, observation) = match result {
            Ok((output, observation)) => (Ok(output), Ok(observation)),
            Err((error, wire)) => (Err(error), Err(wire)),
        };
        self.notices
            .try_send(Notice::Observed {
                request_id,
                result: observation,
            })
            .map_err(|_| TranslationError::InternalError)?;
        output
    }
}

fn unix_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos() as u64)
}

pub(crate) fn translation_direction(direction: Direction) -> TranslationDirection {
    let (source, target) = match direction {
        Direction::ZhEn => (Language::Chinese, Language::English),
        Direction::EnZh => (Language::English, Language::Chinese),
    };
    TranslationDirection { source, target }
}

fn both_errors(error: TranslationError) -> (TranslationError, IpcError) {
    (error, wire_error(error))
}

pub(crate) fn wire_error(error: TranslationError) -> IpcError {
    match error {
        TranslationError::InvalidInput => IpcError::InvalidInput,
        TranslationError::InputTooLong => IpcError::InputTooLong,
        TranslationError::InvalidConfig => IpcError::ModelInvalid,
        TranslationError::ModelNotFound => IpcError::ModelNotFound,
        TranslationError::ModelLoadFailed => IpcError::ModelLoadFailed,
        TranslationError::UnsupportedLanguagePair | TranslationError::UnsupportedTaskKind => {
            IpcError::UnsupportedDirection
        }
        TranslationError::Busy => IpcError::Busy,
        TranslationError::Cancelled => IpcError::Cancelled,
        TranslationError::Expired | TranslationError::Timeout => IpcError::Timeout,
        TranslationError::InvalidResponse => IpcError::InvalidResponse,
        TranslationError::ResponseTooLarge => IpcError::OutputTruncated,
        _ => IpcError::Internal,
    }
}
