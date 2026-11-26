//! 对固定按键轨迹和独立真实翻译进程采样，不启动桌面或 TSF。
use cloudleaf_input::{
    InputConfig, InputEngine, LanguageSnapshot, ResearchLanguageSnapshot, SearchSnapshot,
};
use cloudleaf_search::{Config, Method, Ranker, ResearchRanker};
use cloudleaf_translation_ipc::{
    Command, Direction, Envelope, IpcError, ProgressStage, RequestKey, ResourceThreads,
    SourceIdentity, WorkerClient, WorkerConfig, WorkerEvent, WorkerOutput, WorkerScheduling,
    WorkerState, read_frame, write_frame,
};
use cloudleaf_types::{InputEvent, Key};
use cloudleaf_user::Store;
use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

type Outcome<T> = Result<T, Box<dyn Error>>;
const TIMEOUT: Duration = Duration::from_secs(90);
const EN: &str = "I am developing a Chinese input method and I want to preserve the original input until the user confirms it. ";
const ZH: &str = "2026年11月2日，Cloudleaf 发布了第13轮研发计划，预算为123.45元。";

#[repr(C)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThread() -> isize;
    fn GetThreadTimes(
        thread: isize,
        creation: *mut FileTime,
        exit: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn GetCurrentProcessorNumber() -> u32;
    fn QueryThreadCycleTime(thread: isize, cycles: *mut u64) -> i32;
}

fn thread_probe() -> (u64, u32) {
    let [mut creation, mut exit, mut kernel, mut user] = Default::default();
    // 使用当前线程伪句柄。四块存储是独立且可写的 FILETIME。
    unsafe {
        assert_ne!(
            GetThreadTimes(
                GetCurrentThread(),
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user
            ),
            0
        );
        let ticks = |time: FileTime| (u64::from(time.high) << 32) | u64::from(time.low);
        (
            (ticks(kernel) + ticks(user)) * 100,
            GetCurrentProcessorNumber(),
        )
    }
}

fn thread_cycles() -> Option<u64> {
    let mut cycles = 0;
    // 当前线程伪句柄和有效的 u64 输出地址。保留周期数，不换算时间。
    unsafe { (QueryThreadCycleTime(GetCurrentThread(), &mut cycles) != 0).then_some(cycles) }
}

fn unix_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .try_into()
        .unwrap()
}

fn publish(line: std::fmt::Arguments<'_>) {
    println!("{line}");
    io::stdout().flush().unwrap();
}

fn trace(text: &str) -> Vec<InputEvent> {
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .flat_map(|line| {
            line.split_once('\t')
                .unwrap()
                .1
                .split_ascii_whitespace()
                .flat_map(|token| match token {
                    "<Blur>" => vec![InputEvent::FocusLost],
                    "<Left>" => vec![InputEvent::Key(Key::Left)],
                    "<Right>" => vec![InputEvent::Key(Key::Right)],
                    "<Space>" => vec![InputEvent::Key(Key::Space)],
                    "<Enter>" => vec![InputEvent::Key(Key::Enter)],
                    "<Esc>" => vec![InputEvent::Key(Key::Escape)],
                    "<Backspace>" => vec![InputEvent::Key(Key::Backspace)],
                    "<Down>" => vec![InputEvent::Key(Key::Down)],
                    "<PageDown>" => vec![InputEvent::Key(Key::PageDown)],
                    _ => token
                        .chars()
                        .map(|ch| InputEvent::Key(Key::Char(ch)))
                        .collect(),
                })
        })
        .collect()
}

fn next_event(worker: &mut WorkerClient) -> Outcome<WorkerEvent> {
    let limit = Instant::now() + TIMEOUT;
    loop {
        if let Some(event) = worker.try_event() {
            return Ok(event);
        }
        if Instant::now() >= limit {
            return Err("等待工作进程事件超时".into());
        }
        thread::sleep(Duration::from_micros(400));
    }
}

fn wait_stage(worker: &mut WorkerClient, task: RequestKey, target: ProgressStage) -> Outcome<()> {
    loop {
        match next_event(worker)? {
            WorkerEvent::Progress { key, stage, .. } if key == task => {
                publish(format_args!("STAGE\t{stage:?}\t{}", unix_ns()));
                if stage == target {
                    return Ok(());
                }
            }
            WorkerEvent::Failed { error, .. } => return Err(error.into()),
            WorkerEvent::Completed { .. } | WorkerEvent::Cancelled { .. } => {
                return Err("任务结束前未观察到预期阶段".into());
            }
            _ => {}
        }
    }
}

