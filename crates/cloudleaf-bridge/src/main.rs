//! 无窗口研发宿主。输入来自显式 JSONL，提交目标只存在于本进程内存。

use cloudleaf_bridge::{BridgeError, PendingCommit, TranslationBridge, TranslationState};
use cloudleaf_input::{InputConfig, InputEngine, SearchSnapshot};
use cloudleaf_search::{Config, Method, Ranker};
use cloudleaf_translation::{WorkerObserver, WorkerProvider};
use cloudleaf_translation_ipc::{ResourceThreads, WorkerConfig};
use cloudleaf_types::{CommitOutcome, EventResult, InputEvent, InputMode, Key};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead},
    path::PathBuf,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

struct Headless {
    bridge: TranslationBridge,
    document: String,
    attempt: Option<PendingCommit>,
    show_text: bool,
    worker: WorkerObserver,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let show_text = args.get(4).is_some_and(|arg| arg == "--show-text");
    if !(args.len() == 4 || args.len() == 5 && show_text) {
        return Err("usage: cloudleaf-bridge-headless RUNTIME_TSV LEXICON_SHA CATALOG_JSON CATALOG_SHA [--show-text] < actions.jsonl".into());
    }
    let dictionary = std::fs::read_to_string(&args[0])?;
    let snapshot = Arc::new(SearchSnapshot::from_verified_tsv(&dictionary, &args[1])?);
    let engine = InputEngine::with_search(
        snapshot,
        Config {
            method: Method::PositionDag,
            ranker: Ranker::Structural,
            output_k: 10,
            max_input_bytes: 64,
            max_letters: 64,
            ..Default::default()
        },
        InputConfig::default(),
    )?;
    let provider = Arc::new(WorkerProvider::new(WorkerConfig::sibling(
        PathBuf::from(&args[2]),
        args[3].clone(),
        ResourceThreads::One,
    )?));
    let worker = provider.observer();
    let bridge = TranslationBridge::new(engine, 1, provider, Duration::from_secs(120))?;
    let mut host = Headless {
        bridge,
        document: String::new(),
        attempt: None,
        show_text,
        worker,
    };
    for (index, line) in io::stdin().lock().lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if line.len() > 256 * 1024 {
            return Err("JSONL action exceeds 256 KiB".into());
        }
        let action: Value = serde_json::from_str(&line)?;
        let op = field(&action, "op")?;
        let start = Instant::now();
        let outcome = host.execute(op, &action);
        println!("{}", host.record(index, op, start.elapsed(), outcome));
    }
    // 此时输入流已关闭，允许等待取消生效后再释放后台线程与权重。
    host.bridge.shutdown()?;
    Ok(())
}

