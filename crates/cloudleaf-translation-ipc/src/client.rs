mod transport;

use crate::{
    Command, Direction, Envelope, IpcError, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, MAX_PENDING,
    MAX_TIMEOUT_MS, PROTOCOL_VERSION, PROVIDER_ID, ProgressStage, RESULT_VERSION, Reply,
    RequestKey, ResourceThreads, SourceIdentity, WorkerOutput, valid_sha256, validate_text,
};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use transport::Process;

const EVENT_CAPACITY: usize = 32;
const TICK: Duration = Duration::from_millis(5);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WorkerScheduling {
    #[default]
    Normal,
    /// 仅限 Windows 子进程，保持调用者线程及进程调度不变。
    BelowNormal,
}

#[derive(Clone)]
pub struct WorkerConfig {
    pub(super) catalog: PathBuf,
    pub(super) catalog_sha256: String,
    pub(super) threads: ResourceThreads,
    pub(super) scheduling: WorkerScheduling,
    pub(super) host_executable: Option<PathBuf>,
}

impl WorkerConfig {
    pub fn sibling(
        catalog: PathBuf,
        catalog_sha256: String,
        threads: ResourceThreads,
    ) -> Result<Self, IpcError> {
        if catalog.as_os_str().is_empty() || !valid_sha256(&catalog_sha256) {
            return Err(IpcError::InvalidConfiguration);
        }
        Ok(Self {
            catalog,
            catalog_sha256,
            threads,
            scheduling: WorkerScheduling::Normal,
            host_executable: None,
        })
    }

    /// 自动档仅使用宿主当前可用的逻辑处理器数，显式档仍由调用者选择。
    pub fn sibling_auto(catalog: PathBuf, catalog_sha256: String) -> Result<Self, IpcError> {
        let available = thread::available_parallelism().map_or(1, usize::from);
        Self::sibling(
            catalog,
            catalog_sha256,
            ResourceThreads::for_available_parallelism(available),
        )
    }

    pub fn compute_threads(&self) -> ResourceThreads {
        self.threads
    }

    pub fn catalog_path(&self) -> &std::path::Path {
        &self.catalog
    }

    pub fn catalog_sha256(&self) -> &str {
        &self.catalog_sha256
    }

    pub fn with_scheduling(mut self, scheduling: WorkerScheduling) -> Result<Self, IpcError> {
        if scheduling == WorkerScheduling::BelowNormal && !cfg!(windows) {
            return Err(IpcError::InvalidConfiguration);
        }
        self.scheduling = scheduling;
        Ok(self)
    }

    /// 从显式的宿主路径定位兄弟程序，供独立命令和测试宿主使用。
    pub fn for_host_executable(mut self, path: PathBuf) -> Result<Self, IpcError> {
        if !path.is_absolute() {
            return Err(IpcError::InvalidWorkerPath);
        }
        self.host_executable = Some(path);
        Ok(self)
    }
}

#[cfg(test)]
#[test]
fn automatic_worker_selection_is_bounded_and_default_scheduling_is_unchanged() {
    let config = WorkerConfig::sibling_auto(PathBuf::from("catalog.json"), "a".repeat(64)).unwrap();
    let cpus = thread::available_parallelism().map_or(1, usize::from);
    assert_eq!(
        config.compute_threads(),
        ResourceThreads::for_available_parallelism(cpus)
    );
    assert_eq!(config.scheduling, WorkerScheduling::Normal);
    for (available, expected) in [(0, 1), (1, 1), (4, 1), (5, 1), (6, 2), (64, 2)] {
        assert_eq!(
            ResourceThreads::for_available_parallelism(available).count(),
            expected
        );
    }
    let configured = config.with_scheduling(WorkerScheduling::BelowNormal);
    assert_eq!(configured.is_ok(), cfg!(windows));
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WorkerState {
    #[default]
    NotStarted,
    Starting,
    Ready,
    Busy,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerEvent {
    State(WorkerState),
    Accepted {
        key: RequestKey,
        task_id: u64,
    },
    Progress {
        key: RequestKey,
        task_id: u64,
        stage: ProgressStage,
    },
    Completed {
        key: RequestKey,
        task_id: u64,
        result_version: u32,
        output: WorkerOutput,
    },
    Failed {
        key: RequestKey,
        error: IpcError,
    },
    Cancelled {
        key: RequestKey,
    },
}

impl WorkerEvent {
    fn key(&self) -> Option<RequestKey> {
        match self {
            Self::State(_) => None,
            Self::Accepted { key, .. }
            | Self::Progress { key, .. }
            | Self::Completed { key, .. }
            | Self::Failed { key, .. }
            | Self::Cancelled { key } => Some(*key),
        }
    }

    fn terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Failed { .. } | Self::Cancelled { .. }
        )
    }
}