fn terminal(
    worker: &mut WorkerClient,
    task: RequestKey,
    cancelled: bool,
) -> Outcome<Option<WorkerOutput>> {
    loop {
        match next_event(worker)? {
            WorkerEvent::Completed { key, output, .. } if key == task && !cancelled => {
                return Ok(Some(output));
            }
            WorkerEvent::Cancelled { key, .. } if key == task && cancelled => return Ok(None),
            WorkerEvent::Failed { error, .. } => return Err(error.into()),
            WorkerEvent::Completed { .. } | WorkerEvent::Cancelled { .. } => {
                return Err("工作进程任务身份或最终状态不符".into());
            }
            _ => {}
        }
    }
}

fn submit(
    worker: &mut WorkerClient,
    sequence: u64,
    direction: Direction,
    text: &str,
) -> Outcome<(RequestKey, Instant)> {
    let start = Instant::now();
    let key = worker.submit(
        SourceIdentity {
            session: 1,
            epoch: 1,
            input_revision: sequence,
        },
        direction,
        text.to_owned(),
        TIMEOUT,
    )?;
    Ok((key, start))
}

fn report_model(tag: &str, output: &WorkerOutput, elapsed: Duration, configured_threads: usize) {
    let m = &output.metrics;
    assert_eq!(m.compute_threads, configured_threads);
    assert_eq!(m.resident_models, 1);
    publish(format_args!(
        "MODEL\t{tag}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}",
        m.compute_threads,
        m.resident_models,
        m.verification_ns,
        m.load_ns,
        m.inference_ns,
        m.encoder_ns,
        m.decoder_ns,
        m.generated_steps,
        m.queue_wait_ns,
        m.inference_started_unix_ns,
        m.inference_finished_unix_ns,
        m.loading_started_unix_ns,
        m.loading_started_unix_ns.saturating_add(m.load_ns),
        elapsed.as_nanos(),
        output.model_identity,
        output.raw_translation,
    ));
}

fn codec() -> Outcome<()> {
    let source = SourceIdentity {
        session: 1,
        epoch: 1,
        input_revision: 1,
    };
    let env = Envelope::new(
        1,
        Command::Request {
            key: RequestKey {
                generation: 1,
                request_id: 1,
                source,
            },
            direction: Direction::EnZh,
            text: EN.repeat(4),
            timeout_ms: 90_000,
        },
    );
    for index in 0..200 {
        let start = Instant::now();
        let mut bytes = Vec::with_capacity(1024);
        write_frame(&mut bytes, &env)?;
        let decoded = read_frame::<Envelope<Command>>(bytes.as_slice())?.unwrap();
        let elapsed = start.elapsed();
        assert_eq!(decoded.worker_generation, 1);
        println!("CODEC\t{index}\t{}\t{}", bytes.len(), elapsed.as_nanos());
    }
    Ok(())
}

