//! 显式试验后端；词典在会话创建前校验，按键查询只读不可变快照。
use cloudleaf_search::{
    Config, Features, Lattice, Lexicon, MatchKind, Method, Ranker, SearchError, Statistics,
    search_graph,
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
}

impl SentenceBackend {
    pub(crate) fn new(snapshot: Arc<SearchSnapshot>, config: Config) -> Result<Self, CoreError> {
        config
            .validate()
            .map_err(|_| CoreError::new(ErrorCode::InvalidConfig, "搜索配置无效"))?;
        if matches!(config.ranker, Ranker::Bigram | Ranker::ContextRerank) {
            return Err(CoreError::new(
                ErrorCode::InvalidConfig,
                "输入试验未提供历史评分模型",
            ));
        }
        Ok(Self { snapshot, config })
    }

    pub(crate) fn replace_snapshot(&mut self, snapshot: Arc<SearchSnapshot>) {
        self.snapshot = snapshot;
    }

    pub(crate) fn capacity_bounds(&self) -> SearchCapacityBounds {
        self.snapshot.bounds(self.config.output_k)
    }

    pub(crate) fn query(&self, input: &str, revision: u64) -> (Vec<Candidate>, SearchDiagnostic) {
        let mut diagnostic = SearchDiagnostic {
            revision,
            snapshot_version: self.snapshot.version.clone(),
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
        let result = match search_graph(&self.snapshot.lexicon, &graph, &self.config, None, None) {
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
            let id = candidate_id(&self.snapshot.version, input, &path, &graph);
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
    raw: &str,
    path: &cloudleaf_search::Candidate,
    graph: &Lattice<'_>,
) -> String {
    let mut hash = Context::new(&SHA256);
    for value in ["cloudleaf-sentence-path-v1", version, raw] {
        hash.update(&(value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
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