struct Backend {
    generation: u64,
    submit: SyncSender<Pending>,
    events: Receiver<WorkerEvent>,
    stop: Arc<AtomicBool>,
    pid: Arc<AtomicU32>,
    handle: JoinHandle<()>,
}

struct Pending {
    key: RequestKey,
    direction: Direction,
    text: Option<String>,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
    task_id: Option<u64>,
    stage: Option<ProgressStage>,
}

struct Admission {
    key: RequestKey,
    cancel: Arc<AtomicBool>,
}

pub struct WorkerClient {
    config: WorkerConfig,
    backend: Option<Backend>,
    active: Vec<Admission>,
    next_request: u64,
    state: WorkerState,
    closed: bool,
}

impl WorkerClient {
    pub fn new(config: WorkerConfig) -> Self {
        Self {
            config,
            backend: None,
            active: Vec::with_capacity(MAX_PENDING),
            next_request: 1,
            state: WorkerState::NotStarted,
            closed: false,
        }
    }

    /// 显式启动传输，握手不访问模型。普通输入无需调用此方法。
    pub fn start(&mut self) -> Result<(), IpcError> {
        self.prepare()?;
        if self.backend.is_none() {
            self.spawn(new_generation()?, None)?;
        }
        Ok(())
    }

    pub fn state(&self) -> WorkerState {
        self.state
    }

    pub fn child_pid(&self) -> Option<u32> {
        self.backend
            .as_ref()
            .map(|backend| backend.pid.load(Ordering::Acquire))
            .filter(|pid| *pid != 0)
    }

