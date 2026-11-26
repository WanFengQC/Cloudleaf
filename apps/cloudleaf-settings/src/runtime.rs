use crate::{
    InputPreferences, InputPreferencesRepository, InputSettingsError, LoadedInputPreferences,
    read_bounded, write_atomic,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RECEIPTS: &str = "input-runtime";
const RECEIPT_BYTES: usize = 4096;
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const HEARTBEAT: Duration = Duration::from_secs(2);
const ACTIVE_FOR_MS: u64 = 10_000;
static NEXT_HOST: AtomicU64 = AtomicU64::new(1);

type SettingsUpdate = Result<LoadedInputPreferences, InputSettingsError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSettingsStatus {
    Applied,
    Deferred,
    Rejected,
}

impl RuntimeSettingsStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Applied => "已应用",
            Self::Deferred => "等待安全应用",
            Self::Rejected => "保留当前配置",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSettingsReason {
    CompositionOrCommit,
    RecoveredFile,
    InvalidFile,
    StaleRevision,
    IncompatibleHost,
}

impl RuntimeSettingsReason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CompositionOrCommit => "组合或提交事务仍需收尾",
            Self::RecoveredFile => "主配置缺失或损坏",
            Self::InvalidFile => "无法验证当前主配置",
            Self::StaleRevision => "配置修订退回或同修订内容不同",
            Self::IncompatibleHost => "输入宿主无法应用此配置",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSettingsReceipt {
    pub host_id: String,
    pub process_id: u32,
    pub generation: u64,
    pub active: InputPreferences,
    pub dictionary_version: Option<String>,
    pub pending_revision: Option<u64>,
    pub status: RuntimeSettingsStatus,
    pub reason: Option<RuntimeSettingsReason>,
    updated_at_ms: u64,
}

#[derive(Default)]
struct Mailboxes {
    stop: AtomicBool,
    incoming: Mutex<Option<SettingsUpdate>>,
    outgoing: Mutex<Option<RuntimeSettingsReceipt>>,
}

pub struct RuntimeSettingsWatch {
    mailboxes: Arc<Mailboxes>,
    worker: thread::Thread,
    host_id: String,
}

impl RuntimeSettingsWatch {
    pub fn start(directory: PathBuf) -> io::Result<Self> {
        let mailboxes = Arc::new(Mailboxes::default());
        let host_id = format!(
            "{}-{}-{}",
            std::process::id(),
            now_ms(),
            NEXT_HOST.fetch_add(1, Ordering::Relaxed)
        );
        let state = Arc::clone(&mailboxes);
        let worker_host = host_id.clone();
        let worker = thread::Builder::new()
            .name("cloudleaf-input-settings".into())
            .spawn(move || watch(directory, &worker_host, state))?;
        Ok(Self {
            mailboxes,
            worker: worker.thread().clone(),
            host_id,
        })
    }

    pub fn poll(&self) -> Option<SettingsUpdate> {
        self.mailboxes.incoming.try_lock().ok()?.take()
    }

    /// 忙时由宿主保留回执并在下次轮询重试，窗口线程无需等待后台。
    pub fn publish(
        &self,
        generation: u64,
        active: &InputPreferences,
        dictionary_version: Option<&str>,
        pending_revision: Option<u64>,
        status: RuntimeSettingsStatus,
        reason: Option<RuntimeSettingsReason>,
    ) -> bool {
        let Ok(mut slot) = self.mailboxes.outgoing.try_lock() else {
            return false;
        };
        *slot = Some(RuntimeSettingsReceipt {
            host_id: self.host_id.clone(),
            process_id: std::process::id(),
            generation,
            active: active.clone(),
            dictionary_version: dictionary_version.map(str::to_owned),
            pending_revision,
            status,
            reason,
            updated_at_ms: 0,
        });
        self.worker.unpark();
        true
    }
}

impl Drop for RuntimeSettingsWatch {
    fn drop(&mut self) {
        self.mailboxes.stop.store(true, Ordering::Relaxed);
        self.worker.unpark();
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn watch(directory: PathBuf, host_id: &str, mailboxes: Arc<Mailboxes>) {
    let path = directory.join(RECEIPTS).join(format!("{host_id}.json"));
    let repository = InputPreferencesRepository::new(directory);
    let mut last_seen = None;
    let mut receipt = None::<RuntimeSettingsReceipt>;
    let mut last_written = None::<Instant>;
    while !mailboxes.stop.load(Ordering::Relaxed) {
        let current = repository.load();
        if last_seen.as_ref() != Some(&current) {
            *mailboxes.incoming.lock().expect("输入设置邮箱被污染") = Some(current.clone());
            last_seen = Some(current);
        }
        let newer = mailboxes
            .outgoing
            .lock()
            .expect("输入设置回执被污染")
            .take();
        if let Some(newer) = newer {
            receipt = Some(newer);
            last_written = None;
        }
        if let Some(current) = receipt.as_mut()
            && last_written.is_none_or(|last| last.elapsed() >= HEARTBEAT)
        {
            current.updated_at_ms = now_ms();
            if let Ok(bytes) = serde_json::to_vec(current)
                && fs::create_dir_all(path.parent().expect("回执必须有父目录"))
                    .and_then(|()| write_atomic(&path, &bytes))
                    .is_ok()
            {
                last_written = Some(Instant::now());
            }
        }
        thread::park_timeout(POLL_INTERVAL);
    }
    let _ = fs::remove_file(path);
}

/// 只返回仍在发送心跳的宿主，忽略崩溃留下或正在清理的旧回执。
pub fn read_runtime_receipts(directory: &Path) -> io::Result<Vec<RuntimeSettingsReceipt>> {
    let entries = match fs::read_dir(directory.join(RECEIPTS)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let now = now_ms();
    let mut receipts = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = read_bounded(&path, RECEIPT_BYTES) else {
            continue;
        };
        if bytes.len() <= RECEIPT_BYTES
            && let Ok(receipt) = serde_json::from_slice::<RuntimeSettingsReceipt>(&bytes)
            && receipt.updated_at_ms <= now
            && now - receipt.updated_at_ms <= ACTIVE_FOR_MS
            && Some(receipt.host_id.as_str()) == path.file_stem().and_then(|value| value.to_str())
            && receipt.active.validate().is_ok()
        {
            receipts.push(receipt);
        }
    }
    receipts.sort_by(|left, right| left.host_id.cmp(&right.host_id));
    Ok(receipts)
}
