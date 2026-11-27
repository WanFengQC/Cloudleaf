use crate::{read_bounded, write_atomic};
use cloudleaf_input::{LanguageSnapshot, SearchSnapshot};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::{SystemTime, UNIX_EPOCH};

const STORE: &str = "language-resources.json";
const RECEIPTS: &str = "resource-runtime";
const CONFIG_BYTES: usize = 32 * 1024;
const RESOURCE_BYTES: usize = 32 * 1024 * 1024;
const INDEX_BYTES: usize = 192 * 1024 * 1024;
const RECEIPT_BYTES: usize = 8192;
const ACTIVE_MS: u64 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceSlot {
    Base,
    Supplement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAdmission {
    ApprovedSource,
    ResearchOnly,
    StartupUnregistered,
}

impl ResourceAdmission {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ApprovedSource => "来源与许可已核对",
            Self::ResearchOnly => "仅研究",
            Self::StartupUnregistered => "宿主启动资源",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DictionaryArtifact {
    path: PathBuf,
    sha256: String,
}

impl DictionaryArtifact {
    pub fn new(path: PathBuf, sha256: String) -> Result<Self, ResourceError> {
        let artifact = Self {
            path,
            sha256: sha256.to_ascii_lowercase(),
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    fn validate(&self) -> Result<(), ResourceError> {
        if !self.path.is_absolute()
            || self.path.as_os_str().len() > 4096
            || !is_digest(&self.sha256)
        {
            return Err(ResourceError::InvalidSettings);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DictionaryIdentity {
    pub version: String,
    pub runtime_sha256: String,
    pub source_id: String,
    pub source_sha256: Option<String>,
    pub source_lock: Option<String>,
    pub source_lock_sha256: Option<String>,
    pub license_id: String,
    pub license_sha256: Option<String>,
    pub admission: ResourceAdmission,
}

#[derive(Deserialize)]
struct Registry {
    schema_version: u32,
    resources: Vec<RegistryEntry>,
}

#[derive(Deserialize)]
struct RegistryEntry {
    slot: ResourceSlot,
    #[serde(flatten)]
    identity: DictionaryIdentity,
}

static REGISTRY: LazyLock<Registry> = LazyLock::new(|| {
    let registry: Registry =
        serde_json::from_str(include_str!("../../../config/language-resources.json"))
            .expect("随源码审阅的词典来源登记");
    assert_eq!(registry.schema_version, 1);
    registry
});

fn registered(slot: ResourceSlot, sha256: &str) -> Option<&'static DictionaryIdentity> {
    REGISTRY.resources.iter().find_map(|entry| {
        (entry.slot == slot && entry.identity.runtime_sha256 == sha256).then_some(&entry.identity)
    })
}

fn approved(
    slot: ResourceSlot,
    artifact: &DictionaryArtifact,
) -> Result<&'static DictionaryIdentity, ResourceError> {
    let identity = registered(slot, artifact.sha256()).ok_or(ResourceError::Unapproved)?;
    if slot == ResourceSlot::Base && identity.admission != ResourceAdmission::ApprovedSource {
        return Err(ResourceError::Unapproved);
    }
    Ok(identity)
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageResourceConfig {
    pub schema_version: u32,
    pub revision: u64,
    pub base: Option<DictionaryArtifact>,
    pub supplement: Option<DictionaryArtifact>,
    pub supplement_enabled: bool,
    pub previous_base: Option<DictionaryArtifact>,
    pub previous_supplement: Option<DictionaryArtifact>,
}

impl Default for LanguageResourceConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 1,
            base: None,
            supplement: None,
            supplement_enabled: false,
            previous_base: None,
            previous_supplement: None,
        }
    }
}

impl LanguageResourceConfig {
    fn validate(&self) -> Result<(), ResourceError> {
        if self.schema_version != 1
            || self.revision == 0
            || (self.supplement_enabled && self.supplement.is_none())
        {
            return Err(ResourceError::InvalidSettings);
        }
        for (slot, item) in [
            (ResourceSlot::Base, &self.base),
            (ResourceSlot::Base, &self.previous_base),
            (ResourceSlot::Supplement, &self.supplement),
            (ResourceSlot::Supplement, &self.previous_supplement),
        ] {
            if let Some(artifact) = item {
                artifact.validate()?;
                approved(slot, artifact)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceAction {
    Select(ResourceSlot, DictionaryArtifact),
    SetSupplementEnabled(bool),
    Rollback(ResourceSlot),
    ResetBase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceError {
    InvalidSettings,
    StaleRevision,
    Unapproved,
    NoPrevious,
    Io,
    VerifyResource,
    IncompatibleLayers,
    Capacity,
}

impl fmt::Display for ResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSettings => "资源设置无效，请检查路径、身份、版本与启用状态",
            Self::StaleRevision => "资源设置已更新，请重读当前版本",
            Self::Unapproved => "资源身份与该槽位的已核对来源不匹配",
            Self::NoPrevious => "此槽没有可回退的选择",
            Self::Io => "资源文件或设置无法读写",
            Self::VerifyResource => "资源 SHA-256、TSV 结构或索引校验失败",
            Self::IncompatibleLayers => "基础与补充的稳定词条 ID 或组合格式冲突",
            Self::Capacity => "词典文件或已准备索引超出装载容量",
        })
    }
}

impl std::error::Error for ResourceError {}

pub struct LanguageResourceRepository {
    directory: PathBuf,
}

impl LanguageResourceRepository {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn load(&self) -> Result<LanguageResourceConfig, ResourceError> {
        let bytes = match read_bounded(&self.directory.join(STORE), CONFIG_BYTES) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(LanguageResourceConfig::default());
            }
            Err(_) => return Err(ResourceError::Io),
        };
        if bytes.len() > CONFIG_BYTES {
            return Err(ResourceError::InvalidSettings);
        }
        let config: LanguageResourceConfig =
            serde_json::from_slice(&bytes).map_err(|_| ResourceError::InvalidSettings)?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(
        &self,
        expected_revision: u64,
        action: ResourceAction,
    ) -> Result<LanguageResourceConfig, ResourceError> {
        fs::create_dir_all(&self.directory).map_err(|_| ResourceError::Io)?;
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(self.directory.join("language-resources.lock"))
            .map_err(|_| ResourceError::Io)?;
        lock.lock().map_err(|_| ResourceError::Io)?;
        let current = self.load()?;
        if current.revision != expected_revision {
            return Err(ResourceError::StaleRevision);
        }
        let mut next = current.clone();
        match action {
            ResourceAction::Select(slot, artifact) => {
                approved(slot, &artifact)?;
                verify_dictionary(&artifact)?;
                let (active, previous) = match slot {
                    ResourceSlot::Base => (&mut next.base, &mut next.previous_base),
                    ResourceSlot::Supplement => {
                        (&mut next.supplement, &mut next.previous_supplement)
                    }
                };
                if active.as_ref() != Some(&artifact) {
                    let old = active.replace(artifact);
                    if old
                        .as_ref()
                        .is_none_or(|prior| verify_dictionary(prior).is_ok())
                    {
                        *previous = old;
                    }
                }
            }
            ResourceAction::SetSupplementEnabled(enabled) => {
                if enabled {
                    verify_dictionary(
                        next.supplement
                            .as_ref()
                            .ok_or(ResourceError::InvalidSettings)?,
                    )?;
                }
                next.supplement_enabled = enabled;
            }
            ResourceAction::ResetBase => {
                if let Some(old) = next.base.take()
                    && verify_dictionary(&old).is_ok()
                {
                    next.previous_base = Some(old);
                }
            }
            ResourceAction::Rollback(slot) => {
                let (active, previous) = match slot {
                    ResourceSlot::Base => (&mut next.base, &mut next.previous_base),
                    ResourceSlot::Supplement => {
                        (&mut next.supplement, &mut next.previous_supplement)
                    }
                };
                if active.is_none() && previous.is_none() {
                    return Err(ResourceError::NoPrevious);
                }
                if let Some(target) = previous.as_ref() {
                    verify_dictionary(target)?;
                }
                std::mem::swap(active, previous);
                if previous
                    .as_ref()
                    .is_some_and(|old| verify_dictionary(old).is_err())
                {
                    *previous = None;
                }
                next.supplement_enabled &= next.supplement.is_some();
            }
        }
        next.validate()?;
        if next == current {
            return Ok(current);
        }
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or(ResourceError::StaleRevision)?;
        let bytes = serde_json::to_vec_pretty(&next).map_err(|_| ResourceError::InvalidSettings)?;
        write_atomic(&self.directory.join(STORE), &bytes).map_err(|_| ResourceError::Io)?;
        Ok(next)
    }

    pub fn publish_receipt(&self, receipt: &ResourceReceipt) -> Result<(), ResourceError> {
        if !valid_host(&receipt.host_id) {
            return Err(ResourceError::InvalidSettings);
        }
        let envelope = StoredReceipt {
            receipt: receipt.clone(),
            updated_ms: now_ms(),
        };
        let bytes = serde_json::to_vec(&envelope).map_err(|_| ResourceError::InvalidSettings)?;
        if bytes.len() > RECEIPT_BYTES {
            return Err(ResourceError::Capacity);
        }
        let directory = self.directory.join(RECEIPTS);
        fs::create_dir_all(&directory).map_err(|_| ResourceError::Io)?;
        write_atomic(&directory.join(format!("{}.json", receipt.host_id)), &bytes)
            .map_err(|_| ResourceError::Io)
    }

    pub fn receipts(&self) -> Result<Vec<ResourceReceipt>, ResourceError> {
        let entries = match fs::read_dir(self.directory.join(RECEIPTS)) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(ResourceError::Io),
        };
        let mut receipts = Vec::new();
        let now = now_ms();
        for entry in entries {
            let entry = entry.map_err(|_| ResourceError::Io)?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json")
                || !entry.file_type().is_ok_and(|kind| kind.is_file())
            {
                continue;
            }
            if let Ok(bytes) = read_bounded(&path, RECEIPT_BYTES)
                && bytes.len() <= RECEIPT_BYTES
                && let Ok(envelope) = serde_json::from_slice::<StoredReceipt>(&bytes)
                && now
                    .checked_sub(envelope.updated_ms)
                    .is_some_and(|age| age <= ACTIVE_MS)
                && Some(envelope.receipt.host_id.as_str())
                    == path.file_stem().and_then(|name| name.to_str())
            {
                receipts.push(envelope.receipt);
            }
        }
        receipts.sort_unstable_by(|left, right| left.host_id.cmp(&right.host_id));
        Ok(receipts)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageResourceIdentity {
    pub base: DictionaryIdentity,
    pub supplement: Option<DictionaryIdentity>,
    pub model_sha256: Option<String>,
    pub snapshot_sha256: String,
}

impl LanguageResourceIdentity {
    pub fn startup(base: &SearchSnapshot, model: Option<&LanguageSnapshot>) -> Self {
        let identity = registered(ResourceSlot::Base, base.version())
            .cloned()
            .unwrap_or_else(|| DictionaryIdentity {
                version: base.version().to_owned(),
                runtime_sha256: base.version().to_owned(),
                source_id: "host-startup".to_owned(),
                source_sha256: None,
                source_lock: None,
                source_lock_sha256: None,
                license_id: String::new(),
                license_sha256: None,
                admission: ResourceAdmission::StartupUnregistered,
            });
        Self {
            base: identity,
            supplement: None,
            model_sha256: model.map(|model| model.version().to_owned()),
            snapshot_sha256: base.version().to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceState {
    Preparing,
    Prepared,
    Deferred,
    Applied,
    Rejected,
}

impl ResourceState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Preparing => "后台验证与装载",
            Self::Prepared => "后台快照已准备",
            Self::Deferred => "等待组合与提交收尾",
            Self::Applied => "宿主已应用",
            Self::Rejected => "应用失败，保留旧快照",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceReceipt {
    pub host_id: String,
    pub process_id: u32,
    pub desired_revision: u64,
    pub active_revision: u64,
    pub generation: u64,
    pub active: LanguageResourceIdentity,
    pub user_generation: Option<u64>,
    pub state: ResourceState,
    pub error: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredReceipt {
    receipt: ResourceReceipt,
    updated_ms: u64,
}

#[derive(Clone, Debug)]
pub struct PreparedLanguageResources {
    pub snapshot: Arc<SearchSnapshot>,
    pub language: Option<Arc<LanguageSnapshot>>,
    pub identity: LanguageResourceIdentity,
}

#[derive(Default)]
pub struct LanguageResourceManager;

impl LanguageResourceManager {
    /// 必须由宿主的后台资源服务调用；实例内没有按键路径可变状态。
    pub fn prepare(
        &self,
        config: &LanguageResourceConfig,
        startup_base: &Arc<SearchSnapshot>,
        startup_language: Option<&Arc<LanguageSnapshot>>,
    ) -> Result<PreparedLanguageResources, ResourceError> {
        config.validate()?;
        let mut identity =
            LanguageResourceIdentity::startup(startup_base, startup_language.map(Arc::as_ref));
        let mut base = Arc::clone(startup_base);
        if let Some(artifact) = &config.base {
            identity.base = approved(ResourceSlot::Base, artifact)?.clone();
            base = load_dictionary(artifact, Some(startup_base))?;
        }
        let language = if config.supplement_enabled {
            let artifact = config.supplement.as_ref().expect("已验证补充状态");
            let supplement = load_dictionary(artifact, None)?;
            identity.supplement = Some(approved(ResourceSlot::Supplement, artifact)?.clone());
            base = Arc::new(
                base.with_supplement(&supplement)
                    .map_err(|_| ResourceError::IncompatibleLayers)?,
            );
            None
        } else if Arc::ptr_eq(&base, startup_base) {
            startup_language.cloned()
        } else {
            None
        };
        if base.estimated_bytes() > INDEX_BYTES {
            return Err(ResourceError::Capacity);
        }
        identity.snapshot_sha256 = base.version().to_owned();
        identity.model_sha256 = language.as_ref().map(|model| model.version().to_owned());
        Ok(PreparedLanguageResources {
            snapshot: base,
            language,
            identity,
        })
    }
}

fn load_dictionary(
    artifact: &DictionaryArtifact,
    reuse: Option<&Arc<SearchSnapshot>>,
) -> Result<Arc<SearchSnapshot>, ResourceError> {
    let bytes = read_dictionary(artifact)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| ResourceError::VerifyResource)?;
    if let Some(startup) = reuse.filter(|base| base.version() == artifact.sha256()) {
        startup
            .verify_tsv_identity(text, artifact.sha256())
            .map_err(|_| ResourceError::VerifyResource)?;
        return Ok(Arc::clone(startup));
    }
    let snapshot = SearchSnapshot::from_verified_tsv(text, artifact.sha256())
        .map_err(|_| ResourceError::VerifyResource)?;
    Ok(Arc::new(snapshot))
}

fn verify_dictionary(artifact: &DictionaryArtifact) -> Result<(), ResourceError> {
    let bytes = read_dictionary(artifact)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| ResourceError::VerifyResource)?;
    SearchSnapshot::verify_tsv(text, artifact.sha256()).map_err(|_| ResourceError::VerifyResource)
}

fn read_dictionary(artifact: &DictionaryArtifact) -> Result<Vec<u8>, ResourceError> {
    let info = fs::metadata(artifact.path()).map_err(|_| ResourceError::Io)?;
    if !info.is_file() {
        return Err(ResourceError::VerifyResource);
    }
    if info.len() > RESOURCE_BYTES as u64 {
        return Err(ResourceError::Capacity);
    }
    let bytes = read_bounded(artifact.path(), RESOURCE_BYTES).map_err(|_| ResourceError::Io)?;
    if bytes.len() > RESOURCE_BYTES {
        return Err(ResourceError::Capacity);
    }
    Ok(bytes)
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 96
        && host.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
