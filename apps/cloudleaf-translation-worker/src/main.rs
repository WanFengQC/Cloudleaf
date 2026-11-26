mod provider;

use cloudleaf_translation::{
    ProviderRegistry, ReadPolicy, ServiceLimits, TaskState, TranslationContext, TranslationKind,
    TranslationProviderConfig, TranslationRequest, TranslationService, TranslationTaskId,
};
use cloudleaf_translation_ipc::{
    Command, Direction, Envelope, IpcError, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, MAX_PENDING,
    MAX_TIMEOUT_MS, PROTOCOL_VERSION, PROVIDER_ID, QualityStatus, RESULT_VERSION, Reply,
    RequestKey, ResourceThreads, WorkerOutput, read_frame, valid_sha256, validate_text,
    write_frame,
};
use provider::{LocalMarian, Notice, Observation, translation_direction, wire_error};
use std::ffi::OsString;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_millis(3);

struct Settings {
    catalog: PathBuf,
    sha256: String,
    threads: ResourceThreads,
}

impl Settings {
    fn parse() -> Option<Self> {
        let args: [OsString; 6] = std::env::args_os()
            .skip(1)
            .collect::<Vec<_>>()
            .try_into()
            .ok()?;
        let [catalog_key, catalog, sha_key, sha, threads_key, threads] = args;
        let sha256 = sha.into_string().ok()?;
        let count = threads.to_str()?.parse::<usize>().ok()?;
        if catalog_key != "--catalog"
            || sha_key != "--sha256"
            || threads_key != "--threads"
            || !valid_sha256(&sha256)
            || std::env::var("RAYON_NUM_THREADS").ok().as_deref() != threads.to_str()
        {
            return None;
        }
        Some(Self {
            catalog: PathBuf::from(catalog),
            sha256,
            threads: ResourceThreads::try_from(count).ok()?,
        })
    }
}

fn main() -> ExitCode {
    let Some(settings) = Settings::parse() else {
        return ExitCode::FAILURE;
    };
    let mut output = BufWriter::new(io::stdout().lock());
    match serve(settings, &mut output) {
        Ok(()) => ExitCode::SUCCESS,
        Err((worker_generation, error)) => {
            let _ = write_frame(
                output,
                &Envelope::new(worker_generation, Reply::Error { key: None, error }),
            );
            ExitCode::FAILURE
        }
    }
}

fn serve(settings: Settings, output: &mut impl Write) -> Result<(), (u64, IpcError)> {
    let mut input = BufReader::new(io::stdin());
    let first: Envelope<Command> = match read_frame(&mut input) {
        Ok(Some(frame)) => frame,
        Ok(None) => return Ok(()),
        Err(error) => return Err((0, error)),
    };
    let generation = first.worker_generation;
    let connection_error = |error| (generation, error);
    check_protocol(&first, generation).map_err(connection_error)?;
    match first.message {
        Command::Handshake { catalog_sha256 } if catalog_sha256 == settings.sha256 => {}
        _ => return Err((generation, IpcError::InvalidIdentity)),
    };
    let (notice_sender, notices) = sync_channel(16);
    let mut worker = Worker::new(settings, notice_sender, notices).map_err(connection_error)?;
    worker
        .reply(
            output,
            generation,
            Reply::Capabilities {
                provider_identity: PROVIDER_ID.to_owned(),
                catalog_sha256: worker.sha256.clone(),
                threads: worker.threads,
                max_pending: MAX_PENDING,
            },
        )
        .map_err(connection_error)?;

    let (sender, receiver) = sync_channel(8);
    thread::Builder::new()
        .name("cloudleaf-worker-command-reader".to_owned())
        .spawn(move || {
            loop {
                let frame = read_frame(&mut input);
                let terminal = !matches!(frame, Ok(Some(_)));
                if sender.send(frame).is_err() || terminal {
                    break;
                }
            }
        })
        .map_err(|_| (generation, IpcError::Internal))?;
    let result = worker.run(output, generation, receiver);
    worker
        .service
        .shutdown()
        .map_err(wire_error)
        .map_err(connection_error)?;
    result.map_err(connection_error)
}

fn check_protocol(frame: &Envelope<Command>, generation: u64) -> Result<(), IpcError> {
    if frame.protocol_version != PROTOCOL_VERSION {
        return Err(IpcError::InvalidProtocolVersion);
    }
    if generation == 0 || frame.worker_generation != generation {
        return Err(IpcError::InvalidIdentity);
    }
    Ok(())
}

