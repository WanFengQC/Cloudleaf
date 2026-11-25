//! 无窗口观测：当前构建的输入、真实 Marian、服务队列、合成词库。
use cloudleaf_input::{InputConfig, InputEngine, LanguageSnapshot, SearchSnapshot};
use cloudleaf_offline::{MarianProvider, sha256_file};
use cloudleaf_search::{Config, Method, Ranker};
use cloudleaf_translation::*;
use cloudleaf_types::{InputEvent, Key};
use cloudleaf_user::Store;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant};

type Outcome<T> = Result<T, Box<dyn Error>>;
const PROVIDER: &str = "offline-model";
const TEXT: &str = "I am developing a Chinese input method and I want to preserve the original input until the user confirms it. ";

struct Observed {
    catalog: PathBuf,
    sha: String,
    provider: Mutex<Option<MarianProvider>>,
    stage: AtomicU8,
}

impl TranslationProvider for Observed {
    fn identity(&self) -> ProviderIdentity {
        ProviderIdentity {
            id: PROVIDER.to_owned(),
            version: self.sha.clone(),
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            directions: vec![TranslationDirection {
                source: Language::English,
                target: Language::Chinese,
            }],
            kinds: vec![TranslationKind::Sentence],
            auto_source: false,
            network: false,
            available: true,
        }
    }

    fn translate(
        &self,
        req: &TranslationRequest,
        cancel: &TranslationCancellation,
    ) -> Result<ProviderOutput, TranslationError> {
        cancel.check()?;
        let mut provider = self
            .provider
            .try_lock()
            .map_err(|_| TranslationError::Busy)?;
        let outcome = (|| {
            if provider.is_none() {
                self.stage.store(1, Ordering::Release);
                let start = Instant::now();
                if sha256_file(&self.catalog)? != self.sha {
                    return Err(TranslationError::ModelLoadFailed);
                }
                *provider = Some(MarianProvider::load(&self.catalog)?);
                println!("MODEL_LOAD\t{}", start.elapsed().as_nanos());
            }
            cancel.check()?;
            self.stage.store(2, Ordering::Release);
            let start = Instant::now();
            let result = provider.as_ref().unwrap().translate(req, cancel);
            println!(
                "MODEL_INFER\t{}\t{:?}",
                start.elapsed().as_nanos(),
                result.as_ref().err()
            );
            result
        })();
        self.stage.store(0, Ordering::Release);
        outcome
    }
}

fn request(text: &str, rev: u64) -> TranslationRequest {
    TranslationRequest {
        text: text.to_owned(),
        direction: TranslationDirection {
            source: Language::English,
            target: Language::Chinese,
        },
        kind: TranslationKind::Sentence,
        context: TranslationContext {
            session: 1,
            epoch: 1,
            input_revision: rev,
        },
        send_authorized_to: None,
        use_cache: false,
    }
}

