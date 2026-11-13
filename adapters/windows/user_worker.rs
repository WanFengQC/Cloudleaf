//! 一个有界后台任务拥有私有文件锁；UI只派发命令和接收不可变快照。
use super::*;
use cloudleaf_input::{LanguageSnapshot, SearchSnapshot, SelectionEvidence};
use cloudleaf_user::{SelectionEvent, SelectionOrigin, Store, UserDictionary};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
};

pub(super) enum Work {
    Manage(user_panel::Command),
    Learn {
        event: SelectionEvent,
        generation: u64,
    },
}
pub(super) struct Prepared {
    pub snapshot: Arc<SearchSnapshot>,
    pub language: Option<Arc<LanguageSnapshot>>,
    pub listing: String,
    pub status: String,
    pub generation: u64,
    pub epoch: u64,
    pub learning: bool,
    pub _reservation: Arc<budget::Reservation>,
}
pub(super) enum Reply {
    Ready(Prepared),
    Error(String),
}
pub(super) struct Worker {
    tx: Option<SyncSender<(Work, u64)>>,
    rx: Receiver<Reply>,
    join: Option<JoinHandle<()>>,
    epoch: Arc<std::sync::atomic::AtomicU64>,
}
impl Worker {
    pub fn start(
        base: Arc<SearchSnapshot>,
        language: Option<Arc<LanguageSnapshot>>,
        path: PathBuf,
    ) -> Self {
        let (tx, commands) = mpsc::sync_channel(8);
        let (results, rx) = mpsc::sync_channel(2);
        let epoch = Arc::new(std::sync::atomic::AtomicU64::new(1));
        let live = epoch.clone();
        let join = std::thread::spawn(move || run(base, language, path, commands, results, live));
        Self {
            tx: Some(tx),
            rx,
            join: Some(join),
            epoch,
        }
    }
    pub fn send(&self, work: Work) -> std::result::Result<(), &'static str> {
        self.tx
            .as_ref()
            .unwrap()
            .try_send((work, self.epoch.load(std::sync::atomic::Ordering::Acquire)))
            .map_err(|_| "用户词库任务队列已满或工作线程退出；输入提交不会重试")
    }
    pub fn poll(&self) -> Option<Reply> {
        self.rx.try_recv().ok()
    }
    pub fn shutdown(&mut self) {
        self.cancel_learning();
        self.tx.take();
        if let Some(join) = self.join.take() {
            while !join.is_finished() {
                let _ = self.rx.recv_timeout(std::time::Duration::from_millis(20));
            }
            let _ = join.join();
        }
    }
    pub fn cancel_learning(&self) {
        self.epoch.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }
}

pub(super) fn learning_event(
    evidence: &SelectionEvidence,
    session: &str,
    epoch: u64,
) -> SelectionEvent {
    SelectionEvent {
        token: format!("{session}:{}", evidence.commit_id),
        origin: SelectionOrigin::IsolatedOwnedWindow,
        learning_epoch: epoch,
        entries: evidence.entries.clone(),
    }
}

