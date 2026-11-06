//! 显式试验后端；词典在会话创建前校验，按键查询只读不可变快照。
use cloudleaf_search::{
    Config, Features, LanguageModelSnapshot, Lattice, Lexicon, MatchKind, Method, Ranker,
    SearchError, Statistics, search_graph,
};
use cloudleaf_types::{Candidate, CoreError, ErrorCode};
use ring::digest::{Context, SHA256, digest};
use std::sync::Arc;

const INPUT_BYTES: usize = 64;
const CANDIDATE_ID_BYTES: usize = 64;
const SEARCH_SOURCE: &str = "cloudleaf:search";

/// 身份来自完整原始TSV字节；不以词条数量或文件名替代版本。
#[derive(Debug)]
pub struct SearchSnapshot {
    lexicon: Lexicon,
    version: String,
    max_text_bytes: usize,
    max_edge_source_bytes: usize,
}

impl SearchSnapshot {
    pub fn from_verified_tsv(text: &str, expected_sha256: &str) -> Result<Self, CoreError> {
        let actual = hex(digest(&SHA256, text.as_bytes()).as_ref());
        if actual != expected_sha256 || !text.starts_with("# cloudleaf-lexicon-v2\n") {
            return Err(CoreError::new(
                ErrorCode::InvalidDictionary,
                "词典版本或实际SHA-256不符",
            ));
        }
        let lexicon = Lexicon::from_tsv(text)
            .map_err(|_| CoreError::new(ErrorCode::InvalidDictionary, "词典快照格式或索引无效"))?;
        let max_text_bytes = lexicon
            .entries()
            .iter()
            .map(|entry| {
                let letters = entry.pinyin.bytes().filter(|byte| *byte != b' ').count();
                let last = entry.pinyin.rsplit(' ').next().unwrap().len();
                // 最后一个音节可为前缀；不能按完整读音长度低估每消耗字母的输出。
                entry.text.len().div_ceil(letters - last + 1)
            })
            .max()
            .unwrap_or(0);
        let max_edge_source_bytes = lexicon
            .entries()
            .iter()
            .map(|entry| {
                entry.id.len()
                    + entry.source.len()
                    + entry.source_record_id.len()
                    + entry.license_id.len()
            })
            .max()
            .unwrap_or(0);
        Ok(Self {
            lexicon,
            version: actual,
            max_text_bytes,
            max_edge_source_bytes,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn entries(&self) -> &[cloudleaf_search::Entry] {
        self.lexicon.entries()
    }

    /// 容器容量与固定工具链树节点估计，包含32字节的本对象余量；不是RSS硬上界。
    pub fn estimated_bytes(&self) -> usize {
        self.lexicon.memory_estimate().total_estimated_bytes
            + std::mem::size_of::<Self>()
            + self.version.capacity()
            + 32
    }

    pub fn capacity_bounds(&self) -> SearchCapacityBounds {
        self.bounds(50)
    }

    fn bounds(&self, candidates: usize) -> SearchCapacityBounds {
        let max_candidate_text_bytes = self.max_text_bytes * INPUT_BYTES;
        // 每条边至少消耗一个ASCII字母；末音节前缀仍只增加一个词条。
        let diagnostic_path_bytes = std::mem::size_of::<SearchPath>()
            + CANDIDATE_ID_BYTES
            + INPUT_BYTES * (std::mem::size_of::<EdgeSource>() + self.max_edge_source_bytes);
        SearchCapacityBounds {
            max_input_bytes: INPUT_BYTES,
            max_candidates: candidates,
            max_candidate_id_bytes: CANDIDATE_ID_BYTES,
            max_candidate_text_bytes,
            max_candidate_annotation_bytes: 0,
            max_candidate_source_bytes: SEARCH_SOURCE.len(),
            max_diagnostic_bytes: std::mem::size_of::<SearchDiagnostic>()
                + CANDIDATE_ID_BYTES
                + CANDIDATE_ID_BYTES
                + 128
                + INPUT_BYTES
                + candidates * diagnostic_path_bytes,
            max_frame_bytes: std::mem::size_of::<cloudleaf_types::InputFrame>()
                + INPUT_BYTES
                + candidates
                    * (std::mem::size_of::<Candidate>()
                        + CANDIDATE_ID_BYTES
                        + max_candidate_text_bytes
                        + SEARCH_SOURCE.len()),
        }
    }
}

/// 模型在输入路径外按完整字节校验并与词典绑定，运行时只读共享。
#[derive(Debug)]
pub struct LanguageSnapshot {
    snapshot: LanguageModelSnapshot,
    version: String,
}

impl LanguageSnapshot {
    pub fn from_verified_tsv(
        text: &str,
        expected_sha256: &str,
        lexicon: &SearchSnapshot,
    ) -> Result<Self, CoreError> {
        let version = hex(digest(&SHA256, text.as_bytes()).as_ref());
        if version != expected_sha256 {
            return Err(CoreError::new(
                ErrorCode::InvalidConfig,
                "语言模型实际SHA-256不符",
            ));
        }
        let snapshot = LanguageModelSnapshot::from_tsv(text, &lexicon.lexicon, lexicon.version())
            .map_err(|_| {
            CoreError::new(ErrorCode::InvalidConfig, "语言模型格式、范围或词典版本不符")
        })?;
        Ok(Self { snapshot, version })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn metadata(&self) -> &cloudleaf_search::ModelMetadata {
        self.snapshot.metadata()
    }

    pub fn estimated_bytes(&self) -> usize {
        self.snapshot.estimated_bytes() + std::mem::size_of::<Self>() + self.version.capacity() + 32
    }
}

/// 字符串有效字节和结构容量上界，不冒充allocator真实峰值或共享Trie字节数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchCapacityBounds {
    pub max_input_bytes: usize,
    pub max_candidates: usize,
    pub max_candidate_id_bytes: usize,
    pub max_candidate_text_bytes: usize,
    pub max_candidate_annotation_bytes: usize,
    pub max_candidate_source_bytes: usize,
    pub max_diagnostic_bytes: usize,
    pub max_frame_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeSource {
    pub entry_index: usize,
    pub entry_id: String,
    pub start: usize,
    pub end: usize,
    pub raw_start: usize,
    pub raw_end: usize,
    pub match_kind: MatchKind,
    pub source: String,
    pub source_record_id: String,
    pub license_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPath {
    pub candidate_id: String,
    pub score: i64,
    pub features: Features,
    pub edges: Vec<EdgeSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDiagnostic {
    pub revision: u64,
    pub snapshot_version: String,
    pub model_version: Option<String>,
    pub context_entry_id: Option<String>,
    pub method: Method,
    pub ranker: Ranker,
    pub raw_input: String,
    pub guaranteed: bool,
    pub waiting: bool,
    pub finite_pool_only: bool,
    pub statistics: Statistics,
    pub paths: Vec<SearchPath>,
    pub error: Option<SearchError>,
}

pub(crate) struct SentenceBackend {
    snapshot: Arc<SearchSnapshot>,
    config: Config,
    language: Option<Arc<LanguageSnapshot>>,
    left_word: Option<String>,
}

impl SentenceBackend {
    pub(crate) fn new(snapshot: Arc<SearchSnapshot>, config: Config) -> Result<Self, CoreError> {
        Self::configured(snapshot, config, None, None)
    }

    pub(crate) fn configured(
        snapshot: Arc<SearchSnapshot>,
        config: Config,
        language: Option<Arc<LanguageSnapshot>>,
        left_word: Option<String>,
    ) -> Result<Self, CoreError> {
        config
            .validate()
            .map_err(|_| CoreError::new(ErrorCode::InvalidConfig, "搜索配置无效"))?;
        let valid_model = match config.ranker {
            Ranker::Structural | Ranker::Weighted => true,
            Ranker::Lexical => language
                .as_ref()
                .is_some_and(|value| value.metadata().kind == "lexical"),
            Ranker::Bigram | Ranker::ContextRerank => language
                .as_ref()
                .is_some_and(|value| value.metadata().kind == "bigram"),
            Ranker::CharacterBigram => language
                .as_ref()
                .is_some_and(|value| value.metadata().kind == "character_bigram"),
        };
        if !valid_model
            || language.as_ref().is_some_and(|value| {
                value.metadata().lexicon_sha256 != snapshot.version
                    || !value
                        .snapshot
                        .model()
                        .is_compatible_lexicon(&snapshot.lexicon)
            })
            || left_word.as_ref().is_some_and(|id| {
                config.ranker != Ranker::ContextRerank
                    || snapshot
                        .entries()
                        .binary_search_by(|entry| entry.id.cmp(id))
                        .is_err()
            })
        {
            return Err(CoreError::new(
                ErrorCode::InvalidConfig,
                "评分种类、上下文或模型词典绑定无效",
            ));
        }
        Ok(Self {
            snapshot,
            config,
            language,
            left_word,
        })
    }

    pub(crate) fn replace_snapshot(
        &mut self,
        snapshot: Arc<SearchSnapshot>,
    ) -> Result<(), CoreError> {
        if self.language.as_ref().is_some_and(|value| {
            value.metadata().lexicon_sha256 != snapshot.version
                || !value
                    .snapshot
                    .model()
                    .is_compatible_lexicon(&snapshot.lexicon)
        }) {
            return Err(CoreError::new(
                ErrorCode::InvalidConfig,
                "先解除旧模型绑定，不能套用到新词典",
            ));
        }
        self.snapshot = snapshot;
        Ok(())
    }

    pub(crate) fn reconfigured(
        &self,
        config: Config,
        language: Option<Arc<LanguageSnapshot>>,
        left_word: Option<String>,
    ) -> Result<Self, CoreError> {
        Self::configured(self.snapshot.clone(), config, language, left_word)
    }

    pub(crate) fn capacity_bounds(&self) -> SearchCapacityBounds {
        self.snapshot.bounds(self.config.output_k)
    }

    pub(crate) fn query(&self, input: &str, revision: u64) -> (Vec<Candidate>, SearchDiagnostic) {
        let mut diagnostic = SearchDiagnostic {
            revision,
            snapshot_version: self.snapshot.version.clone(),
            model_version: self.language.as_ref().map(|value| value.version.clone()),
            context_entry_id: self.left_word.clone(),
            method: self.config.method,
            ranker: self.config.ranker,
            // 非法超长输入仍由InputEngine持有；诊断不另存无界副本或伪造截断原文。
            raw_input: input.into(),
            guaranteed: false,
            waiting: false,
            finite_pool_only: false,
            statistics: Statistics::default(),
            paths: Vec::new(),
            error: None,
        };
        let graph = match Lattice::build(&self.snapshot.lexicon, input, &self.config) {
            Ok(graph) => graph,
            Err(error) => {
                diagnostic.error = Some(error);
                return (Vec::new(), diagnostic);
            }
        };
        let result = match search_graph(
            &self.snapshot.lexicon,
            &graph,
            &self.config,
            self.language.as_ref().map(|value| value.snapshot.model()),
            self.left_word.as_deref(),
        ) {
            Ok(result) => result,
            Err(error) => {
                diagnostic.error = Some(error);
                return (Vec::new(), diagnostic);
            }
        };
        diagnostic.guaranteed = result.guaranteed;
        diagnostic.waiting = result.waiting;
        diagnostic.finite_pool_only = result.finite_pool_only;
        diagnostic.statistics = result.statistics;
        let mut candidates = Vec::with_capacity(result.candidates.len());
        diagnostic.paths = Vec::with_capacity(result.candidates.len());
        for path in result.candidates {
            let id = candidate_id(
                &self.snapshot.version,
                self.language.as_ref().map(|value| value.version()),
                input,
                &path,
                &graph,
            );
            let edges: Vec<_> = path
                .edge_indices
                .iter()
                .map(|index| {
                    let edge = &graph.edges()[*index];
                    let entry = &self.snapshot.lexicon.entries()[edge.entry];
                    EdgeSource {
                        entry_index: edge.entry,
                        entry_id: entry.id.clone(),
                        start: edge.start,
                        end: edge.end,
                        raw_start: edge.raw_start,
                        raw_end: edge.raw_end,
                        match_kind: edge.kind,
                        source: entry.source.clone(),
                        source_record_id: entry.source_record_id.clone(),
                        license_id: entry.license_id.clone(),
                    }
                })
                .collect();
            let frequency = if path.edge_indices.len() == 1 {
                self.snapshot.lexicon.entries()[graph.edges()[path.edge_indices[0]].entry].frequency
            } else {
                None
            };
            let match_kind = if edges
                .iter()
                .any(|edge| edge.match_kind == MatchKind::Prefix)
            {
                cloudleaf_types::MatchKind::Prefix
            } else {
                cloudleaf_types::MatchKind::Exact
            };
            diagnostic.paths.push(SearchPath {
                candidate_id: id.clone(),
                score: path.score,
                features: path.features,
                edges,
            });
            if diagnostic.statistics.budget.is_none() {
                candidates.push(Candidate {
                    id,
                    text: path.text,
                    annotation: None,
                    source: SEARCH_SOURCE.into(),
                    frequency,
                    match_kind,
                });
            }
        }
        (candidates, diagnostic)
    }
}

fn candidate_id(
    version: &str,
    model_version: Option<&str>,
    raw: &str,
    path: &cloudleaf_search::Candidate,
    graph: &Lattice<'_>,
) -> String {
    let mut hash = Context::new(&SHA256);
    for value in ["cloudleaf-sentence-path-v1", version, raw] {
        hash.update(&(value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    if let Some(version) = model_version {
        hash.update(&(version.len() as u64).to_le_bytes());
        hash.update(version.as_bytes());
    }
    for (id, index) in path.entry_ids.iter().zip(&path.edge_indices) {
        hash.update(&(id.len() as u64).to_le_bytes());
        hash.update(id.as_bytes());
        let edge = &graph.edges()[*index];
        for value in [edge.start, edge.end, edge.raw_start, edge.raw_end] {
            hash.update(&(value as u64).to_le_bytes());
        }
        hash.update(&[u8::from(edge.kind == MatchKind::Prefix)]);
    }
    hex(hash.finish().as_ref())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
