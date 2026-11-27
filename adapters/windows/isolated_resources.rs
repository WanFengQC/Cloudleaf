//! 词典配置与索引由后台准备，输入宿主只接收已验证的不可变快照。
use super::{InputFrame, LocalHost, user_worker};
use cloudleaf_input::{LanguageSnapshot, SearchCapacityBounds, SearchSnapshot};
use cloudleaf_settings::{
    LanguageResourceIdentity, LanguageResourceManager, LanguageResourceRepository,
    PreparedLanguageResources, ResourceReceipt, ResourceState,
};
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, Thread},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

type Resources = (Arc<SearchSnapshot>, Option<Arc<LanguageSnapshot>>);
type Retired = (Resources, Option<user_worker::Prepared>);

enum Update {
    Preparing(u64),
    Ready(u64, Result<Box<PreparedLanguageResources>, String>),
    Invalid(String),
}

#[derive(Default)]
struct Exchange {
    stopped: AtomicBool,
    incoming: Mutex<Option<Update>>,
    outgoing: Mutex<Option<ResourceReceipt>>,
    retired: Mutex<Vec<Retired>>,
}

static NEXT_HOST: AtomicU64 = AtomicU64::new(1);

pub(super) struct Runtime {
    exchange: Arc<Exchange>,
    worker: Thread,
    receipt: ResourceReceipt,
    bound: SearchCapacityBounds,
    dirty: bool,
    retire: Vec<Retired>,
    pub(super) current: Resources,
    pub(super) pending: Option<(u64, PreparedLanguageResources)>,
}

impl Runtime {
    pub(super) fn start(directory: PathBuf, local: &LocalHost) -> io::Result<Self> {
        let engine = local.host.engine.borrow();
        let (base, language) = engine.search_resources().expect("宿主已经建立搜索资源");
        let bound = local.resource_bounds.expect("宿主已有容量契约");
        let host_id = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            NEXT_HOST.fetch_add(1, Ordering::Relaxed)
        );
        let active = LanguageResourceIdentity::startup(&base, language.as_deref());
        let exchange = Arc::new(Exchange::default());
        let background = exchange.clone();
        let initial = (base.clone(), language.clone());
        let worker = thread::Builder::new()
            .name("cloudleaf-language-resources".into())
            .spawn(move || watch(directory, initial, background))?
            .thread()
            .clone();
        Ok(Self {
            exchange,
            worker,
            receipt: ResourceReceipt {
                host_id,
                process_id: std::process::id(),
                desired_revision: 0,
                active_revision: 0,
                generation: 1,
                active,
                user_generation: None,
                state: ResourceState::Applied,
                error: None,
            },
            bound,
            dirty: true,
            retire: Vec::new(),
            current: (base, language),
            pending: None,
        })
    }

    pub(super) fn receive(&mut self) {
        if !self.retire.is_empty() {
            let Ok(mut retired) = self.exchange.retired.try_lock() else {
                return;
            };
            if !retired.is_empty() {
                return;
            }
            std::mem::swap(&mut *retired, &mut self.retire);
            self.worker.unpark();
        }
        let update = self
            .exchange
            .incoming
            .try_lock()
            .ok()
            .and_then(|mut m| m.take());
        match update {
            Some(Update::Preparing(revision)) => {
                self.retire_pending();
                self.receipt.desired_revision = revision;
                self.state(ResourceState::Preparing, None);
            }
            Some(Update::Ready(revision, result)) => {
                self.retire_pending();
                self.receipt.desired_revision = revision;
                match result {
                    Ok(next) if revision > self.receipt.active_revision => {
                        self.pending = Some((revision, *next));
                        self.state(ResourceState::Prepared, None);
                    }
                    Ok(next) => {
                        let next = *next;
                        let same = next.snapshot.version() == self.current.0.version();
                        self.retire.push(((next.snapshot, next.language), None));
                        if revision == self.receipt.active_revision && same {
                            self.state(ResourceState::Applied, None);
                        } else {
                            self.reject("词典配置修订过期或同修订内容不同，保留原快照".into());
                        }
                    }
                    Err(error) => self.reject(error),
                }
            }
            Some(Update::Invalid(error)) => self.reject(error),
            None => (),
        }
    }

    pub(super) fn defer(&mut self, reason: &str) {
        if self.receipt.state != ResourceState::Deferred
            || self.receipt.error.as_deref() != Some(reason)
        {
            self.state(ResourceState::Deferred, Some(reason.into()));
        }
    }

    pub(super) fn reject(&mut self, error: String) {
        self.retire_pending();
        self.state(ResourceState::Rejected, Some(error));
    }

    pub(super) fn applied(&mut self, user: Option<user_worker::Prepared>) {
        let (revision, next) = self.pending.take().expect("已完成双引擎资源发布");
        let old = std::mem::replace(&mut self.current, (next.snapshot, next.language));
        // 最后一份大索引引用交由资源线程释放，避免窗口计时器释放词典。
        self.retire.push((old, user));
        self.receipt.active = next.identity;
        self.receipt.active_revision = revision;
        self.receipt.generation += 1;
        self.state(ResourceState::Applied, None);
    }

    pub(super) fn receipt(&self) -> &ResourceReceipt {
        &self.receipt
    }

    pub(super) fn bound(&self) -> SearchCapacityBounds {
        self.bound
    }

    pub(super) fn publish(&mut self, local: &LocalHost, user_generation: Option<u64>) {
        let engine = local.host.engine.borrow();
        if self.receipt.user_generation != user_generation {
            self.receipt.user_generation = user_generation;
            self.dirty = true;
        }
        if let Some(version) = engine.search_snapshot_version()
            && self.receipt.active.snapshot_sha256 != version
        {
            self.receipt.active.snapshot_sha256 = version.into();
            self.dirty = true;
        }
        if self.dirty
            && let Ok(mut out) = self.exchange.outgoing.try_lock()
        {
            *out = Some(self.receipt.clone());
            self.dirty = false;
            self.worker.unpark();
        }
    }

    fn retire_pending(&mut self) {
        if let Some((_, next)) = self.pending.take() {
            self.retire.push(((next.snapshot, next.language), None));
        }
    }

    fn state(&mut self, state: ResourceState, reason: Option<String>) {
        self.receipt.state = state;
        self.receipt.error = reason;
        self.dirty = true;
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.exchange.stopped.store(true, Ordering::Relaxed);
        self.worker.unpark();
    }
}

