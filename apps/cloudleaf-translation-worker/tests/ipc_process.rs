use cloudleaf_translation_ipc::{
    Command, Direction, Envelope, IpcError, MAX_FRAME_BYTES, PROTOCOL_VERSION, PROVIDER_ID,
    ProgressStage, Reply, RequestKey, ResourceThreads, SourceIdentity, WorkerClient, WorkerConfig,
    WorkerEvent, WorkerState, read_frame, write_frame,
};
use std::fs;
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command as ProcessCommand, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

const WORKER: &str = env!("CARGO_BIN_EXE_cloudleaf-translation-worker");
const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const WAIT: Duration = Duration::from_secs(12);
const SOURCE: SourceIdentity = SourceIdentity {
    session: 21,
    epoch: 8,
    input_revision: 42,
};
static TEMP_ID: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "cloudleaf-worker-test-{}-{}",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("catalog.json"));
        let _ = fs::remove_dir(&self.0);
    }
}

struct ChildWire {
    child: Child,
    stdin: Option<ChildStdin>,
    replies: Receiver<Result<Option<Envelope<Reply>>, IpcError>>,
}

impl ChildWire {
    fn start(path: PathBuf) -> Self {
        let mut command = ProcessCommand::new(WORKER);
        command
            .arg("--catalog")
            .arg(path)
            .args(["--sha256", SHA, "--threads", "1"])
            .env("RAYON_NUM_THREADS", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().unwrap();
        let stdin = child.stdin.take();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let (sender, replies) = sync_channel(12);
        thread::spawn(move || {
            loop {
                let frame = read_frame(&mut stdout);
                let terminal = !matches!(frame, Ok(Some(_)));
                if sender.send(frame).is_err() || terminal {
                    break;
                }
            }
        });
        Self {
            child,
            stdin,
            replies,
        }
    }

    fn send(&mut self, generation: u64, command: Command) {
        write_frame(
            self.stdin.as_mut().unwrap(),
            &Envelope::new(generation, command),
        )
        .unwrap();
    }

    fn reply(&mut self) -> Envelope<Reply> {
        self.replies.recv_timeout(WAIT).unwrap().unwrap().unwrap()
    }

    fn handshake(&mut self, generation: u64) {
        self.send(
            generation,
            Command::Handshake {
                catalog_sha256: SHA.to_owned(),
            },
        );
        let ready = self.reply();
        assert_eq!(ready.worker_generation, generation);
        assert!(
            matches!(ready.message, Reply::Capabilities { ref provider_identity, threads: ResourceThreads::One, .. } if provider_identity == PROVIDER_ID)
        );
    }

    fn wait(&mut self) -> bool {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.success();
            }
            assert!(Instant::now() < deadline, "子进程退出超时");
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for ChildWire {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn request(key: RequestKey) -> Command {
    Command::Request {
        key,
        direction: Direction::ZhEn,
        text: "你好".to_owned(),
        timeout_ms: 2_000,
    }
}

fn next_event(client: &mut WorkerClient, select: impl Fn(&WorkerEvent) -> bool) -> WorkerEvent {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(event) = client.try_event()
            && select(&event)
        {
            return event;
        }
        assert!(Instant::now() < deadline, "工作进程事件超时");
        thread::sleep(Duration::from_millis(3));
    }
}

fn client(catalog: PathBuf, sha: String, threads: ResourceThreads) -> WorkerClient {
    WorkerClient::new(
        WorkerConfig::sibling(catalog, sha, threads)
            .unwrap()
            .for_host_executable(PathBuf::from(WORKER))
            .unwrap(),
    )
}

#[test]
fn process_rejects_wrong_version_and_exits() {
    let directory = TestDir::new();
    let mut wire = ChildWire::start(directory.0.join("missing.json"));
    let mut frame = Envelope::new(
        6,
        Command::Handshake {
            catalog_sha256: SHA.to_owned(),
        },
    );
    frame.protocol_version = PROTOCOL_VERSION + 1;
    write_frame(wire.stdin.as_mut().unwrap(), &frame).unwrap();
    assert!(matches!(
        wire.reply().message,
        Reply::Error {
            key: None,
            error: IpcError::InvalidProtocolVersion
        }
    ));
    assert!(!wire.wait());
}

#[test]
fn true_worker_rejects_oversized_truncated_and_non_utf8_frames() {
    let directory = TestDir::new();
    for (payload, error) in [
        (
            ((MAX_FRAME_BYTES + 1) as u32).to_be_bytes().to_vec(),
            IpcError::FrameTooLarge,
        ),
        (vec![0, 0, 0, 2, b'{'], IpcError::TruncatedFrame),
        (vec![0, 0, 0, 1, 0xff], IpcError::InvalidUtf8),
        (vec![0, 0, 0, 2, b'{', b'}'], IpcError::InvalidMessage),
    ] {
        let mut wire = ChildWire::start(directory.0.join("missing.json"));
        wire.handshake(4);
        let mut stdin = wire.stdin.take().unwrap();
        stdin.write_all(&payload).unwrap();
        drop(stdin);
        let response = wire.reply();
        assert_eq!(response.worker_generation, 4);
        assert!(matches!(response.message, Reply::Error { key: None, error: e } if e == error));
        assert!(!wire.wait());
    }
}

#[test]
fn request_identity_is_generation_bound_and_a_completed_id_cannot_be_replayed() {
    let directory = TestDir::new();
    let mut wire = ChildWire::start(directory.0.join("missing.json"));
    wire.handshake(9);
    let mut key = RequestKey {
        generation: 8,
        source: SOURCE,
        request_id: 1,
    };
    wire.send(9, request(key));
    assert!(
        matches!(wire.reply().message, Reply::Error { key: Some(invalid), error: IpcError::InvalidIdentity } if invalid == key)
    );
    key.generation = 9;
    wire.send(9, request(key));
    assert!(
        matches!(wire.reply().message, Reply::Accepted { key: accepted, task_id: 1 } if accepted == key)
    );
    loop {
        let response = wire.reply().message;
        if let Reply::Error {
            key: Some(failed),
            error,
        } = response
        {
            assert_eq!(failed, key);
            assert_eq!(error, IpcError::ModelNotFound);
            break;
        }
        assert!(matches!(response, Reply::Progress { key: observed, .. } if observed == key));
    }
    wire.send(9, request(key));
    assert!(
        matches!(wire.reply().message, Reply::Error { key: Some(duplicate), error: IpcError::DuplicateRequest } if duplicate == key)
    );
    wire.send(9, Command::Shutdown {});
    assert!(wire.wait());
}

#[test]
fn client_is_lazy_bounded_cancelable_and_reaps_the_real_child() {
    let directory = TestDir::new();
    let mut client = client(
        directory.0.join("missing.json"),
        SHA.to_owned(),
        ResourceThreads::Two,
    );
    assert_eq!(client.state(), WorkerState::NotStarted);
    assert_eq!(client.child_pid(), None);
    client.start().unwrap();
    next_event(&mut client, |event| {
        *event == WorkerEvent::State(WorkerState::Ready)
    });
    assert!(client.child_pid().is_some());
    let timeout = Duration::from_secs(10);
    let first = client
        .submit(SOURCE, Direction::ZhEn, "你好".to_owned(), timeout)
        .unwrap();
    let second = client
        .submit(SOURCE, Direction::EnZh, "Hello".to_owned(), timeout)
        .unwrap();
    assert_eq!(
        client.submit(SOURCE, Direction::EnZh, "World".to_owned(), timeout),
        Err(IpcError::Busy)
    );
    assert!(client.cancel(second));
    let mut first_failed = false;
    let mut second_cancelled = false;
    for _ in 0..2 {
        match next_event(&mut client, |event| {
            matches!(
                event,
                WorkerEvent::Failed { .. } | WorkerEvent::Cancelled { .. }
            )
        }) {
            WorkerEvent::Failed { key, error } => {
                assert_eq!((key, error), (first, IpcError::ModelNotFound));
                first_failed = true;
            }
            WorkerEvent::Cancelled { key } => {
                assert_eq!(key, second);
                second_cancelled = true;
            }
            _ => unreachable!(),
        }
    }
    assert!(first_failed && second_cancelled);
    client.close();
    assert!(client.wait_for_shutdown(Duration::from_secs(5)));
    assert_eq!(client.child_pid(), None);
    assert_eq!(client.state(), WorkerState::Stopped);
}

#[test]
fn wrong_catalog_hash_is_a_typed_failure_and_no_result() {
    let directory = TestDir::new();
    let catalog = directory.0.join("catalog.json");
    fs::write(&catalog, "{}").unwrap();
    let mut client = client(catalog, SHA.to_owned(), ResourceThreads::One);
    let key = client
        .submit(SOURCE, Direction::ZhEn, "你好".to_owned(), WAIT)
        .unwrap();
    let event = next_event(&mut client, |event| {
        matches!(
            event,
            WorkerEvent::Failed { .. } | WorkerEvent::Completed { .. }
        )
    });
    assert_eq!(
        event,
        WorkerEvent::Failed {
            key,
            error: IpcError::ModelInvalid
        }
    );
    assert!(client.wait_for_shutdown(Duration::from_secs(5)));
}

#[test]
fn missing_sibling_cannot_fall_back_to_path_and_explicit_retry_gets_new_generation() {
    let directory = TestDir::new();
    let config = WorkerConfig::sibling(
        directory.0.join("model.json"),
        SHA.to_owned(),
        ResourceThreads::One,
    )
    .unwrap()
    .for_host_executable(directory.0.join("host"))
    .unwrap();
    let mut client = WorkerClient::new(config);
    let first = client
        .submit(SOURCE, Direction::ZhEn, "你好".to_owned(), WAIT)
        .unwrap();
    assert_eq!(
        next_event(&mut client, |event| matches!(
            event,
            WorkerEvent::Failed { .. }
        )),
        WorkerEvent::Failed {
            key: first,
            error: IpcError::InvalidWorkerPath
        },
    );
    next_event(&mut client, |event| {
        *event == WorkerEvent::State(WorkerState::Failed)
    });
    assert_eq!(client.child_pid(), None);
    let deadline = Instant::now() + WAIT;
    let second = loop {
        match client.submit(SOURCE, Direction::ZhEn, "你好".to_owned(), WAIT) {
            Ok(key) => break key,
            Err(IpcError::WorkerExited) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5))
            }
            other => panic!("显式重试异常：{other:?}"),
        }
    };
    assert!(second.generation > first.generation);
    assert!(client.wait_for_shutdown(Duration::from_secs(5)));
}