    pub fn submit(
        &mut self,
        source: SourceIdentity,
        direction: Direction,
        text: String,
        timeout: Duration,
    ) -> Result<RequestKey, IpcError> {
        validate_text(&text, MAX_INPUT_BYTES)?;
        if timeout.as_millis() == 0 || timeout.as_millis() > u128::from(MAX_TIMEOUT_MS) {
            return Err(IpcError::InvalidConfiguration);
        }
        if source.session == 0 || source.epoch == 0 {
            return Err(IpcError::InvalidIdentity);
        }
        self.prepare()?;
        let request_id = self.next_request;
        let next_request = request_id.checked_add(1).ok_or(IpcError::Busy)?;
        let generation = match &self.backend {
            Some(backend) => backend.generation,
            None => new_generation()?,
        };
        let key = RequestKey {
            generation,
            source,
            request_id,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let pending = Pending {
            key,
            direction,
            text: Some(text),
            deadline: Instant::now() + timeout,
            cancel: Arc::clone(&cancel),
            task_id: None,
            stage: None,
        };
        if let Some(backend) = &self.backend {
            backend
                .submit
                .try_send(pending)
                .map_err(|error| match error {
                    TrySendError::Full(_) => IpcError::Busy,
                    TrySendError::Disconnected(_) => IpcError::WorkerExited,
                })?;
        } else {
            self.spawn(generation, Some(pending))?;
        }
        self.next_request = next_request;
        self.active.push(Admission { key, cancel });
        Ok(key)
    }

    pub fn cancel(&self, key: RequestKey) -> bool {
        let Some(admission) = self.active.iter().find(|entry| entry.key == key) else {
            return false;
        };
        !admission.cancel.swap(true, Ordering::AcqRel)
    }

    pub fn try_event(&mut self) -> Option<WorkerEvent> {
        let backend = self.backend.as_ref()?;
        loop {
            let event = match backend.events.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    let entry = self.active.pop()?;
                    self.state = WorkerState::Failed;
                    return Some(WorkerEvent::Failed {
                        key: entry.key,
                        error: IpcError::WorkerExited,
                    });
                }
            };
            let Some(key) = event.key() else {
                if let WorkerEvent::State(state) = event {
                    self.state = state;
                }
                return Some(event);
            };
            let Some(index) = self.active.iter().position(|entry| entry.key == key) else {
                continue;
            };
            let cancelled = self.active[index].cancel.load(Ordering::Acquire);
            if event.terminal() {
                self.active.remove(index);
                return if cancelled {
                    Some(WorkerEvent::Cancelled { key })
                } else {
                    Some(event)
                };
            }
            if !cancelled {
                return Some(event);
            }
        }
    }

    /// 消息线程只登记关闭。退出消息循环后可在有界等待内收割监督线程。
    pub fn close(&mut self) {
        self.closed = true;
        for admission in &self.active {
            admission.cancel.store(true, Ordering::Release);
        }
        if let Some(backend) = &self.backend {
            backend.stop.store(true, Ordering::Release);
        }
    }

    pub fn wait_for_shutdown(&mut self, timeout: Duration) -> bool {
        self.close();
        let deadline = Instant::now() + timeout;
        while let Some(backend) = &self.backend {
            if backend.handle.is_finished() {
                self.active.clear();
                if backend.pid.load(Ordering::Acquire) != 0 {
                    // 监督线程结束不证明子进程已经退出，保留失败与子进程标识。
                    self.state = WorkerState::Failed;
                    return false;
                }
                if let Some(backend) = self.backend.take() {
                    let _ = backend.handle.join();
                }
                self.state = WorkerState::Stopped;
                return true;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            thread::sleep(TICK.min(remaining));
        }
        self.state = WorkerState::Stopped;
        true
    }

    fn prepare(&mut self) -> Result<(), IpcError> {
        if self.closed {
            return Err(IpcError::WorkerExited);
        }
        if self.active.len() >= MAX_PENDING {
            return Err(IpcError::Busy);
        }
        if self
            .backend
            .as_ref()
            .is_some_and(|backend| backend.handle.is_finished())
        {
            if !self.active.is_empty() {
                return Err(IpcError::Busy);
            }
            if self.child_pid().is_some() {
                return Err(IpcError::WorkerReapFailed);
            }
            // 旧进程已经退出，只有下一次显式请求才分配新代。
            self.backend.take();
        }
        Ok(())
    }

    fn spawn(&mut self, generation: u64, initial: Option<Pending>) -> Result<(), IpcError> {
        let (submit, requests) = sync_channel(MAX_PENDING);
        let (events, receiver) = sync_channel(EVENT_CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));
        let pid = Arc::new(AtomicU32::new(0));
        let manager = Supervisor {
            config: self.config.clone(),
            generation,
            requests,
            events,
            stop: Arc::clone(&stop),
            pid: Arc::clone(&pid),
            pending: initial.into_iter().collect(),
            ready: false,
            last_task_id: 0,
        };
        let handle = thread::Builder::new()
            .name("cloudleaf-worker-supervisor".to_owned())
            .spawn(move || manager.run())
            .map_err(|_| IpcError::WorkerStartFailed)?;
        self.backend = Some(Backend {
            generation,
            submit,
            events: receiver,
            stop,
            pid,
            handle,
        });
        self.state = WorkerState::Starting;
        Ok(())
    }
}