fn prepare(
    base: &Arc<SearchSnapshot>,
    language: &Option<Arc<LanguageSnapshot>>,
    dictionary: &UserDictionary,
    status: String,
) -> std::result::Result<Prepared, String> {
    // 同时保留旧代和新代的额度；申请失败时不构建、不发布，也不改变输入引擎。
    let bytes = dictionary.to_bytes().len();
    let estimate = bytes.saturating_mul(8)
        + dictionary.entries().len() * 8192
        + base.entries().len() * 4
        + 512 * 1024
        + language
            .as_ref()
            .filter(|l| l.metadata().kind.starts_with("character_"))
            .map_or(0, |l| l.estimated_bytes());
    let reservation = Arc::new(
        budget::reserve_batch(1, estimate).ok_or("用户层构建超过现有128MiB容量，原快照保留")?,
    );
    let mut tsv = String::from(
        "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n",
    );
    for entry in dictionary.entries().values().filter(|e| e.enabled) {
        tsv.push_str(&format!(
            "{}\t{}\t{}\t\tunknown\t\tuser\t{}\tLicenseRef-User-Private\n",
            entry.id, entry.pinyin, entry.text, entry.id
        ));
    }
    let mut preferences = std::collections::BTreeMap::new();
    for ((id, fingerprint), count) in dictionary.preferences() {
        let actual = if id.starts_with("user:") {
            dictionary
                .get(id)
                .filter(|e| e.enabled)
                .map(|e| cloudleaf_user::fingerprint(&e.text, &e.pinyin))
        } else {
            base.entry_fingerprint(id)
        };
        let units = u32::from(cloudleaf_user::choice_units(*count));
        if units > 0 && actual.as_ref() == Some(fingerprint) {
            preferences.insert(id.clone(), units);
        }
    }
    let mut snapshot = base
        .with_user_layer(&tsv, &preferences)
        .map_err(|_| "用户层组合索引失败")?;
    snapshot
        .bind_user_state(&dictionary.to_bytes())
        .map_err(|_| "用户状态身份失败")?;
    let snapshot = Arc::new(snapshot);
    let language = language
        .as_ref()
        .map(|l| l.for_user_layer(base, &snapshot).map(Arc::new))
        .transpose()
        .map_err(|_| "模型用户层绑定失败")?;
    let actual = snapshot.incremental_estimated_bytes()
        + dictionary.estimated_bytes() * 2
        + language
            .as_ref()
            .filter(|l| l.metadata().kind.starts_with("character_"))
            .map_or(0, |l| l.estimated_bytes());
    if actual > estimate {
        return Err("用户层实际容量超过预留，未发布".into());
    }
    let mut listing = format!(
        "学习：{}；generation={}；词条={}；只在本地明文保存\r\n",
        if dictionary.learning_enabled() {
            "开"
        } else {
            "关"
        },
        dictionary.generation(),
        dictionary.entries().len()
    );
    for entry in dictionary.entries().values() {
        let count = dictionary.preference(
            &entry.id,
            &cloudleaf_user::fingerprint(&entry.text, &entry.pinyin),
        );
        listing.push_str(&format!(
            "{}\t{}\t{}\t{}\trev={}\t选择={}\r\n",
            entry.id,
            entry.text,
            entry.pinyin,
            if entry.enabled { "启用" } else { "禁用" },
            entry.revision,
            count
        ));
    }
    Ok(Prepared {
        snapshot,
        language,
        listing,
        status,
        generation: dictionary.generation(),
        epoch: dictionary.learning_epoch(),
        learning: dictionary.learning_enabled(),
        _reservation: reservation,
    })
}

fn run(
    base: Arc<SearchSnapshot>,
    language: Option<Arc<LanguageSnapshot>>,
    path: PathBuf,
    commands: Receiver<(Work, u64)>,
    results: SyncSender<Reply>,
    live: Arc<std::sync::atomic::AtomicU64>,
) {
    let mut store = match Store::open(path.clone()) {
        Ok(s) => s,
        Err(e) => {
            let _ = results.send(Reply::Error(e.to_string()));
            return;
        }
    };
    let mut state_reservation = match reserve_state(&path, None) {
        Ok(r) => r,
        Err(e) => {
            let _ = results.send(Reply::Error(e));
            return;
        }
    };
    let mut dictionary = match store.load() {
        Ok(d) => Some(d),
        Err(e) => {
            let candidates = store.recovery_candidates().unwrap_or_default();
            let _ = results.send(Reply::Error(format!(
                "{e}；可恢复代：{candidates:?}；基础输入仍可使用"
            )));
            None
        }
    };
    if let Some(d) = &dictionary {
        let reply = match prepare(&base, &language, d, "已加载；用户层待发布".into()) {
            Ok(p) => Reply::Ready(p),
            Err(e) => Reply::Error(e),
        };
        if results.send(reply).is_err() {
            return;
        }
    }
    while let Ok((work, epoch)) = commands.recv() {
        let result = (|| -> std::result::Result<Prepared, String> {
            let import = match &work {
                Work::Manage(user_panel::Command::Import { path }) => {
                    Some(std::path::Path::new(path))
                }
                _ => None,
            };
            let working = reserve_state(&path, import)?;
            if let Work::Learn { event, generation } = work {
                if live.load(std::sync::atomic::Ordering::Acquire) != epoch {
                    return Err("学习会话已失效，未记录".into());
                }
                let current = dictionary.as_ref().ok_or("损坏词库不接受学习")?;
                if generation != current.generation() {
                    return Err("学习事件属于旧词库版本，未记录".into());
                }
                let mut next = current.clone();
                if !next.record_selection(&event).map_err(|e| e.to_string())? {
                    return Err("学习关闭、重复或过期事件，未记录".into());
                }
                let preview = prepare(&base, &language, &next, "选择学习已保存".into())?;
                if live.load(std::sync::atomic::Ordering::Acquire) != epoch {
                    return Err("学习会话已失效，未保存".into());
                }
                let saved = store
                    .save(&next, current.generation())
                    .map_err(|e| e.to_string())?;
                let mut preview = preview;
                finalize(&mut preview, &saved)?;
                dictionary = Some(saved);
                state_reservation = working;
                return Ok(preview);
            }
            let Work::Manage(command) = work else {
                unreachable!()
            };
            use user_panel::Command::*;
            if matches!(command, Reload | Recover { .. } | ClearAll) {
                let next = match command {
                    Recover { generation } => store.recover(generation),
                    ClearAll => store.latest_generation().and_then(|g| store.clear_all(g)),
                    _ => store.load(),
                }
                .map_err(|e| e.to_string())?;
                let p = prepare(&base, &language, &next, "已加载/恢复；用户层待发布".into())?;
                dictionary = Some(next);
                state_reservation = working;
                return Ok(p);
            }
            let current = dictionary
                .as_ref()
                .ok_or("请先恢复损坏词库；没有将其当作空词库")?;
            let mut next = current.clone();
            let result = match command {
                Add { text, pinyin } => next.add(&text, &pinyin).map(|_| ()),
                Update { id, text, pinyin } => next.update(&id, &text, &pinyin),
                Delete { id } => next.remove(&id),
                Toggle { id } => {
                    let enabled = next.get(&id).ok_or("用户词条ID不存在")?.enabled;
                    next.set_enabled(&id, !enabled)
                }
                ClearEntries => next.clear_entries(),
                ToggleLearning => next.set_learning_enabled(!current.learning_enabled()),
                ClearLearning => next.clear_learning(),
                Import { path } => {
                    next = store
                        .import(std::path::Path::new(&path), current)
                        .map_err(|e| e.to_string())?;
                    Ok(())
                }
                Export { path } => {
                    store
                        .export(current, std::path::Path::new(&path))
                        .map_err(|e| e.to_string())?;
                    return prepare(
                        &base,
                        &language,
                        current,
                        "已导出；外部导出文件由用户管理".into(),
                    );
                }
                Save => Ok(()),
                _ => unreachable!(),
            };
            result.map_err(|e| e.to_string())?;
            let mut preview = prepare(&base, &language, &next, "修改已保存；用户层待发布".into())?;
            let saved = store
                .save(&next, current.generation())
                .map_err(|e| e.to_string())?;
            finalize(&mut preview, &saved)?;
            dictionary = Some(saved);
            state_reservation = working;
            Ok(preview)
        })();
        if results
            .send(match result {
                Ok(p) => Reply::Ready(p),
                Err(e) => Reply::Error(e),
            })
            .is_err()
        {
            break;
        }
        let _ = &state_reservation;
    }
}