#[test]
fn only_an_owned_child_handle_can_force_an_exit() {
    let directory = TestDir::new();
    let mut wire = ChildWire::start(directory.0.join("missing.json"));
    wire.handshake(10);
    wire.child.kill().unwrap();
    assert!(!wire.wait());
    assert!(matches!(
        wire.replies.recv_timeout(WAIT).unwrap(),
        Ok(None) | Err(IpcError::BrokenPipe)
    ));
}

#[test]
#[ignore = "需环境变量提供已校验的本地 Marian 目录和 SHA-256；无权重 CI 单独记录"]
fn real_worker_translates_two_directions_and_reports_one_resident_model() {
    let catalog = PathBuf::from(std::env::var_os("CLOUDLEAF_TEST_CATALOG").unwrap());
    let sha = std::env::var("CLOUDLEAF_TEST_CATALOG_SHA256").unwrap();
    let mut client = client(catalog, sha.clone(), ResourceThreads::Two);
    let mut first_generation = 0;
    for (index, direction, text) in [
        (0, Direction::ZhEn, "你好。"),
        (1, Direction::ZhEn, "今天天气很好。"),
        (
            2,
            Direction::EnZh,
            "Please keep the original text until confirmation.",
        ),
    ] {
        let key = client
            .submit(SOURCE, direction, text.to_owned(), Duration::from_secs(110))
            .unwrap();
        if index == 0 {
            first_generation = key.generation;
        }
        assert_eq!(first_generation, key.generation);
        let event = next_event(&mut client, |event| {
            matches!(
                event,
                WorkerEvent::Completed { .. } | WorkerEvent::Failed { .. }
            )
        });
        let WorkerEvent::Completed {
            key: returned,
            output,
            ..
        } = event
        else {
            panic!("真实推理失败：{event:?}");
        };
        assert_eq!(returned, key);
        assert_eq!(output.catalog_sha256, sha);
        assert_eq!(output.direction, direction);
        assert_eq!(output.metrics.compute_threads, 2);
        assert_eq!(output.metrics.resident_models, 1);
        assert!(output.metrics.inference_started_unix_ns > 0);
        assert!(
            output.metrics.inference_finished_unix_ns > output.metrics.inference_started_unix_ns
        );
        if index == 1 {
            assert_eq!(output.metrics.verification_ns, 0);
            assert_eq!(output.metrics.load_ns, 0);
        } else {
            assert!(output.metrics.verification_ns > 0);
            assert!(output.metrics.load_ns > 0);
        }
        assert!(!output.raw_translation.trim().is_empty());
        if index == 0 {
            assert!(output.raw_translation.contains("Hello"));
        }
        if index == 2 {
            assert!(
                output
                    .raw_translation
                    .chars()
                    .any(|character| ('\u{4e00}'..='\u{9fff}').contains(&character))
            );
        }
    }
    let sentence = "Please preserve the source text, respect every punctuation mark, keep the original brand names, dates, order quantities, weights and measurements, and return the entire translation only after carefully reviewing the details.";
    let cancelled_task = client
        .submit(
            SOURCE,
            Direction::EnZh,
            sentence.to_owned(),
            Duration::from_secs(60),
        )
        .unwrap();
    next_event(&mut client, |event| {
        matches!(event, WorkerEvent::Progress {
        key, stage: ProgressStage::Inferring, ..
    } if *key == cancelled_task)
    });
    let cancelled_at = Instant::now();
    assert!(client.cancel(cancelled_task));
    let cancelled = next_event(&mut client, |event| {
        matches!(
            event,
            WorkerEvent::Cancelled { .. }
                | WorkerEvent::Completed { .. }
                | WorkerEvent::Failed { .. }
        )
    });
    assert_eq!(
        cancelled,
        WorkerEvent::Cancelled {
            key: cancelled_task
        }
    );
    assert!(cancelled_at.elapsed() < Duration::from_secs(2));
    assert!(client.child_pid().is_some());

    let expired_task = client
        .submit(
            SOURCE,
            Direction::EnZh,
            sentence.to_owned(),
            Duration::from_millis(1),
        )
        .unwrap();
    let expired = next_event(&mut client, |event| {
        matches!(
            event,
            WorkerEvent::Cancelled { .. }
                | WorkerEvent::Completed { .. }
                | WorkerEvent::Failed { .. }
        )
    });
    assert_eq!(
        expired,
        WorkerEvent::Failed {
            key: expired_task,
            error: IpcError::Timeout
        }
    );
    assert!(client.wait_for_shutdown(Duration::from_secs(5)));
    assert_eq!(client.child_pid(), None);
}