fn main() -> Outcome<()> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let gate = if args.len() >= 2 && args[args.len() - 2] == "--replay-gate" {
        let option = args.split_off(args.len() - 2);
        Some(PathBuf::from(&option[1]))
    } else {
        None
    };
    let research = if args.len() == 12 {
        Some(args.split_off(10))
    } else {
        None
    };
    let [
        runtime,
        runtime_sha,
        language,
        language_sha,
        catalog,
        catalog_sha,
        events,
        user_path,
        scenario,
        threads,
    ] = args.as_slice()
    else {
        return Err(
            "usage: runtime sha language sha catalog sha events user-dir scenario threads".into(),
        );
    };
    let (thread_option, scheduling) = if let Some(raw) = threads.strip_suffix("-below") {
        (raw, WorkerScheduling::BelowNormal)
    } else {
        (threads.as_str(), WorkerScheduling::Normal)
    };
    let lexicon = Arc::new(SearchSnapshot::from_verified_tsv(
        &std::fs::read_to_string(runtime)?,
        runtime_sha,
    )?);
    let signal = Arc::new(LanguageSnapshot::from_verified_tsv(
        &std::fs::read_to_string(language)?,
        language_sha,
        &lexicon,
    )?);
    let events = trace(&std::fs::read_to_string(events)?);
    if gate.is_some() {
        assert!((1..=1024).contains(&events.len()));
    } else {
        assert_eq!(events.len(), 171);
    }
    let config = Config {
        method: Method::PositionDag,
        ranker: Ranker::Lexical,
        output_k: 10,
        max_input_bytes: 64,
        max_letters: 64,
        max_expansions: 200_000,
        max_resident_paths: 50_000,
        ..Default::default()
    };
    let load_start = Instant::now();
    let mut engine = if let Some(args) = research {
        let model = Arc::new(ResearchLanguageSnapshot::from_verified_tsv(
            &std::fs::read_to_string(&args[0])?,
            &args[1],
            &lexicon,
            None,
            ResearchRanker::Word,
            50,
        )?);
        println!(
            "INPUT_MODEL\tword\t{}\t{}",
            model.estimated_bytes(),
            load_start.elapsed().as_nanos()
        );
        InputEngine::with_research_context(
            lexicon,
            Config {
                ranker: Ranker::Structural,
                ..config
            },
            model,
            InputConfig::default(),
        )?
    } else {
        println!(
            "INPUT_MODEL\tlexical\t{}\t{}",
            signal.estimated_bytes(),
            load_start.elapsed().as_nanos()
        );
        InputEngine::with_language(lexicon, config, signal, None, InputConfig::default())?
    };
    engine.handle(InputEvent::Activate);

    let started = Instant::now();
    let conf = || {
        let config = if thread_option == "auto" {
            WorkerConfig::sibling_auto(PathBuf::from(catalog), catalog_sha.to_owned())
        } else {
            let value = ResourceThreads::try_from(
                thread_option
                    .parse::<usize>()
                    .map_err(|_| IpcError::InvalidConfiguration)?,
            )?;
            WorkerConfig::sibling(PathBuf::from(catalog), catalog_sha.to_owned(), value)
        }?;
        config.with_scheduling(scheduling)
    };
    let threads = conf()?.compute_threads();
    let mut worker = WorkerClient::new(conf()?);
    if scenario != "idle" {
        worker.start()?;
        while worker.state() != WorkerState::Ready {
            if started.elapsed() >= TIMEOUT {
                return Err("握手未就绪".into());
            }
            if let Some(WorkerEvent::Failed { error, .. }) = worker.try_event() {
                return Err(error.into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        publish(format_args!(
            "READY\t{}\t{}",
            worker.child_pid().ok_or("工作进程身份不可用")?,
            started.elapsed().as_nanos()
        ));
    }
    if let Some(path) = gate {
        publish(format_args!(
            "RESOURCE\t{}\t{scheduling:?}",
            threads.count()
        ));
        let deadline = Instant::now() + Duration::from_secs(15);
        while !path.is_file() {
            if Instant::now() >= deadline {
                return Err("等待探针资源配置超时".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    let english = EN.repeat(4);
    let mut first_model = None;
    if matches!(
        scenario.as_str(),
        "direction-switch" | "repeat-release" | "warm"
    ) {
        let (task, start) = submit(&mut worker, 1, Direction::EnZh, &english)?;
        let output = terminal(&mut worker, task, false)?.unwrap();
        report_model("prime", &output, start.elapsed(), threads.count());
        first_model = Some(output.model_identity);
    }
    if matches!(scenario.as_str(), "after-exit" | "repeat-release") {
        let stop = Instant::now();
        worker.close();
        if !worker.wait_for_shutdown(Duration::from_secs(5)) {
            return Err("工作进程未按时退出".into());
        }
        publish(format_args!("STOP\t{}", stop.elapsed().as_nanos()));
        if scenario == "repeat-release" {
            worker = WorkerClient::new(conf()?);
        }
    }
    let active = if matches!(scenario.as_str(), "idle" | "ready" | "after-exit") {
        None
    } else {
        let direction = if scenario == "direction-switch" {
            Direction::ZhEn
        } else {
            Direction::EnZh
        };
        let text = if direction == Direction::ZhEn {
            ZH
        } else {
            &english
        };
        let task = submit(&mut worker, 2, direction, text)?;
        let wanted = if scenario == "loading" {
            ProgressStage::Loading
        } else {
            ProgressStage::Inferring
        };
        wait_stage(&mut worker, task.0, wanted)?;
        if let Some(pid) = worker.child_pid() {
            publish(format_args!("WORKER\t{pid}"));
        }
        Some(task)
    };

    let mut queued = None;
    if scenario == "full" {
        queued = Some(submit(&mut worker, 3, Direction::EnZh, "Hello, world.")?.0);
        assert!(matches!(
            submit(&mut worker, 4, Direction::EnZh, "Hello again.")
                .unwrap_err()
                .downcast_ref::<IpcError>(),
            Some(IpcError::Busy)
        ));
    }

    let saving = Arc::new(AtomicBool::new(false));
    let user = if scenario == "store" {
        let busy = saving.clone();
        let directory = PathBuf::from(user_path);
        let (begun, ready) = mpsc::sync_channel(1);
        let handle = thread::spawn(move || {
            let mut store = Store::open(directory).unwrap();
            let mut dict = store.load().unwrap();
            dict.add("云笺", "yun jian").unwrap();
            busy.store(true, Ordering::Release);
            begun.send(()).unwrap();
            for _ in 0..32 {
                dict = store.save(&dict, dict.generation()).unwrap();
            }
            busy.store(false, Ordering::Release);
        });
        ready.recv_timeout(Duration::from_secs(5))?;
        Some(handle)
    } else {
        None
    };

    let mut observations = Vec::new();
    let mut cancel_at = None;
    for (index, event) in events.into_iter().enumerate() {
        if matches!(scenario.as_str(), "cancel" | "cancel-next") && index == 10 {
            assert!(worker.cancel(active.as_ref().unwrap().0));
            cancel_at = Some(Instant::now());
        }
        let (cpu0, core0) = thread_probe();
        let save = saving.load(Ordering::Acquire);
        let t0 = unix_ns();
        let start = Instant::now();
        let cycle0 = thread_cycles();
        let result = engine.handle(event);
        let cycle1 = thread_cycles();
        let wall_ns = start.elapsed().as_nanos();
        let t1 = unix_ns();
        let (cpu1, core1) = thread_probe();
        let store_overlap = save && saving.load(Ordering::Acquire);
        if let Some(commit) = &result.commit {
            engine.handle(InputEvent::CommitSucceeded { id: commit.id });
        }
        observations.push((
            index,
            t0,
            t1,
            wall_ns,
            cpu1 - cpu0,
            core0,
            core1,
            store_overlap,
            cycle0
                .zip(cycle1)
                .and_then(|(before, after)| after.checked_sub(before)),
            result,
        ));
        if matches!(scenario.as_str(), "cancel" | "cancel-next")
            && let Some(sent) = cancel_at
            && let Some(event) = worker.try_event()
        {
            match event {
                WorkerEvent::Cancelled { key, .. } if key == active.as_ref().unwrap().0 => {
                    publish(format_args!("CANCEL\t{}", sent.elapsed().as_nanos()));
                    cancel_at = None;
                }
                WorkerEvent::Completed { .. } => return Err("取消任务返回了译文".into()),
                _ => {}
            }
        }
    }
    if let Some((task, asked)) = active {
        if !matches!(scenario.as_str(), "cancel" | "cancel-next") {
            let output = terminal(&mut worker, task, false)?.unwrap();
            if let Some(prime) = first_model {
                assert_eq!(
                    output.model_identity == prime,
                    matches!(scenario.as_str(), "repeat-release" | "warm")
                );
            }
            if scenario == "warm" {
                assert_eq!(output.metrics.load_ns, 0);
                assert_eq!(output.metrics.verification_ns, 0);
            }
            report_model("main", &output, asked.elapsed(), threads.count());
        } else if let Some(start) = cancel_at {
            assert!(terminal(&mut worker, task, true)?.is_none());
            publish(format_args!("CANCEL\t{}", start.elapsed().as_nanos()));
        }
    }
    if let Some(task) = queued {
        assert!(terminal(&mut worker, task, false)?.is_some());
    }
    if scenario == "cancel-next" {
        let (next, began) = submit(&mut worker, 3, Direction::EnZh, &english)?;
        let output = terminal(&mut worker, next, false)?.unwrap();
        assert_eq!(output.metrics.load_ns, 0);
        report_model("recovered", &output, began.elapsed(), threads.count());
    }
    if let Some(user) = user {
        user.join().map_err(|_| "合成词库写线程异常退出")?;
        let mut reopened = Store::open(PathBuf::from(user_path))?;
        assert_eq!(reopened.load()?.generation(), 32);
    }
    for (i, t0, t1, wall, cpu, core0, core1, store, cycles, result) in observations {
        println!("KEY\t{i}\t{t0}\t{t1}\t{wall}\t{cpu}\t{core0}\t{core1}\t{store}");
        println!("RESULT\t{i}\t{result:?}");
        if let Some(cycles) = cycles {
            println!("CYCLE\t{i}\t{cycles}");
        }
    }
    if !matches!(scenario.as_str(), "idle" | "after-exit") {
        let stop = Instant::now();
        worker.close();
        if !worker.wait_for_shutdown(Duration::from_secs(5)) {
            return Err("工作进程未按时退出".into());
        }
        publish(format_args!("STOP\t{}", stop.elapsed().as_nanos()));
    }
    if scenario == "ready" {
        codec()?;
    }
    Ok(())
}
