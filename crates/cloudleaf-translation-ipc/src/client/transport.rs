use super::WorkerConfig;
use crate::{Command, Envelope, IpcError, Reply, read_frame, write_frame};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const TRANSPORT_CAPACITY: usize = 16;
const EXIT_TIMEOUT: Duration = Duration::from_millis(1500);

pub(super) struct Process {
    child: Child,
    stdin: SyncSender<Envelope<Command>>,
    stdout: Receiver<Result<Envelope<Reply>, IpcError>>,
    writer: JoinHandle<()>,
    reader: JoinHandle<()>,
    received_too_many: Arc<AtomicBool>,
}

impl Process {
    pub(super) fn launch(config: &WorkerConfig) -> Result<Self, IpcError> {
        let executable = worker_path(config)?;
        let threads = config.threads.count().to_string();
        let mut command = ProcessCommand::new(executable);
        command
            .arg("--catalog")
            .arg(&config.catalog)
            .arg("--sha256")
            .arg(&config.catalog_sha256)
            .arg("--threads")
            .arg(&threads)
            .env("RAYON_NUM_THREADS", &threads)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|_| IpcError::WorkerStartFailed)?;
        let pipe_in = child.stdin.take().expect("已请求子进程标准输入管道");
        let pipe_out = child.stdout.take().expect("已请求子进程标准输出管道");
        let (stdin, outbound) = sync_channel(TRANSPORT_CAPACITY);
        let (inbound, stdout) = sync_channel(TRANSPORT_CAPACITY);
        let writer_errors = inbound.clone();
        let writer = thread::Builder::new()
            .name("cloudleaf-worker-pipe-writer".to_owned())
            .spawn(move || {
                let mut pipe = BufWriter::new(pipe_in);
                while let Ok(message) = outbound.recv() {
                    if let Err(error) = write_frame(&mut pipe, &message) {
                        let _ = writer_errors.try_send(Err(error));
                        break;
                    }
                }
            });
        let writer = match writer {
            Ok(writer) => writer,
            Err(_) => {
                terminate_child(&mut child);
                return Err(IpcError::WorkerStartFailed);
            }
        };
        let received_too_many = Arc::new(AtomicBool::new(false));
        let input_overload = Arc::clone(&received_too_many);
        let reader = thread::Builder::new()
            .name("cloudleaf-worker-pipe-reader".to_owned())
            .spawn(move || {
                let mut pipe = BufReader::new(pipe_out);
                loop {
                    match read_frame(&mut pipe) {
                        Ok(Some(reply)) => match inbound.try_send(Ok(reply)) {
                            Ok(()) => {}
                            Err(TrySendError::Full(_)) => {
                                input_overload.store(true, Ordering::Release);
                                break;
                            }
                            Err(TrySendError::Disconnected(_)) => break,
                        },
                        Ok(None) => {
                            let _ = inbound.try_send(Err(IpcError::WorkerExited));
                            break;
                        }
                        Err(error) => {
                            let _ = inbound.try_send(Err(error));
                            break;
                        }
                    }
                }
            });
        match reader {
            Ok(reader) => Ok(Self {
                child,
                stdin,
                stdout,
                reader,
                writer,
                received_too_many,
            }),
            Err(_) => {
                drop(stdin);
                terminate_child(&mut child);
                Err(IpcError::WorkerStartFailed)
            }
        }
    }

    pub(super) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(super) fn send(&self, envelope: Envelope<Command>) -> Result<(), IpcError> {
        self.stdin.try_send(envelope).map_err(|error| match error {
            TrySendError::Full(_) => IpcError::Busy,
            TrySendError::Disconnected(_) => IpcError::BrokenPipe,
        })
    }

    pub(super) fn receive(&self, timeout: Duration) -> Result<Option<Envelope<Reply>>, IpcError> {
        if self.received_too_many.load(Ordering::Acquire) {
            return Err(IpcError::InvalidResponse);
        }
        match self.stdout.recv_timeout(timeout) {
            Ok(result) => result.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(IpcError::WorkerExited),
        }
    }

    pub(super) fn stop(self, generation: u64) -> bool {
        let _ = self.send(Envelope::new(generation, Command::Shutdown {}));
        let Self {
            mut child,
            stdin,
            stdout,
            reader,
            writer,
            ..
        } = self;
        drop(stdin);
        drop(stdout);
        let deadline = Instant::now() + EXIT_TIMEOUT;
        let reaped = wait_child(&mut child, deadline) || terminate_child(&mut child);
        for handle in [reader, writer] {
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
        reaped
    }
}

fn wait_child(child: &mut Child, deadline: Instant) -> bool {
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => return false,
        }
    }
}

fn terminate_child(child: &mut Child) -> bool {
    // 仅使用本监督器刚创建的子进程句柄，不查找或终止其他进程。
    let _ = child.kill();
    wait_child(child, Instant::now() + Duration::from_secs(1))
}

fn worker_path(config: &WorkerConfig) -> Result<PathBuf, IpcError> {
    let host = match &config.host_executable {
        Some(path) => path.clone(),
        None => std::env::current_exe().map_err(|_| IpcError::InvalidWorkerPath)?,
    };
    let directory = host
        .parent()
        .ok_or(IpcError::InvalidWorkerPath)?
        .canonicalize()
        .map_err(|_| IpcError::InvalidWorkerPath)?;
    let file = directory.join(if cfg!(windows) {
        "cloudleaf-translation-worker.exe"
    } else {
        "cloudleaf-translation-worker"
    });
    let resolved = file
        .canonicalize()
        .map_err(|_| IpcError::InvalidWorkerPath)?;
    if resolved != file || !resolved.is_file() {
        return Err(IpcError::InvalidWorkerPath);
    }
    Ok(resolved)
}