fn reserve_state(
    path: &std::path::Path,
    import: Option<&std::path::Path>,
) -> std::result::Result<Arc<budget::Reservation>, String> {
    let mut bytes = 0;
    for (n, entry) in std::fs::read_dir(path)
        .map_err(|_| "用户目录元数据无法读取")?
        .enumerate()
    {
        if n >= 64 {
            return Err("用户目录文件过多，未读取私有内容".into());
        }
        let entry = entry.map_err(|_| "用户目录元数据无效")?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("generation-") || name.starts_with(".pending-") {
            bytes = bytes.max(
                entry
                    .metadata()
                    .map_err(|_| "用户文件元数据无法读取")?
                    .len(),
            );
        }
    }
    if let Some(import) = import {
        bytes = bytes.saturating_add(
            std::fs::metadata(import)
                .map_err(|_| "导入文件元数据无法读取")?
                .len(),
        );
    }
    if bytes > cloudleaf_user::MAX_BYTES as u64 {
        return Err("用户数据超过可读取容量，原件保留".into());
    }
    let charge = (bytes as usize)
        .saturating_add(1024)
        .saturating_mul(16)
        .saturating_add(64 * 1024);
    budget::reserve_batch(1, charge)
        .map(Arc::new)
        .ok_or_else(|| "用户存储工作容量超过128MiB门禁，未读取/复制数据".into())
}

fn finalize(preview: &mut Prepared, saved: &UserDictionary) -> std::result::Result<(), String> {
    preview.listing = preview.listing.replacen(
        &format!("generation={}；", preview.generation),
        &format!("generation={}；", saved.generation()),
        1,
    );
    preview.generation = saved.generation();
    let snapshot = Arc::get_mut(&mut preview.snapshot).ok_or("未发布快照意外共享，需reload")?;
    snapshot
        .bind_user_state(&saved.to_bytes())
        .map_err(|_| "保存状态身份未能绑定，需reload")?;
    if let Some(language) = &mut preview.language {
        Arc::get_mut(language)
            .ok_or("未发布模型意外共享，需reload")?
            .bind_user_state(snapshot)
            .map_err(|_| "保存状态模型未能绑定，需reload")?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "user_worker_tests.rs"]
mod tests;