impl Drop for WorkerClient {
    fn drop(&mut self) {
        self.close();
    }
}

fn new_generation() -> Result<u64, IpcError> {
    NEXT_GENERATION
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| IpcError::Busy)
}

struct Supervisor {
    config: WorkerConfig,
    generation: u64,
    requests: Receiver<Pending>,
    events: SyncSender<WorkerEvent>,
    stop: Arc<AtomicBool>,
    pid: Arc<AtomicU32>,
    pending: VecDeque<Pending>,
    ready: bool,
    last_task_id: u64,
}

impl Supervisor {
    fn run(mut self) {
        let _ = self.emit(WorkerEvent::State(WorkerState::Starting));
        let result = self.supervise();
        self.pending.extend(self.requests.try_iter());
        while let Some(pending) = self.pending.pop_front() {
            let _ = self.emit(match result {
                Ok(()) => WorkerEvent::Cancelled { key: pending.key },
                Err(error) => WorkerEvent::Failed {
                    key: pending.key,
                    error,
                },
            });
        }
        let state = if result.is_ok() {
            WorkerState::Stopped
        } else {
            WorkerState::Failed
        };
        let _ = self.emit(WorkerEvent::State(state));
    }

    fn supervise(&mut self) -> Result<(), IpcError> {
        if self.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        let process = Process::launch(&self.config, &self.pid)?;
        let result = self.exchange(&process);
        let _ = self.emit(WorkerEvent::State(WorkerState::Stopping));
        if process.stop(self.generation) {
            self.pid.store(0, Ordering::Release);
            result
        } else {
            Err(IpcError::WorkerReapFailed)
        }
    }

    fn exchange(&mut self, process: &Process) -> Result<(), IpcError> {
        process.send(Envelope::new(
            self.generation,
            Command::Handshake {
                catalog_sha256: self.config.catalog_sha256.clone(),
            },
        ))?;
        let handshake_deadline = Instant::now() + HANDSHAKE_TIMEOUT;
        while !self.stop.load(Ordering::Acquire) {
            self.pending.extend(self.requests.try_iter());
            self.dispatch(process)?;
            if !self.ready && Instant::now() >= handshake_deadline {
                return Err(IpcError::HandshakeTimeout);
            }
            if let Some(message) = process.receive(TICK)? {
                self.receive(message)?;
            }
        }
        Ok(())
    }

    fn dispatch(&mut self, process: &Process) -> Result<(), IpcError> {
        let mut index = 0;
        while index < self.pending.len() {
            let pending = &mut self.pending[index];
            let remaining = pending.deadline.saturating_duration_since(Instant::now());
            let cancelled = pending.cancel.load(Ordering::Acquire);
            let key = pending.key;
            let already_sent = pending.text.is_none();
            if cancelled || remaining.is_zero() {
                if already_sent {
                    process.send(Envelope::new(self.generation, Command::Cancel { key }))?;
                }
                self.complete(
                    index,
                    if cancelled {
                        WorkerEvent::Cancelled { key }
                    } else {
                        WorkerEvent::Failed {
                            key,
                            error: IpcError::Timeout,
                        }
                    },
                )?;
                continue;
            }
            if self.ready
                && let Some(text) = pending.text.take()
            {
                let timeout_ms = (remaining.as_millis() as u64).clamp(1, MAX_TIMEOUT_MS);
                process.send(Envelope::new(
                    self.generation,
                    Command::Request {
                        key,
                        direction: pending.direction,
                        text,
                        timeout_ms,
                    },
                ))?;
                self.emit(WorkerEvent::State(WorkerState::Busy))?;
            }
            index += 1;
        }
        Ok(())
    }