fn wait_stage(model: &Observed, stage: u8) {
    let start = Instant::now();
    while model.stage.load(Ordering::Acquire) != stage {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "worker stage timed out"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

fn terminal(service: &mut TranslationService, task: TranslationTaskId) -> Outcome<TaskSnapshot> {
    let start = Instant::now();
    loop {
        let snapshot = service.poll(task)?;
        if snapshot.state.is_terminal() {
            return Ok(snapshot);
        }
        if start.elapsed() > Duration::from_secs(120) {
            return Err("task timed out".into());
        }
        thread::sleep(Duration::from_millis(2));
    }
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

fn main() -> Outcome<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 8 {
        return Err(
            "usage: runtime sha language-model sha catalog sha events synthetic-user-root".into(),
        );
    }
    let lexicon = Arc::new(SearchSnapshot::from_verified_tsv(
        &std::fs::read_to_string(&args[0])?,
        &args[1],
    )?);
    let signal = Arc::new(LanguageSnapshot::from_verified_tsv(
        &std::fs::read_to_string(&args[2])?,
        &args[3],
        &lexicon,
    )?);
    let events = trace(&std::fs::read_to_string(&args[6])?);
    assert_eq!(events.len(), 171);
    let model = Arc::new(Observed {
        catalog: PathBuf::from(&args[4]),
        sha: args[5].clone(),
        provider: Mutex::new(None),
        stage: AtomicU8::new(0),
    });
    let mut registry = ProviderRegistry::new();
    registry.register(
        model.clone(),
        TranslationProviderConfig {
            timeout: Duration::from_secs(120),
            allow_network: false,
            allow_cache: false,
            read_policy: ReadPolicy::Research,
            ..TranslationProviderConfig::default()
        },
    )?;
    let mut service = TranslationService::new(
        registry,
        ServiceLimits {
            workers: 1,
            queued: 1,
            offline_only: true,
            cache_entries: 0,
            cache_bytes: 0,
            ..ServiceLimits::default()
        },
    )?;
    let user_root = Path::new(&args[7]);
    let user_busy = Arc::new(AtomicBool::new(false));
    let (user_start, user_tasks) = mpsc::sync_channel(1);
    let (user_done, user_results) = mpsc::sync_channel(1);
    let user = {
        let user_root = user_root.to_owned();
        let status = user_busy.clone();
        thread::spawn(move || {
            let mut store = Store::open(user_root).unwrap();
            let mut dictionary = store.load().unwrap();
            dictionary.add("云笺", "yun jian").unwrap();
            while user_tasks.recv().is_ok() {
                status.store(true, Ordering::Release);
                for _ in 0..32 {
                    dictionary = store.save(&dictionary, dictionary.generation()).unwrap();
                }
                status.store(false, Ordering::Release);
                user_done.send(dictionary.generation()).unwrap();
            }
        })
    };
    let text = TEXT.repeat(4);
    let mut revision = 1;
    let mut rows = Vec::new();
    for scope in [
        "idle-cold",
        "loading",
        "infer",
        "queue-full",
        "cancel",
        "user-store-coexist",
        "idle-warm",
    ] {
        let task = if matches!(scope, "idle-cold" | "idle-warm") {
            None
        } else {
            revision += 1;
            let task = service.submit(PROVIDER, request(&text, revision))?;
            wait_stage(&model, if scope == "loading" { 1 } else { 2 });
            Some(task)
        };
        let mut queued = None;
        if scope == "queue-full" {
            revision += 1;
            queued = Some(service.submit(PROVIDER, request("Hello, world.", revision))?);
            revision += 1;
            assert_eq!(
                service.submit(PROVIDER, request("Hello again.", revision)),
                Err(TranslationError::Busy)
            );
        }
        if scope == "user-store-coexist" {
            user_start.send(())?;
            let start = Instant::now();
            while !user_busy.load(Ordering::Acquire) {
                assert!(start.elapsed() < Duration::from_secs(10));
                thread::sleep(Duration::from_millis(1));
            }
        }
        let mut engine = InputEngine::with_language(
            lexicon.clone(),
            Config {
                method: Method::PositionDag,
                ranker: Ranker::Lexical,
                output_k: 10,
                max_input_bytes: 64,
                max_letters: 64,
                ..Default::default()
            },
            signal.clone(),
            None,
            InputConfig::default(),
        )?;
        engine.handle(InputEvent::Activate);
        let mut semantic = Vec::new();
        for (i, event) in events.iter().copied().enumerate() {
            if scope == "cancel" && i == 10 {
                service.cancel(task.unwrap())?;
            }
            let stage_before = model.stage.load(Ordering::Acquire);
            let user_before = user_busy.load(Ordering::Acquire);
            let start = Instant::now();
            let result = engine.handle(event);
            let elapsed = start.elapsed().as_nanos();
            rows.push(format!(
                "EVENT\t{scope}\t{i}\t{elapsed}\t{stage_before}\t{user_before}"
            ));
            if let Some(commit) = &result.commit {
                engine.handle(InputEvent::CommitSucceeded { id: commit.id });
            }
            semantic.push(result);
        }
        if let Some(task) = task {
            let snapshot = terminal(&mut service, task)?;
            if scope == "cancel" {
                assert_eq!(snapshot.state, TaskState::Cancelled);
                assert!(snapshot.result.is_none());
                let start = Instant::now();
                while model.stage.load(Ordering::Acquire) != 0 {
                    assert!(start.elapsed() < Duration::from_secs(30));
                    thread::sleep(Duration::from_millis(1));
                }
            } else {
                assert_eq!(
                    snapshot.state,
                    TaskState::Succeeded,
                    "{scope}: {:?}",
                    snapshot.error
                );
            }
            service.forget(task)?;
            println!("TASK\t{scope}\t{:?}", snapshot.state);
        }
        if let Some(queued) = queued {
            assert_eq!(terminal(&mut service, queued)?.state, TaskState::Succeeded);
            service.forget(queued)?;
        }
        if scope == "user-store-coexist" {
            assert_eq!(user_results.recv_timeout(Duration::from_secs(30))?, 32);
        }
        for (i, result) in semantic.into_iter().enumerate() {
            println!("RESULT\t{scope}\t{i}\t{result:?}");
        }
    }
    for row in rows {
        println!("{row}");
    }
    drop(user_start);
    user.join().map_err(|_| "synthetic user worker panicked")?;
    let mut reopen = Store::open(user_root.to_owned())?;
    assert_eq!(reopen.load()?.generation(), 32);
    service.shutdown()?;
    assert!(Arc::strong_count(&model) == 1);
    assert_eq!(model.stage.load(Ordering::Acquire), 0);
    drop(model);
    println!("END\tuser_lock_reopened\tmodel_released\ttranslation_worker_joined");
    Ok(())
}