fn watch(directory: PathBuf, startup: Resources, exchange: Arc<Exchange>) {
    let repo = LanguageResourceRepository::new(directory);
    let manager = LanguageResourceManager;
    let mut previous = None;
    let mut heartbeat = None::<Instant>;
    let mut outgoing = None;
    while !exchange.stopped.load(Ordering::Relaxed) {
        let retired = std::mem::take(&mut *exchange.retired.lock().expect("释放邮箱不可用"));
        drop(retired);
        let loaded = repo.load().map_err(|error| error.to_string());
        if previous.as_ref() != Some(&loaded) {
            previous = Some(loaded.clone());
            let update = match loaded {
                Ok(config) => {
                    *exchange.incoming.lock().expect("词典邮箱不可用") =
                        Some(Update::Preparing(config.revision));
                    let prepared = manager
                        .prepare(&config, &startup.0, startup.1.as_ref())
                        .map(Box::new)
                        .map_err(|error| error.to_string());
                    // 后台构建期间新配置可能已提交。过期快照不会进入宿主。
                    if repo.load().map_err(|error| error.to_string()).as_ref() != Ok(&config) {
                        previous = None;
                        continue;
                    }
                    Update::Ready(config.revision, prepared)
                }
                Err(error) => Update::Invalid(error),
            };
            *exchange.incoming.lock().expect("词典邮箱不可用") = Some(update);
        }
        if let Some(receipt) = exchange.outgoing.lock().expect("词典回执邮箱不可用").take()
        {
            outgoing = Some(receipt);
            heartbeat = None;
        }
        if let Some(receipt) = &outgoing
            && heartbeat.is_none_or(|last| last.elapsed() >= Duration::from_secs(2))
            && repo.publish_receipt(receipt).is_ok()
        {
            heartbeat = Some(Instant::now());
        }
        thread::park_timeout(Duration::from_millis(250));
    }
}

pub(super) fn idle(local: &LocalHost) -> bool {
    !local.host.quarantined.get()
        && local.host.engine.borrow().idle_for_preferences()
        && local.host.projection.borrow().idle_for_preferences()
}

pub(super) fn install(
    local: &LocalHost,
    bound: SearchCapacityBounds,
    snapshot: &Arc<SearchSnapshot>,
    language: Option<&Arc<LanguageSnapshot>>,
    before_apply: impl FnOnce() -> Result<(), String>,
) -> Result<Option<InputFrame>, String> {
    local.host.check().map_err(|error| error.to_string())?;
    if !idle(local) {
        return Ok(None);
    }
    let mut actual = local.host.engine.borrow_mut();
    let mut projected = local.host.projection.borrow_mut();
    let before = actual.frame();
    let mut projection = projected.frame();
    projection.revision = before.revision;
    if before != projection
        || actual.search_snapshot_version() != projected.search_snapshot_version()
    {
        return Err("双引擎当前使用的输入快照不一致".into());
    }
    let actual_update = actual
        .prepare_idle_resources(snapshot.clone(), language.cloned())
        .map_err(|error| error.to_string())?;
    let projection_update = projected
        .prepare_idle_resources(snapshot.clone(), language.cloned())
        .map_err(|error| error.to_string())?;
    for capacity in [
        actual_update.capacity_bounds(),
        projection_update.capacity_bounds(),
    ] {
        if capacity.max_frame_bytes > bound.max_frame_bytes
            || capacity.max_diagnostic_bytes > bound.max_diagnostic_bytes
        {
            return Err("词典超出宿主帧或恢复容量，保留原快照".into());
        }
    }
    before_apply()?;
    actual_update.apply();
    projection_update.apply();
    Ok(Some(actual.frame()))
}