    fn receive(&mut self, frame: Envelope<Reply>) -> Result<(), IpcError> {
        if frame.worker_generation != self.generation {
            // 旧进程输出绝不能触发当前会话的事务。
            return Ok(());
        }
        if frame.protocol_version != PROTOCOL_VERSION {
            return Err(IpcError::InvalidProtocolVersion);
        }
        self.reply(frame.message)
    }

    fn reply(&mut self, reply: Reply) -> Result<(), IpcError> {
        if !self.ready {
            match reply {
                Reply::Capabilities {
                    provider_identity,
                    catalog_sha256,
                    threads,
                    max_pending,
                } if provider_identity == PROVIDER_ID
                    && catalog_sha256 == self.config.catalog_sha256
                    && threads == self.config.threads
                    && max_pending == MAX_PENDING =>
                {
                    self.ready = true;
                    return self.emit(WorkerEvent::State(WorkerState::Ready));
                }
                Reply::Error { key: None, error } => return Err(error),
                _ => return Err(IpcError::InvalidIdentity),
            }
        }
        let key = match &reply {
            Reply::Accepted { key, .. }
            | Reply::Progress { key, .. }
            | Reply::Result { key, .. }
            | Reply::Cancelled { key }
            | Reply::Error { key: Some(key), .. } => *key,
            Reply::Error { key: None, error } => return Err(*error),
            Reply::Capabilities { .. } => return Err(IpcError::InvalidMessage),
        };
        let Some(index) = self.pending.iter().position(|pending| pending.key == key) else {
            // 已经结束的任务及重复响应不会被重新接纳。
            return Ok(());
        };
        let pending = &mut self.pending[index];
        if Instant::now() >= pending.deadline || pending.cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        if pending.text.is_some() {
            return Err(IpcError::InvalidIdentity);
        }
        match reply {
            Reply::Accepted { task_id, .. }
                if pending.task_id.is_none() && task_id > self.last_task_id =>
            {
                pending.task_id = Some(task_id);
                self.last_task_id = task_id;
                self.emit(WorkerEvent::Accepted { key, task_id })
            }
            Reply::Progress { task_id, stage, .. }
                if pending.task_id == Some(task_id)
                    && pending.stage.is_none_or(|observed| observed < stage) =>
            {
                pending.stage = Some(stage);
                self.emit(WorkerEvent::Progress {
                    key,
                    task_id,
                    stage,
                })
            }
            Reply::Result {
                task_id,
                result_version,
                output,
                ..
            } if pending.task_id == Some(task_id)
                && result_version == RESULT_VERSION
                && output.direction == pending.direction
                && output.provider_identity == PROVIDER_ID
                && output.catalog_sha256 == self.config.catalog_sha256
                && !output.model_identity.is_empty()
                && output.metrics.compute_threads == self.config.threads.count()
                && output.metrics.resident_models == 1 =>
            {
                match validate_text(&output.raw_translation, MAX_OUTPUT_BYTES) {
                    Ok(()) => {}
                    Err(IpcError::InputTooLong) => {
                        return self.complete(
                            index,
                            WorkerEvent::Failed {
                                key,
                                error: IpcError::OutputTruncated,
                            },
                        );
                    }
                    Err(_) => return Err(IpcError::InvalidResponse),
                }
                self.complete(
                    index,
                    WorkerEvent::Completed {
                        key,
                        task_id,
                        result_version,
                        output,
                    },
                )
            }
            Reply::Error { error, .. } => self.complete(index, WorkerEvent::Failed { key, error }),
            Reply::Cancelled { .. } => self.complete(index, WorkerEvent::Cancelled { key }),
            _ => Err(IpcError::InvalidResponse),
        }
    }

    fn complete(&mut self, index: usize, event: WorkerEvent) -> Result<(), IpcError> {
        self.pending.remove(index);
        self.emit(event)?;
        if self.ready && self.pending.is_empty() {
            self.emit(WorkerEvent::State(WorkerState::Ready))?;
        }
        Ok(())
    }