impl Headless {
    fn execute(&mut self, op: &str, action: &Value) -> Result<(), BridgeError> {
        match op {
            "activate" => self.event(InputEvent::Activate),
            "blur" => self.event(InputEvent::FocusLost),
            "chinese" => self.event(InputEvent::SetMode(InputMode::Chinese)),
            "english" => {
                let text = field(action, "text")?;
                self.event(InputEvent::SetMode(InputMode::English))?;
                self.bridge.replace_english(text)
            }
            "type" => {
                let text = field(action, "text")?;
                for ch in text.chars() {
                    self.event(InputEvent::Key(Key::Char(ch)))?;
                }
                Ok(())
            }
            "escape" => self.event(InputEvent::Key(Key::Escape)),
            "backspace" => self.event(InputEvent::Key(Key::Backspace)),
            "space" => self.event(InputEvent::Key(Key::Space)),
            "enter" => self.event(InputEvent::Key(Key::Enter)),
            "candidate" => {
                let frame = self.bridge.input().frame();
                let index = usize::try_from(action["index"].as_u64().unwrap_or(0))
                    .map_err(|_| BridgeError::InvalidState)?;
                let candidate = frame
                    .candidates
                    .get(index)
                    .ok_or(BridgeError::InvalidState)?;
                self.bridge
                    .request_candidate(frame.revision, &candidate.id)?;
                Ok(())
            }
            "request-english" => self.bridge.request_english().map(|_| ()),
            "poll" => {
                self.bridge.poll();
                Ok(())
            }
            "wait" => {
                let deadline = Instant::now() + Duration::from_secs(125);
                while matches!(
                    self.bridge.poll(),
                    TranslationState::Requested | TranslationState::Running
                ) {
                    if Instant::now() >= deadline {
                        return Err(cloudleaf_translation::TranslationError::Timeout.into());
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                Ok(())
            }
            "confirm" => {
                let result = self.bridge.result().ok_or(BridgeError::StaleConfirmation)?;
                self.attempt = Some(self.bridge.confirm(
                    result.context,
                    result.task_id,
                    result.result_version,
                )?);
                Ok(())
            }
            "retry" => {
                let old = self
                    .attempt
                    .as_ref()
                    .ok_or(BridgeError::StaleReceipt)?
                    .identity;
                self.attempt = Some(self.bridge.retry(old)?);
                Ok(())
            }
            "applied" => self.receipt(CommitOutcome::Applied),
            "not-applied" => self.receipt(CommitOutcome::NotApplied),
            "unknown" => self.receipt(CommitOutcome::Unknown),
            "cancel" => self.bridge.cancel(),
            "reset-unknown" => {
                let old = self
                    .attempt
                    .as_ref()
                    .ok_or(BridgeError::StaleReceipt)?
                    .identity;
                let session = action["session"]
                    .as_u64()
                    .ok_or(BridgeError::InvalidState)?;
                self.bridge.reset_after_unknown(old, session)?;
                self.attempt = None;
                Ok(())
            }
            _ => Err(BridgeError::InvalidState),
        }
    }

    fn event(&mut self, event: InputEvent) -> Result<(), BridgeError> {
        let EventResult { error, commit, .. } = self.bridge.apply_input(event)?;
        if let Some(error) = error {
            return Err(error.into());
        }
        if let Some(commit) = commit {
            if self.document.len() + commit.text.len() > 64 * 1024 {
                self.bridge
                    .apply_input(InputEvent::CommitFailed { id: commit.id })?;
                return Err(BridgeError::InvalidState);
            }
            self.document.push_str(&commit.text);
            let receipt = self
                .bridge
                .apply_input(InputEvent::CommitSucceeded { id: commit.id })?;
            if let Some(error) = receipt.error {
                return Err(error.into());
            }
        }
        Ok(())
    }

    fn receipt(&mut self, outcome: CommitOutcome) -> Result<(), BridgeError> {
        let pending = self.attempt.as_ref().ok_or(BridgeError::StaleReceipt)?;
        if self.bridge.state() != TranslationState::CommitPending {
            return Err(BridgeError::StaleReceipt);
        }
        if outcome == CommitOutcome::Applied
            && self.document.len() + pending.request.text.len() > 64 * 1024
        {
            return Err(BridgeError::InvalidState);
        }
        // 只写本进程的内存文档，未知注入不推断系统目标是否已写入。
        if outcome == CommitOutcome::Applied {
            self.document.push_str(&pending.request.text);
        }
        self.bridge.acknowledge(pending.identity, outcome)
    }

    fn record(
        &self,
        index: usize,
        op: &str,
        elapsed: Duration,
        outcome: Result<(), BridgeError>,
    ) -> Value {
        let frame = self.bridge.input().frame();
        let result = self.bridge.result();
        let original = self.bridge.original();
        let pending = self.attempt.as_ref();
        let worker = self.worker.snapshot();
        let integrity = self.bridge.integrity_report().map(|report| {
            json!({
                "review_required": report.requires_acknowledgement(),
                "findings": report.findings.iter().map(|item| json!({
                    "kind": item.kind.label(),
                    "status": item.status.label(),
                })).collect::<Vec<_>>(),
            })
        });
        json!({
            "index": index,
            "op": op,
            "state": format!("{:?}", self.bridge.state()),
            "revision": frame.revision,
            "candidate_count": frame.candidates.len(),
            "preedit_bytes": frame.preedit.len(),
            "elapsed_ns": elapsed.as_nanos() as u64,
            "error": outcome.err().map(|error| error.to_string()),
            "translation_failure": self.bridge.failure().map(|error| error.to_string()),
            "task": result.map(|record| record.task_id.0),
            "model_identity": result.map(|record| record.provider.version.as_str()),
            "quality": result.map(|record| format!("{:?}", record.output.quality)),
            "commit": pending.map(|attempt| attempt.identity.commit_id),
            "original_bytes": original.map(str::len),
            "output_bytes": result.map(|record| record.output.translated_text.len()),
            "original": if self.show_text { original } else { None },
            "translation": if self.show_text { result.map(|record| record.output.translated_text.as_str()) } else { None },
            "document": if self.show_text { Some(self.document.as_str()) } else { None },
            "document_bytes": self.document.len(),
            "learning_selection": self.bridge.input().pending_selection().is_some(),
            "target": "in-memory",
            "integrity": integrity,
            "worker": worker.map(|worker| json!({
                "stage": format!("{:?}", worker.stage),
                "pid": worker.child_pid,
                "request": worker.request,
                "worker_task": worker.task_id,
                "matches_result": original.zip(result).is_some_and(|(source, translated)| {
                    worker.matches_result(source, translated)
                }),
                "model_identity": worker.model_identity,
                "metrics": worker.metrics,
                "error": worker.error.map(|error| error.to_string()),
            })),
        })
    }
}

fn field<'a>(action: &'a Value, name: &str) -> Result<&'a str, BridgeError> {
    action[name].as_str().ok_or(BridgeError::InvalidState)
}
