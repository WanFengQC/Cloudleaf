//! 已校验语义候选的研究只读查询。注释只绑定候选身份，不参与排序或上屏。
use super::{bounded_read, hash, sha256, text};
use crate::{ReadPolicy, TranslationError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;
const QUERY_LIMIT: usize = 64;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticCatalogBinding {
    pub namespace: String,
    pub language: String,
    pub manifest_sha256: String,
    pub facts_sha256: String,
    pub runtime_sha256: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticSource {
    pub id: String,
    pub version: String,
    pub artifact_sha256: String,
    pub license_id: String,
    pub status: String,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct SemanticEntryKey {
    pub namespace: String,
    pub entry_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticEntry {
    pub namespace: String,
    pub entry_id: String,
    pub headword: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticAssertion {
    pub source_id: String,
    pub unit_id: String,
    pub pointer: String,
    pub sha256: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPartOfSpeech {
    pub namespace: String,
    pub entry_id: String,
    pub tag: String,
    pub tagset: String,
    pub tagset_version: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticSense {
    pub id: String,
    pub namespace: String,
    pub entry_id: String,
    pub language: String,
    pub definition: String,
    pub english_source_gloss: bool,
    pub part_of_speech: Option<SemanticPartOfSpeech>,
    pub assertions: Vec<SemanticAssertion>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRelation {
    pub id: String,
    pub source_sense_id: String,
    pub target_sense_id: String,
    pub relation: String,
    pub assertions: Vec<SemanticAssertion>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPhrase {
    pub id: String,
    pub namespace: String,
    pub entry_id: String,
    pub kind: String,
    pub components: Vec<SemanticEntryKey>,
    pub assertions: Vec<SemanticAssertion>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LookupIndex {
    format: String,
    schema_version: u8,
    scope: String,
    bundle_manifest_sha256: String,
    content_sha256: String,
    catalogs: Vec<SemanticCatalogBinding>,
    sources: Vec<SemanticSource>,
    entries: Vec<SemanticEntry>,
    senses: Vec<SemanticSense>,
    relations: Vec<SemanticRelation>,
    phrases: Vec<SemanticPhrase>,
}

#[derive(Serialize)]
pub struct SemanticHit<'a> {
    pub entry: &'a SemanticEntry,
    pub sense: &'a SemanticSense,
}

#[derive(Serialize)]
pub struct SemanticHits<'a> {
    pub total: usize,
    pub hits: Vec<SemanticHit<'a>>,
}

#[derive(Serialize)]
pub struct SemanticRelations<'a> {
    pub outgoing_total: usize,
    pub incoming_total: usize,
    pub outgoing: Vec<&'a SemanticRelation>,
    pub incoming: Vec<&'a SemanticRelation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticAnnotationIdentity {
    pub session: u64,
    pub input_revision: u64,
    pub candidate_id: String,
    pub dictionary_runtime_sha256: String,
    pub bundle_manifest_sha256: String,
    pub lookup_index_sha256: String,
}

#[derive(Serialize)]
pub struct SemanticAnnotation<'a> {
    pub identity: SemanticAnnotationIdentity,
    pub glosses: SemanticHits<'a>,
}

impl SemanticAnnotation<'_> {
    pub fn is_current(&self, live: &SemanticAnnotationIdentity) -> bool {
        self.identity == *live
    }
}

type StringPair = (String, String);
type ByText = HashMap<String, Vec<usize>>;
type ByPair = HashMap<StringPair, Vec<usize>>;

pub struct SemanticDictionary {
    index: LookupIndex,
    index_sha256: String,
    entries: HashMap<StringPair, usize>,
    catalog: HashMap<String, usize>,
    by_entry: ByPair,
    by_headword: ByPair,
    by_english_gloss: ByText,
    by_english_keyword: ByText,
    outgoing: ByText,
    incoming: ByText,
    phrases_by_entry: ByPair,
}

impl SemanticDictionary {
    /// 由后台或研发工具一次读取并校验，后续查询只访问内存。
    pub fn load(
        path: &Path,
        expected_sha256: &str,
        policy: ReadPolicy,
    ) -> Result<Self, TranslationError> {
        if policy != ReadPolicy::Research {
            return Err(TranslationError::DataNotApproved);
        }
        let raw = bounded_read(path, MAX_INDEX_BYTES)?;
        let index_sha256 = sha256(&raw);
        if index_sha256 != expected_sha256 {
            return Err(TranslationError::InvalidConfig);
        }
        let index: LookupIndex =
            serde_json::from_slice(&raw).map_err(|_| TranslationError::InvalidConfig)?;
        validate_index_header(&index)?;
        let mut catalog = HashMap::new();
        for (position, row) in index.catalogs.iter().enumerate() {
            require(
                text(&row.namespace, 4096)
                    && text(&row.language, 64)
                    && hash(&row.manifest_sha256)
                    && hash(&row.facts_sha256)
                    && hash(&row.runtime_sha256)
                    && catalog.insert(row.namespace.clone(), position).is_none(),
            )?;
        }
        let mut sources = HashSet::new();
        for row in &index.sources {
            require(
                text(&row.id, 4096)
                    && text(&row.version, 4096)
                    && hash(&row.artifact_sha256)
                    && text(&row.license_id, 4096)
                    && matches!(row.status.as_str(), "research_only" | "approved")
                    && sources.insert(row.id.as_str()),
            )?;
        }
        let mut entries = HashMap::new();
        for (position, row) in index.entries.iter().enumerate() {
            require(
                catalog.contains_key(&row.namespace)
                    && text(&row.entry_id, 4096)
                    && text(&row.headword, 4096)
                    && entries
                        .insert((row.namespace.clone(), row.entry_id.clone()), position)
                        .is_none(),
            )?;
        }
        let mut ids = HashSet::new();
        let mut sense_ids = HashSet::new();
        let mut by_entry: ByPair = HashMap::new();
        let mut by_headword: ByPair = HashMap::new();
        let mut by_english_gloss: ByText = HashMap::new();
        let mut by_english_keyword: ByText = HashMap::new();
        for (position, row) in index.senses.iter().enumerate() {
            let key = (row.namespace.clone(), row.entry_id.clone());
            let entry = entries.get(&key).ok_or(TranslationError::InvalidConfig)?;
            let binding = &index.catalogs[catalog[&row.namespace]];
            require(
                row.language == binding.language
                    && text(&row.definition, 16384)
                    && (!row.english_source_gloss || row.language == "zh")
                    && row.part_of_speech.as_ref().is_none_or(|pos| {
                        pos.namespace == row.namespace
                            && pos.entry_id == row.entry_id
                            && text(&pos.tag, 4096)
                            && text(&pos.tagset, 4096)
                            && text(&pos.tagset_version, 4096)
                    }),
            )?;
            validate_fact(&row.id, &row.assertions, &mut ids, &sources)?;
            sense_ids.insert(row.id.as_str());
            by_entry.entry(key).or_default().push(position);
            by_headword
                .entry((row.language.clone(), index.entries[*entry].headword.clone()))
                .or_default()
                .push(position);
            if row.english_source_gloss {
                by_english_gloss
                    .entry(row.definition.clone())
                    .or_default()
                    .push(position);
                for keyword in row
                    .definition
                    .split(|character: char| !english_keyword_character(character))
                {
                    if !keyword.is_empty() {
                        let hits = by_english_keyword
                            .entry(keyword.to_ascii_lowercase())
                            .or_default();
                        if hits.last() != Some(&position) {
                            hits.push(position);
                        }
                    }
                }
            }
        }
        let mut outgoing: ByText = HashMap::new();
        let mut incoming: ByText = HashMap::new();
        for (position, row) in index.relations.iter().enumerate() {
            require(
                sense_ids.contains(row.source_sense_id.as_str())
                    && sense_ids.contains(row.target_sense_id.as_str())
                    && row.source_sense_id != row.target_sense_id
                    && matches!(
                        row.relation.as_str(),
                        "equivalent" | "broader" | "narrower" | "related"
                    ),
            )?;
            validate_fact(&row.id, &row.assertions, &mut ids, &sources)?;
            outgoing
                .entry(row.source_sense_id.clone())
                .or_default()
                .push(position);
            incoming
                .entry(row.target_sense_id.clone())
                .or_default()
                .push(position);
        }
        let mut phrases_by_entry: ByPair = HashMap::new();
        for (position, row) in index.phrases.iter().enumerate() {
            let key = (row.namespace.clone(), row.entry_id.clone());
            require(
                entries.contains_key(&key)
                    && matches!(row.kind.as_str(), "idiom" | "collocation" | "multiword")
                    && (1..=64).contains(&row.components.len())
                    && row.components.iter().all(|component| {
                        entries.contains_key(&(
                            component.namespace.clone(),
                            component.entry_id.clone(),
                        ))
                    }),
            )?;
            validate_fact(&row.id, &row.assertions, &mut ids, &sources)?;
            phrases_by_entry.entry(key).or_default().push(position);
        }
        Ok(Self {
            index,
            index_sha256,
            entries,
            catalog,
            by_entry,
            by_headword,
            by_english_gloss,
            by_english_keyword,
            outgoing,
            incoming,
            phrases_by_entry,
        })
    }

    pub fn bundle_manifest_sha256(&self) -> &str {
        &self.index.bundle_manifest_sha256
    }

    pub fn content_sha256(&self) -> &str {
        &self.index.content_sha256
    }

    pub fn lookup_index_sha256(&self) -> &str {
        &self.index_sha256
    }

    pub fn sources(&self) -> &[SemanticSource] {
        &self.index.sources
    }

    pub fn catalogs(&self) -> &[SemanticCatalogBinding] {
        &self.index.catalogs
    }

    pub fn entry(&self, namespace: &str, id: &str) -> SemanticHits<'_> {
        self.hits(self.by_entry.get(&(namespace.to_owned(), id.to_owned())))
    }

    pub fn headword(&self, language: &str, text: &str) -> SemanticHits<'_> {
        self.hits(
            self.by_headword
                .get(&(language.to_owned(), text.to_owned())),
        )
    }

    /// 英语释义原文反查。命中意味着可检索到源词头，不生成反向翻译。
    pub fn english_gloss_exact(&self, text: &str) -> SemanticHits<'_> {
        self.hits(self.by_english_gloss.get(text))
    }

    /// 单词检索只索引原始英语释义，返回原文和来源。
    pub fn english_gloss_keyword(&self, keyword: &str) -> SemanticHits<'_> {
        if keyword.is_empty() || !keyword.chars().all(english_keyword_character) {
            return self.hits(None);
        }
        self.hits(self.by_english_keyword.get(&keyword.to_ascii_lowercase()))
    }

    pub fn relations(&self, sense_id: &str) -> SemanticRelations<'_> {
        let outgoing = self
            .outgoing
            .get(sense_id)
            .map_or([].as_slice(), Vec::as_slice);
        let incoming = self
            .incoming
            .get(sense_id)
            .map_or([].as_slice(), Vec::as_slice);
        let rows = |positions: &[usize]| {
            positions
                .iter()
                .take(QUERY_LIMIT)
                .map(|&i| &self.index.relations[i])
                .collect()
        };
        SemanticRelations {
            outgoing_total: outgoing.len(),
            incoming_total: incoming.len(),
            outgoing: rows(outgoing),
            incoming: rows(incoming),
        }
    }

    pub fn phrases_for_entry(&self, namespace: &str, id: &str) -> Vec<&SemanticPhrase> {
        self.phrases_by_entry
            .get(&(namespace.to_owned(), id.to_owned()))
            .into_iter()
            .flatten()
            .take(QUERY_LIMIT)
            .map(|&i| &self.index.phrases[i])
            .collect()
    }

    pub fn prepare_annotation(
        &self,
        identity: SemanticAnnotationIdentity,
        namespace: &str,
        entry_id: &str,
    ) -> Result<SemanticAnnotation<'_>, TranslationError> {
        let catalog = self
            .catalog
            .get(namespace)
            .ok_or(TranslationError::InvalidConfig)?;
        require(
            identity.session != 0
                && text(&identity.candidate_id, 4096)
                && identity.dictionary_runtime_sha256
                    == self.index.catalogs[*catalog].runtime_sha256
                && identity.bundle_manifest_sha256 == self.index.bundle_manifest_sha256
                && identity.lookup_index_sha256 == self.index_sha256,
        )?;
        Ok(SemanticAnnotation {
            identity,
            glosses: self.entry(namespace, entry_id),
        })
    }

    fn hits(&self, rows: Option<&Vec<usize>>) -> SemanticHits<'_> {
        let rows = rows.map_or([].as_slice(), Vec::as_slice);
        SemanticHits {
            total: rows.len(),
            hits: rows
                .iter()
                .take(QUERY_LIMIT)
                .map(|&index| {
                    let sense = &self.index.senses[index];
                    let entry = self.entries[&(sense.namespace.clone(), sense.entry_id.clone())];
                    SemanticHit {
                        entry: &self.index.entries[entry],
                        sense,
                    }
                })
                .collect(),
        }
    }
}

fn english_keyword_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '\'')
}

fn require(condition: bool) -> Result<(), TranslationError> {
    condition
        .then_some(())
        .ok_or(TranslationError::InvalidConfig)
}

fn validate_index_header(index: &LookupIndex) -> Result<(), TranslationError> {
    require(
        index.format == "cloudleaf-semantic-lookup-v1"
            && index.schema_version == 1
            && index.scope == "research_only"
            && hash(&index.bundle_manifest_sha256)
            && hash(&index.content_sha256)
            && !index.catalogs.is_empty()
            && !index.sources.is_empty()
            && !index.senses.is_empty(),
    )
}

fn validate_fact<'a>(
    id: &'a str,
    assertions: &[SemanticAssertion],
    ids: &mut HashSet<&'a str>,
    sources: &HashSet<&str>,
) -> Result<(), TranslationError> {
    require(
        text(id, 4096)
            && ids.insert(id)
            && !assertions.is_empty()
            && assertions.iter().all(|row| {
                sources.contains(row.source_id.as_str())
                    && text(&row.unit_id, 4096)
                    && text(&row.pointer, 4096)
                    && hash(&row.sha256)
            }),
    )
}