    fn emit(&self, event: WorkerEvent) -> Result<(), IpcError> {
        self.events
            .try_send(event)
            .map_err(|_| IpcError::BrokenPipe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flight(deadline: Instant) -> (Supervisor, RequestKey, Receiver<WorkerEvent>) {
        let config = WorkerConfig::sibling(
            PathBuf::from("test-catalog.json"),
            "a".repeat(64),
            ResourceThreads::One,
        )
        .unwrap();
        let key = RequestKey {
            source: SourceIdentity {
                session: 7,
                epoch: 4,
                input_revision: 32,
            },
            generation: 10,
            request_id: 24,
        };
        let (_, requests) = sync_channel(MAX_PENDING);
        let (events, received) = sync_channel(EVENT_CAPACITY);
        let supervisor = Supervisor {
            config,
            generation: key.generation,
            requests,
            events,
            stop: Arc::new(AtomicBool::new(false)),
            pid: Arc::new(AtomicU32::new(0)),
            pending: VecDeque::from([Pending {
                key,
                direction: Direction::ZhEn,
                text: None,
                deadline,
                cancel: Arc::new(AtomicBool::new(false)),
                task_id: None,
                stage: None,
            }]),
            ready: true,
            last_task_id: 0,
        };
        (supervisor, key, received)
    }

    #[test]
    fn stale_generation_and_replayed_terminal_reply_never_reach_the_host() {
        let (mut supervisor, key, events) = flight(Instant::now() + Duration::from_secs(1));
        let old_ack = Reply::Accepted { key, task_id: 99 };
        supervisor
            .receive(Envelope::new(key.generation - 1, old_ack))
            .unwrap();
        assert!(events.try_recv().is_err());
        let current_ack = || Reply::Accepted { key, task_id: 1 };
        supervisor
            .receive(Envelope::new(key.generation, current_ack()))
            .unwrap();
        assert_eq!(
            events.try_recv().unwrap(),
            WorkerEvent::Accepted { key, task_id: 1 }
        );
        assert_eq!(
            supervisor.receive(Envelope::new(key.generation, current_ack())),
            Err(IpcError::InvalidResponse),
        );
        let cancelled = || Envelope::new(key.generation, Reply::Cancelled { key });
        supervisor.receive(cancelled()).unwrap();
        assert_eq!(events.try_recv().unwrap(), WorkerEvent::Cancelled { key });
        assert_eq!(
            events.try_recv().unwrap(),
            WorkerEvent::State(WorkerState::Ready)
        );
        supervisor.receive(cancelled()).unwrap();
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn shutdown_cannot_later_claim_success_after_reap_was_not_proven() {
        let (supervisor, _, events) = flight(Instant::now());
        let (submit, _) = sync_channel(MAX_PENDING);
        let mut client = WorkerClient::new(supervisor.config);
        client.backend = Some(Backend {
            generation: 1,
            submit,
            events,
            stop: Arc::new(AtomicBool::new(false)),
            pid: Arc::new(AtomicU32::new(42)),
            handle: thread::spawn(|| {}),
        });
        assert!(!client.wait_for_shutdown(Duration::from_secs(1)));
        assert_eq!(client.state(), WorkerState::Failed);
        assert!(!client.wait_for_shutdown(Duration::ZERO));
        assert_eq!(client.state(), WorkerState::Failed);
        assert_eq!(client.child_pid(), Some(42));
    }

    #[test]
    fn elapsed_deadline_refuses_a_child_reply_before_next_dispatch_tick() {
        let (mut supervisor, key, events) = flight(Instant::now() - Duration::from_secs(1));
        let response = Reply::Accepted { key, task_id: 1 };
        supervisor
            .receive(Envelope::new(key.generation, response))
            .unwrap();
        assert!(events.try_recv().is_err());
        assert!(supervisor.pending[0].task_id.is_none());
    }
}