struct Task {
    key: RequestKey,
    id: TranslationTaskId,
    direction: Direction,
    admitted_at: Instant,
    deadline: Instant,
    observation: Option<Result<Observation, IpcError>>,
}

struct Worker {
    service: TranslationService,
    notices: Receiver<Notice>,
    active: Vec<Task>,
    latest_request: u64,
    sha256: String,
    threads: ResourceThreads,
}

impl Worker {
    fn new(
        settings: Settings,
        notice_sender: std::sync::mpsc::SyncSender<Notice>,
        notices: Receiver<Notice>,
    ) -> Result<Self, IpcError> {
        let mut registry = ProviderRegistry::new();
        let provider = LocalMarian::new(
            settings.catalog,
            settings.sha256.clone(),
            settings.threads.count(),
            notice_sender,
        )
        .map_err(wire_error)?;
        registry
            .register(
                Arc::new(provider),
                TranslationProviderConfig {
                    timeout: Duration::from_millis(MAX_TIMEOUT_MS),
                    read_policy: ReadPolicy::Research,
                    ..TranslationProviderConfig::default()
                },
            )
            .map_err(wire_error)?;
        let service = TranslationService::new(
            registry,
            ServiceLimits {
                workers: 1,
                queued: MAX_PENDING,
                retained_tasks: MAX_PENDING,
                max_input_bytes: MAX_INPUT_BYTES,
                max_output_bytes: MAX_OUTPUT_BYTES,
                cache_entries: 0,
                cache_bytes: 0,
                offline_only: true,
            },
        )
        .map_err(wire_error)?;
        Ok(Self {
            service,
            notices,
            active: Vec::with_capacity(MAX_PENDING),
            latest_request: 0,
            sha256: settings.sha256,
            threads: settings.threads,
        })
    }

