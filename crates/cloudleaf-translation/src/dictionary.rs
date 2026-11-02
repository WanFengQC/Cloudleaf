use crate::{
    Language, OutputKind, ProviderCapabilities, ProviderIdentity, ProviderOutput, QualityStatus,
    ReadPolicy, TranslationCancellation, TranslationDirection, TranslationError, TranslationKind,
    TranslationProvider, TranslationRequest,
};
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_CATALOG_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone)]
pub struct DictionaryTrustPolicy {
    pub path: PathBuf,
    pub expected_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DictionaryStats {
    pub entries: usize,
    pub raw_glosses: usize,
    pub mappings: usize,
    pub forward_keys: usize,
    pub reverse_keys: usize,
    pub production_policy_bound: bool,
    pub human_identity_or_rights_proven: bool,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct DictionarySource {
    id: String,
    version: String,
    artifact_sha256: String,
    license_id: String,
    license_file: String,
    license_sha256: String,
    notice_file: String,
    notice_sha256: String,
    source_lock_file: String,
    source_lock_sha256: String,
    review_status: String,
    scope: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DictionaryEntry {
    id: String,
    source_id: String,
    source_record_id: String,
    headword: String,
    raw_glosses: Vec<String>,
    status: ContentStatus,
}

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ContentStatus {
    Pending,
    ResearchOnly,
    Approved,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DictionaryMapping {
    id: String,
    source_id: String,
    source_text: String,
    target_text: String,
    source_language: String,
    target_language: String,
    kind: String,
    status: ContentStatus,
    review_binding_sha256: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DictionaryCatalog {
    format: String,
    schema_version: u32,
    version: String,
    sources: Vec<DictionarySource>,
    entries: Vec<DictionaryEntry>,
    mappings: Vec<DictionaryMapping>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DictionaryApproval {
    schema_version: u32,
    kind: String,
    catalog_sha256: String,
    catalog_version: String,
    review_binding_sha256: String,
    approved_mapping_ids: Vec<String>,
    sources: Vec<DictionarySource>,
    review_document: Option<ReviewDocumentReference>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewDocumentReference {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewDocument {
    schema_version: u32,
    kind: String,
    review_binding_sha256: String,
    records: Vec<MappingReview>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingReview {
    mapping_id: String,
    author_id: String,
    reviews: Vec<ReviewDecision>,
    adjudication: Option<ReviewAdjudication>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewDecision {
    reviewer_id: String,
    decision: String,
    reason: String,
    review_binding_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewAdjudication {
    adjudicator_id: String,
    decision: String,
    reason: String,
    review_binding_sha256: String,
}

pub struct LocalDictionaryProvider {
    identity: ProviderIdentity,
    catalog: DictionaryCatalog,
    catalog_sha256: String,
    read_policy: ReadPolicy,
    forward: HashMap<String, Vec<usize>>,
    reverse: HashMap<String, Vec<usize>>,
    mappings: HashMap<(String, Language, TranslationKind), Vec<usize>>,
}

impl LocalDictionaryProvider {
    /// 载入和文件核验必须在后台完成，后续translate只访问内存索引。
    pub fn load(
        provider_id: &str,
        path: &Path,
        read_policy: ReadPolicy,
        trust: Option<&DictionaryTrustPolicy>,
    ) -> Result<Self, TranslationError> {
        let root = path
            .parent()
            .ok_or(TranslationError::InvalidConfig)?
            .canonicalize()
            .map_err(|_| TranslationError::ProviderUnavailable)?;
        let raw = bounded_read(path, MAX_CATALOG_BYTES)?;
        let catalog = parse_catalog(&raw)?;
        validate_catalog(&catalog, &root)?;
        let catalog_sha256 = sha256(&raw);
        if read_policy == ReadPolicy::Production {
            verify_production(&catalog, &catalog_sha256, &root, trust)?;
        }
        let mut forward: HashMap<String, Vec<usize>> = HashMap::new();
        let mut reverse: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, entry) in catalog.entries.iter().enumerate() {
            forward
                .entry(entry.headword.clone())
                .or_default()
                .push(index);
            for gloss in &entry.raw_glosses {
                reverse.entry(gloss.clone()).or_default().push(index);
            }
        }
        let mut mappings: HashMap<(String, Language, TranslationKind), Vec<usize>> = HashMap::new();
        for (index, row) in catalog.mappings.iter().enumerate() {
            let key = (
                row.source_text.clone(),
                language(&row.source_language)?,
                kind(&row.kind)?,
            );
            mappings.entry(key).or_default().push(index);
        }
        let identity = ProviderIdentity {
            id: provider_id.to_owned(),
            version: format!("{}:{}:{read_policy:?}", catalog.version, catalog_sha256),
        };
        Ok(Self {
            identity,
            catalog,
            catalog_sha256,
            read_policy,
            forward,
            reverse,
            mappings,
        })
    }

    pub fn stats(&self) -> DictionaryStats {
        DictionaryStats {
            entries: self.catalog.entries.len(),
            raw_glosses: self
                .catalog
                .entries
                .iter()
                .map(|row| row.raw_glosses.len())
                .sum(),
            mappings: self.catalog.mappings.len(),
            forward_keys: self.forward.len(),
            reverse_keys: self.reverse.len(),
            production_policy_bound: self.read_policy == ReadPolicy::Production,
            human_identity_or_rights_proven: false,
        }
    }

    pub fn catalog_sha256(&self) -> &str {
        &self.catalog_sha256
    }
}

impl TranslationProvider for LocalDictionaryProvider {
    fn identity(&self) -> ProviderIdentity {
        self.identity.clone()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            directions: vec![
                TranslationDirection {
                    source: Language::Chinese,
                    target: Language::English,
                },
                TranslationDirection {
                    source: Language::English,
                    target: Language::Chinese,
                },
            ],
            kinds: vec![TranslationKind::Word, TranslationKind::Phrase],
            auto_source: true,
            network: false,
            available: true,
        }
    }

    fn translate(
        &self,
        request: &TranslationRequest,
        cancellation: &TranslationCancellation,
    ) -> Result<ProviderOutput, TranslationError> {
        cancellation.check()?;
        if request.kind == TranslationKind::Sentence {
            return Err(TranslationError::UnsupportedTaskKind);
        }
        let source = match (request.direction.source, request.direction.target) {
            (Language::Chinese | Language::Auto, Language::English) => Language::Chinese,
            (Language::English | Language::Auto, Language::Chinese) => Language::English,
            _ => return Err(TranslationError::UnsupportedLanguagePair),
        };
        let key = (request.text.clone(), source, request.kind);
        let mut values = Vec::new();
        let mut ids = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut pending = false;
        let output_kind = if let Some(rows) = self.mappings.get(&key) {
            for &index in rows {
                let row = &self.catalog.mappings[index];
                pending |= row.status == ContentStatus::Pending;
                ids.insert(row.id.clone());
                if seen.insert(row.target_text.clone()) {
                    values.push(row.target_text.clone());
                }
            }
            OutputKind::Translation
        } else {
            let index = if source == Language::Chinese {
                &self.forward
            } else {
                &self.reverse
            };
            let rows = index.get(&request.text).ok_or(TranslationError::NoMatch)?;
            for &entry_index in rows {
                let entry = &self.catalog.entries[entry_index];
                pending |= entry.status == ContentStatus::Pending;
                ids.insert(entry.id.clone());
                let outputs: &[String] = if source == Language::Chinese {
                    &entry.raw_glosses
                } else {
                    std::slice::from_ref(&entry.headword)
                };
                for value in outputs {
                    if seen.insert(value.clone()) {
                        values.push(value.clone());
                    }
                }
            }
            if source == Language::Chinese {
                OutputKind::DictionaryGloss
            } else {
                OutputKind::ReverseLookup
            }
        };
        let translated_text = values.join("\n");
        if ids.len() > 64 || translated_text.len() > 64 * 1024 {
            return Err(TranslationError::ResponseTooLarge);
        }
        let quality = if pending {
            QualityStatus::Pending
        } else if self.read_policy == ReadPolicy::Production {
            QualityStatus::Approved
        } else {
            // 研究载入不能因包内自称approved而绕过外部审批并允许提交。
            QualityStatus::ResearchOnly
        };
        Ok(ProviderOutput {
            translated_text,
            resolved_source: source,
            output_kind,
            quality,
            source_ids: ids.into_iter().collect(),
        })
    }
}

/// 摘要供制作当前审核材料使用；计算成功本身不表示已批准。
pub fn dictionary_review_binding(raw: &[u8]) -> Result<String, TranslationError> {
    review_binding(&parse_catalog(raw)?)
}

fn review_binding(catalog: &DictionaryCatalog) -> Result<String, TranslationError> {
    let mut value = serde_json::to_value(catalog).map_err(|_| TranslationError::InvalidConfig)?;
    for row in value["mappings"]
        .as_array_mut()
        .ok_or(TranslationError::InvalidConfig)?
    {
        let object = row.as_object_mut().ok_or(TranslationError::InvalidConfig)?;
        object.remove("status");
        object.remove("review_binding_sha256");
    }
    let bytes = serde_json::to_vec(&value).map_err(|_| TranslationError::InvalidConfig)?;
    Ok(sha256(&bytes))
}

fn parse_catalog(raw: &[u8]) -> Result<DictionaryCatalog, TranslationError> {
    if raw.len() as u64 > MAX_CATALOG_BYTES {
        return Err(TranslationError::ResponseTooLarge);
    }
    serde_json::from_slice(raw).map_err(|_| TranslationError::InvalidConfig)
}

fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>, TranslationError> {
    let metadata = fs::metadata(path).map_err(|_| TranslationError::ProviderUnavailable)?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(TranslationError::ResponseTooLarge);
    }
    fs::read(path).map_err(|_| TranslationError::ProviderUnavailable)
}

fn sha256(raw: &[u8]) -> String {
    digest(&SHA256, raw)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

fn hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn language(value: &str) -> Result<Language, TranslationError> {
    match value {
        "zh" => Ok(Language::Chinese),
        "en" => Ok(Language::English),
        _ => Err(TranslationError::InvalidConfig),
    }
}

fn kind(value: &str) -> Result<TranslationKind, TranslationError> {
    match value {
        "word" => Ok(TranslationKind::Word),
        "phrase" => Ok(TranslationKind::Phrase),
        _ => Err(TranslationError::InvalidConfig),
    }
}

fn validate_catalog(catalog: &DictionaryCatalog, root: &Path) -> Result<(), TranslationError> {
    let invalid = TranslationError::InvalidConfig;
    if catalog.format != "cloudleaf-translation-dictionary-v1"
        || catalog.schema_version != 1
        || !text(&catalog.version, 128)
        || catalog.sources.is_empty()
        || catalog.sources.len() > 64
        || catalog.entries.len() + catalog.mappings.len() == 0
        || catalog.entries.len() + catalog.mappings.len() > 1_000_000
    {
        return Err(invalid);
    }
    let mut source_ids = HashMap::new();
    for source in &catalog.sources {
        if !text(&source.id, 128)
            || source_ids.insert(source.id.as_str(), source).is_some()
            || !text(&source.version, 128)
            || !text(&source.license_id, 128)
            || !hash(&source.artifact_sha256)
            || !["approved_bounded_scope", "research_only", "pending"]
                .contains(&source.review_status.as_str())
            || source.scope.is_empty()
            || source.scope.len() > 2
            || source
                .scope
                .iter()
                .any(|name| name != "raw_gloss" && name != "translation")
        {
            return Err(invalid);
        }
        for (name, expected) in [
            (&source.license_file, &source.license_sha256),
            (&source.notice_file, &source.notice_sha256),
            (&source.source_lock_file, &source.source_lock_sha256),
        ] {
            let path = Path::new(name);
            if !hash(expected)
                || name.contains('\\')
                || path
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
            {
                return Err(invalid);
            }
            let path = root.join(path).canonicalize().map_err(|_| invalid)?;
            if !path.starts_with(root)
                || !sha256(&bounded_read(&path, 1024 * 1024)?).eq_ignore_ascii_case(expected)
            {
                return Err(invalid);
            }
        }
    }
    let mut ids = BTreeSet::new();
    for entry in &catalog.entries {
        let source = source_ids.get(entry.source_id.as_str()).ok_or(invalid)?;
        if !text(&entry.id, 128)
            || !ids.insert(entry.id.as_str())
            || !text(&entry.source_record_id, 128)
            || !text(&entry.headword, 1024)
            || entry.raw_glosses.is_empty()
            || entry.raw_glosses.len() > 64
            || entry.raw_glosses.iter().any(|gloss| !text(gloss, 4096))
            || entry.status == ContentStatus::Approved
            || !source.scope.iter().any(|scope| scope == "raw_gloss")
        {
            return Err(invalid);
        }
    }
    let binding = if catalog.mappings.is_empty() {
        None
    } else {
        Some(review_binding(catalog)?)
    };
    for mapping in &catalog.mappings {
        let source = source_ids.get(mapping.source_id.as_str()).ok_or(invalid)?;
        if !text(&mapping.id, 128)
            || !ids.insert(mapping.id.as_str())
            || !source.scope.iter().any(|scope| scope == "translation")
            || !text(&mapping.source_text, 16 * 1024)
            || !text(&mapping.target_text, 64 * 1024)
            || language(&mapping.source_language)? == language(&mapping.target_language)?
        {
            return Err(invalid);
        }
        kind(&mapping.kind)?;
        if mapping.status == ContentStatus::Approved
            && mapping.review_binding_sha256.as_ref() != binding.as_ref()
        {
            return Err(TranslationError::DataNotApproved);
        }
    }
    Ok(())
}

fn verify_production(
    catalog: &DictionaryCatalog,
    catalog_sha256: &str,
    root: &Path,
    trust: Option<&DictionaryTrustPolicy>,
) -> Result<(), TranslationError> {
    let denied = TranslationError::DataNotApproved;
    if !catalog.entries.is_empty()
        || catalog.mappings.is_empty()
        || catalog
            .mappings
            .iter()
            .any(|row| row.status != ContentStatus::Approved)
        || catalog.sources.iter().any(|source| {
            source.review_status != "approved_bounded_scope"
                || !source.scope.iter().any(|scope| scope == "translation")
        })
    {
        return Err(denied);
    }
    let trust = trust.ok_or(denied)?;
    let path = trust.path.canonicalize().map_err(|_| denied)?;
    if path.starts_with(root) || !hash(&trust.expected_sha256) {
        return Err(denied);
    }
    let raw = bounded_read(&path, 1024 * 1024).map_err(|_| denied)?;
    if !sha256(&raw).eq_ignore_ascii_case(&trust.expected_sha256) {
        return Err(denied);
    }
    let approval: DictionaryApproval = serde_json::from_slice(&raw).map_err(|_| denied)?;
    let expected: BTreeSet<_> = catalog.mappings.iter().map(|row| row.id.as_str()).collect();
    let actual: BTreeSet<_> = approval
        .approved_mapping_ids
        .iter()
        .map(String::as_str)
        .collect();
    if approval.schema_version != 1
        || approval.kind != "cloudleaf-translation-dictionary-approval"
        || approval.catalog_sha256 != catalog_sha256
        || approval.catalog_version != catalog.version
        || approval.review_binding_sha256 != review_binding(catalog)?
        || approval.sources != catalog.sources
        || actual != expected
        || actual.len() != approval.approved_mapping_ids.len()
    {
        return Err(denied);
    }
    verify_reviews(&approval, &path, root, &expected)?;
    Ok(())
}

fn verify_reviews(
    approval: &DictionaryApproval,
    policy_path: &Path,
    catalog_root: &Path,
    expected: &BTreeSet<&str>,
) -> Result<(), TranslationError> {
    let denied = TranslationError::DataNotApproved;
    let reference = approval.review_document.as_ref().ok_or(denied)?;
    let relative = Path::new(&reference.path);
    if !hash(&reference.sha256)
        || reference.path.contains('\\')
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(denied);
    }
    let path = policy_path
        .parent()
        .ok_or(denied)?
        .join(relative)
        .canonicalize()
        .map_err(|_| denied)?;
    if path.starts_with(catalog_root) {
        return Err(denied);
    }
    let raw = bounded_read(&path, 1024 * 1024).map_err(|_| denied)?;
    if !sha256(&raw).eq_ignore_ascii_case(&reference.sha256) {
        return Err(denied);
    }
    let document: ReviewDocument = serde_json::from_slice(&raw).map_err(|_| denied)?;
    if document.schema_version != 1
        || document.kind != "cloudleaf-translation-dictionary-reviews"
        || document.review_binding_sha256 != approval.review_binding_sha256
        || document.records.len() != expected.len()
    {
        return Err(denied);
    }
    let binding = &document.review_binding_sha256;
    let mut seen = BTreeSet::new();
    for row in &document.records {
        if !expected.contains(row.mapping_id.as_str())
            || !seen.insert(row.mapping_id.as_str())
            || !text(&row.author_id, 128)
            || row.reviews.len() != 2
        {
            return Err(denied);
        }
        let mut reviewers = BTreeSet::new();
        for review in &row.reviews {
            if !text(&review.reviewer_id, 128)
                || review.reviewer_id == row.author_id
                || !reviewers.insert(review.reviewer_id.as_str())
                || !text(&review.reason, 8192)
                || !["approve", "reject"].contains(&review.decision.as_str())
                || &review.review_binding_sha256 != binding
            {
                return Err(denied);
            }
        }
        if let Some(decision) = &row.adjudication {
            if !text(&decision.adjudicator_id, 128)
                || decision.adjudicator_id == row.author_id
                || reviewers.contains(decision.adjudicator_id.as_str())
                || decision.decision != "approve"
                || !text(&decision.reason, 8192)
                || &decision.review_binding_sha256 != binding
            {
                return Err(denied);
            }
        } else if row.reviews.iter().any(|review| review.decision == "reject") {
            return Err(denied);
        }
    }
    Ok(())
}