    fn run(
        &mut self,
        output: &mut impl Write,
        generation: u64,
        receiver: Receiver<Result<Option<Envelope<Command>>, IpcError>>,
    ) -> Result<(), IpcError> {
        loop {
            match receiver.recv_timeout(TICK) {
                Ok(Ok(Some(frame))) => {
                    check_protocol(&frame, generation)?;
                    match frame.message {
                        Command::Request {
                            key,
                            direction,
                            text,
                            timeout_ms,
                        } => {
                            let reply = self.submit(generation, key, direction, text, timeout_ms);
                            self.reply(output, generation, reply)?;
                        }
                        Command::Cancel { key } if key.generation == generation => {
                            if let Some(index) = self.active.iter().position(|task| task.key == key)
                            {
                                self.cancel(index)?;
                                self.reply(output, generation, Reply::Cancelled { key })?;
                            }
                        }
                        Command::Cancel { .. } => return Err(IpcError::InvalidIdentity),
                        Command::Handshake { .. } => return Err(IpcError::InvalidMessage),
                        Command::Shutdown {} => return Ok(()),
                    }
                }
                Ok(Ok(None)) | Err(RecvTimeoutError::Disconnected) => return Ok(()),
                Ok(Err(error)) => return Err(error),
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.advance(output, generation)?;
        }
    }

    fn submit(
        &mut self,
        generation: u64,
        key: RequestKey,
        direction: Direction,
        text: String,
        timeout_ms: u64,
    ) -> Reply {
        let error = |error| Reply::Error {
            key: Some(key),
            error,
        };
        if key.generation != generation || key.source.session == 0 || key.source.epoch == 0 {
            return error(IpcError::InvalidIdentity);
        }
        if key.request_id <= self.latest_request {
            return error(IpcError::DuplicateRequest);
        }
        self.latest_request = key.request_id;
        if self.active.len() == MAX_PENDING {
            return error(IpcError::Busy);
        }
        if !(1..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
            return error(IpcError::InvalidConfiguration);
        }
        if let Err(reason) = validate_text(&text, MAX_INPUT_BYTES) {
            return error(reason);
        }
        let admitted_at = Instant::now();
        let request = TranslationRequest {
            text,
            direction: translation_direction(direction),
            kind: TranslationKind::Sentence,
            // 工作进程内的 scheduler 通知用请求编号关联；IPC 保留真实输入修订号。
            context: scheduler_context(key),
            send_authorized_to: None,
            use_cache: false,
        };
        match self.service.submit(PROVIDER_ID, request) {
            Ok(id) => {
                self.active.push(Task {
                    key,
                    id,
                    direction,
                    admitted_at,
                    deadline: admitted_at + Duration::from_millis(timeout_ms),
                    observation: None,
                });
                Reply::Accepted { key, task_id: id.0 }
            }
            Err(reason) => error(wire_error(reason)),
        }
    }

    fn cancel(&mut self, index: usize) -> Result<(), IpcError> {
        let task = self.active.remove(index);
        self.service.cancel(task.id).map_err(wire_error)?;
        self.service.forget(task.id).map_err(wire_error)
    }

    fn collect_notices(
        &mut self,
        output: &mut impl Write,
        generation: u64,
    ) -> Result<(), IpcError> {
        while let Ok(notice) = self.notices.try_recv() {
            let Some(task) = self
                .active
                .iter_mut()
                .find(|task| task.key.request_id == notice.request_id())
            else {
                continue;
            };
            match notice {
                Notice::Progress { stage, .. } => {
                    write_frame(
                        &mut *output,
                        &Envelope::new(
                            generation,
                            Reply::Progress {
                                key: task.key,
                                task_id: task.id.0,
                                stage,
                            },
                        ),
                    )?;
                }
                Notice::Observed { result, .. } => task.observation = Some(result),
            }
        }
        Ok(())
    }

    fn advance(&mut self, output: &mut impl Write, generation: u64) -> Result<(), IpcError> {
        self.collect_notices(output, generation)?;
        let mut index = 0;
        while index < self.active.len() {
            let task = &self.active[index];
            if Instant::now() >= task.deadline {
                let key = task.key;
                self.cancel(index)?;
                self.reply(
                    output,
                    generation,
                    Reply::Error {
                        key: Some(key),
                        error: IpcError::Timeout,
                    },
                )?;
                continue;
            }
            let snapshot = self.service.poll(task.id).map_err(wire_error)?;
            if !snapshot.state.is_terminal() {
                index += 1;
                continue;
            }
            // provider 先发送观测，再由 scheduler 发布终态，两次轮询确保关联观测可取。
            self.collect_notices(output, generation)?;
            let task = self.active.remove(index);
            self.service.forget(task.id).map_err(wire_error)?;
            let reply = match (snapshot.state, snapshot.result, task.observation) {
                (TaskState::Succeeded, Some(result), Some(Ok(mut observation)))
                    if result.task_id == task.id
                        && result.context == scheduler_context(task.key)
                        && result.provider.id == PROVIDER_ID
                        && result.provider.version == self.sha256
                        && result.direction == translation_direction(task.direction)
                        && result.output.source_ids.as_slice()
                            == [observation.model_identity.as_str()] =>
                {
                    observation.metrics.queue_wait_ns = observation
                        .started
                        .saturating_duration_since(task.admitted_at)
                        .as_nanos() as u64;
                    Reply::Result {
                        key: task.key,
                        task_id: task.id.0,
                        result_version: RESULT_VERSION,
                        output: WorkerOutput {
                            raw_translation: result.output.translated_text,
                            direction: task.direction,
                            provider_identity: result.provider.id,
                            model_identity: observation.model_identity,
                            catalog_sha256: result.provider.version,
                            quality: QualityStatus::ResearchOnly,
                            metrics: Box::new(observation.metrics),
                        },
                    }
                }
                (TaskState::Cancelled, _, _) => Reply::Cancelled { key: task.key },
                (_, _, Some(Err(error))) => Reply::Error {
                    key: Some(task.key),
                    error,
                },
                (_, _, _) => Reply::Error {
                    key: Some(task.key),
                    error: snapshot.error.map_or(IpcError::InvalidResponse, wire_error),
                },
            };
            self.reply(output, generation, reply)?;
        }
        Ok(())
    }

    fn reply(
        &self,
        output: &mut impl Write,
        generation: u64,
        message: Reply,
    ) -> Result<(), IpcError> {
        write_frame(output, &Envelope::new(generation, message))
    }
}

fn scheduler_context(key: RequestKey) -> TranslationContext {
    TranslationContext {
        session: key.source.session,
        epoch: key.source.epoch,
        input_revision: key.request_id,
    }
}
