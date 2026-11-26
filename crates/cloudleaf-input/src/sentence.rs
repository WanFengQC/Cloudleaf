//! 显式试验后端；词典在会话创建前校验，按键查询只读不可变快照。
use crate::mixed;
use cloudleaf_search::{
    Budget, Config, Features, LanguageModelSnapshot, Lattice, Lexicon, MatchKind, Method, Ranker,
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
    base_version: Option<String>,
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
                entry
                    .text
                    .len()
                    .div_ceil(letters - last + 1)
                    .max(entry.text.len().div_ceil(entry.pinyin.split(' ').count()))
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
            base_version: None,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn entries(&self) -> &cloudleaf_search::Entries {
        self.lexicon.entries()
    }

    /// 持久generation、记录revision与学习epoch变化也必须使旧候选身份失效。
    pub fn bind_user_state(&mut self, state: &[u8]) -> Result<(), CoreError> {
        let base = self
            .base_version
            .as_ref()
            .ok_or_else(|| CoreError::new(ErrorCode::InvalidConfig, "基础快照不能绑定用户状态"))?;
        let mut hash = Context::new(&SHA256);
        hash.update(b"cloudleaf-user-state-v1");
        hash.update(base.as_bytes());
        hash.update(state);
        self.version = hex(hash.finish().as_ref());
        Ok(())
    }

    pub fn with_user_layer(
        &self,
        text: &str,
        preferences: &std::collections::BTreeMap<String, u32>,
    ) -> Result<Self, CoreError> {
        let lexicon = self
            .lexicon
            .with_user_layer(text, preferences)
            .map_err(|_| CoreError::new(ErrorCode::InvalidDictionary, "用户层索引或偏好无效"))?;
        let mut hash = Context::new(&SHA256);
        hash.update(self.version.as_bytes());
        hash.update(text.as_bytes());
        for (id, units) in preferences {
            hash.update(&(id.len() as u64).to_le_bytes());
            hash.update(id.as_bytes());
            hash.update(&units.to_le_bytes());
        }
        let max_text_bytes = lexicon
            .entries()
            .iter()
            .map(|entry| entry.text.len().div_ceil(entry.pinyin.split(' ').count()))
            .max()
            .unwrap_or(0)
            .max(self.max_text_bytes);
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
            version: hex(hash.finish().as_ref()),
            max_text_bytes,
            max_edge_source_bytes,
            base_version: Some(self.version.clone()),
        })
    }

    pub fn incremental_estimated_bytes(&self) -> usize {
        self.lexicon.incremental_estimated_bytes()
            + std::mem::size_of::<Self>()
            + self.version.capacity()
            + self.base_version.as_ref().map_or(0, String::capacity)
            + 32
    }

    pub fn entry_fingerprint(&self, id: &str) -> Option<String> {
        self.entries()
            .iter()
            .find(|e| e.id == id)
            .map(|e| entry_fingerprint(&e.text, &e.pinyin))
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
            + INPUT_BYTES * (std::mem::size_of::<EdgeSource>() + self.max_edge_source_bytes)
            // 最多64个非零消耗音节；每弧两条模糊规则或一条编辑，字符串与结构分开计费。
            + INPUT_BYTES * (std::mem::size_of::<cloudleaf_pinyin::PhoneticArc>()
                + 2 * std::mem::size_of::<cloudleaf_pinyin::PhoneticChange>() + 128);
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
                + 1024
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
    binding: String,
}

impl LanguageSnapshot {
    pub fn bind_user_state(&mut self, snapshot: &SearchSnapshot) -> Result<(), CoreError> {
        if snapshot.base_version.as_deref() != Some(self.metadata().lexicon_sha256.as_str())
            || !self
                .snapshot
                .model()
                .is_compatible_lexicon(&snapshot.lexicon)
        {
            return Err(CoreError::new(
                ErrorCode::InvalidConfig,
                "用户状态模型索引不匹配",
            ));
        }
        self.binding = snapshot.version.clone();
        Ok(())
    }
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
        Ok(Self {
            snapshot,
            version,
            binding: lexicon.version.clone(),
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn metadata(&self) -> &cloudleaf_search::ModelMetadata {
        self.snapshot.metadata()
    }

    pub fn for_user_layer(
        &self,
        base: &SearchSnapshot,
        target: &SearchSnapshot,
    ) -> Result<Self, CoreError> {
        let snapshot = self
            .snapshot
            .for_extension(&base.lexicon, &target.lexicon)
            .map_err(|_| CoreError::new(ErrorCode::InvalidConfig, "模型不属于用户层基础快照"))?;
        if self.binding != base.version || target.base_version.as_deref() != Some(base.version()) {
            return Err(CoreError::new(ErrorCode::InvalidConfig, "模型基础身份不符"));
        }
        Ok(Self {
            snapshot,
            version: self.version.clone(),
            binding: target.version.clone(),
        })
    }

    pub fn estimated_bytes(&self) -> usize {
        self.snapshot.estimated_bytes()
            + std::mem::size_of::<Self>()
            + self.version.capacity()
            + self.binding.capacity()
            + 32
    }
}

/// 只由显式研究配置构造，与具体输入及用户扩展快照绑定。
#[derive(Debug)]
pub struct ResearchLanguageSnapshot {
    words: cloudleaf_search::WordContextModel,
    character: Option<Arc<LanguageSnapshot>>,
    binding: String,
    version: String,
    ranker: cloudleaf_search::ResearchRanker,
    pool: usize,
}

impl ResearchLanguageSnapshot {
    pub fn from_verified_tsv(
        word_text: &str,
        word_sha: &str,
        lexicon: &SearchSnapshot,
        character: Option<Arc<LanguageSnapshot>>,
        ranker: cloudleaf_search::ResearchRanker,
        pool: usize,
    ) -> Result<Self, CoreError> {
        use cloudleaf_search::{MAX_RESEARCH_POOL, ResearchRanker, WordContextModel};
        let invalid = || CoreError::new(ErrorCode::InvalidConfig, "研究资源绑定或候选池无效");
        if !(1..=MAX_RESEARCH_POOL).contains(&pool)
            || hex(digest(&SHA256, word_text.as_bytes()).as_ref()) != word_sha
            || character.as_ref().is_some_and(|model| {
                model.binding != lexicon.version()
                    || !model.snapshot.model().has_character_transitions()
                    || !model
                        .snapshot
                        .model()
                        .is_compatible_lexicon(&lexicon.lexicon)
            })
            || (character.is_none()
                && matches!(ranker, ResearchRanker::Character | ResearchRanker::Mixed))
        {
            return Err(invalid());
        }
        let base = lexicon.base_version.as_deref().unwrap_or(lexicon.version());
        let words =
            WordContextModel::from_tsv(word_text, &lexicon.lexicon, base).map_err(|_| invalid())?;
        let selected_character = character.as_ref().map_or("", |value| value.version());
        let fingerprint = format!("{word_sha}:{selected_character}:{ranker:?}:{pool}");
        Ok(Self {
            words,
            character,
            binding: lexicon.version().to_owned(),
            version: hex(digest(&SHA256, fingerprint.as_bytes()).as_ref()),
            ranker,
            pool,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn estimated_bytes(&self) -> usize {
        self.words.estimated_bytes()
            + self
                .character
                .as_ref()
                .map_or(0, |model| model.estimated_bytes())
            + self.binding.capacity()
            + self.version.capacity()
            + std::mem::size_of::<Self>()
    }

    pub(crate) fn character_for_pool(&self) -> Option<Arc<LanguageSnapshot>> {
        self.character.clone()
    }

    fn rerank(
        &self,
        lexicon: &Lexicon,
        graph: &Lattice<'_>,
        config: &Config,
    ) -> Result<cloudleaf_search::SearchResult, SearchError> {
        let pool_config = Config {
            output_k: self.pool,
            context_pool: self.pool.max(config.context_pool),
            ..config.clone()
        };
        let pool_character = (config.ranker == Ranker::CharacterBigram)
            .then(|| self.character.as_ref().map(|model| model.snapshot.model()))
            .flatten();
        let mut result = search_graph(lexicon, graph, &pool_config, pool_character, None)?;
        let order = self.words.rank_pool(
            graph,
            &result,
            self.ranker,
            self.character.as_ref().map(|model| model.snapshot.model()),
        )?;
        let mut candidates: Vec<_> = std::mem::take(&mut result.candidates)
            .into_iter()
            .map(Some)
            .collect();
        result.candidates = order
            .into_iter()
            .take(config.output_k)
            .map(|score| {
                let mut candidate = candidates[score.pool_index]
                    .take()
                    .expect("研究重排每个候选最多返回一次");
                candidate.score = score.final_units;
                candidate.features.lexical_units = score.unigram_units;
                candidate.features.bigram_units = score.word_units;
                candidate.features.context_units = score.boundary_units;
                candidate.features.character_units = score.character_units.unwrap_or(0);
                candidate.features.lexical_observed = score.observed_words;
                candidate.features.lexical_missing = score.unknown_words;
                candidate
            })
            .collect();
        result.finite_pool_only = true;
        Ok(result)
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
    pub interpretation: Option<cloudleaf_search::EdgeInterpretation>,
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
    pub scheme_version: Option<String>,
    pub scheme_statistics: Option<cloudleaf_search::SchemeStatistics>,
}

pub(crate) struct SentenceBackend {
    snapshot: Arc<SearchSnapshot>,
    config: Config,
    language: Option<Arc<LanguageSnapshot>>,
    research: Option<Arc<ResearchLanguageSnapshot>>,
    left_word: Option<String>,
    scheme: cloudleaf_pinyin::SchemeConfig,
    transitions: cloudleaf_search::TransitionCache,
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
            Ranker::CharacterBigram => language.as_ref().is_some_and(|value| {
                matches!(
                    value.metadata().kind.as_str(),
                    "character_bigram" | "character_bigram_witten_bell"
                )
            }),
        };
        if !valid_model
            || language.as_ref().is_some_and(|value| {
                value.binding != snapshot.version
                    || !value
                        .snapshot
                        .model()
                        .is_compatible_lexicon(&snapshot.lexicon)
            })
            || left_word.as_ref().is_some_and(|id| {
                config.ranker != Ranker::ContextRerank
                    || !snapshot.entries().iter().any(|entry| &entry.id == id)
            })
        {
            return Err(CoreError::new(
                ErrorCode::InvalidConfig,
                "评分种类、上下文或模型词典绑定无效",
            ));
        }
        let mut transitions = cloudleaf_search::TransitionCache::default();
        transitions.set_enabled(false);
        Ok(Self {
            snapshot,
            config,
            language,
            research: None,
            left_word,
            scheme: cloudleaf_pinyin::SchemeConfig::default(),
            transitions,
        })
    }

    pub(crate) fn with_research(
        mut self,
        research: Arc<ResearchLanguageSnapshot>,
    ) -> Result<Self, CoreError> {
        if research.binding != self.snapshot.version()
            || !research.words.is_compatible_lexicon(&self.snapshot.lexicon)
            || research.pool < self.config.output_k
            || !matches!(
                self.config.ranker,
                Ranker::Structural | Ranker::CharacterBigram
            )
            || (self.config.ranker == Ranker::CharacterBigram && research.character.is_none())
            || research.estimated_bytes() > 16 * 1024 * 1024
        {
            return Err(CoreError::new(
                ErrorCode::InvalidConfig,
                "研究配置、词典或内存预算不符",
            ));
        }
        self.research = Some(research);
        Ok(self)
    }

    pub(crate) fn scoring_version(&self) -> Option<&str> {
        self.research
            .as_ref()
            .map(|model| model.version())
            .or_else(|| self.language.as_ref().map(|model| model.version()))
    }

    pub(crate) fn replace_snapshot(
        &mut self,
        snapshot: Arc<SearchSnapshot>,
    ) -> Result<(), CoreError> {
        if self.research.is_some() && !Arc::ptr_eq(&self.snapshot, &snapshot)
            || self.language.as_ref().is_some_and(|value| {
                value.binding != snapshot.version
                    || !value
                        .snapshot
                        .model()
                        .is_compatible_lexicon(&snapshot.lexicon)
            })
        {
            return Err(CoreError::new(
                ErrorCode::InvalidConfig,
                "先解除旧模型绑定，不能套用到新词典",
            ));
        }
        self.transitions.clear();
        self.snapshot = snapshot;
        Ok(())
    }

    pub(crate) fn reconfigured(
        &self,
        config: Config,
        language: Option<Arc<LanguageSnapshot>>,
        left_word: Option<String>,
    ) -> Result<Self, CoreError> {
        let mut replacement = Self::configured(self.snapshot.clone(), config, language, left_word)?;
        replacement.scheme = self.scheme.clone();
        replacement
            .transitions
            .set_enabled(self.transitions.is_enabled());
        Ok(replacement)
    }

    pub(crate) fn resources(
        &self,
        snapshot: Arc<SearchSnapshot>,
        language: Option<Arc<LanguageSnapshot>>,
    ) -> Result<Self, CoreError> {
        if self.research.is_some()
            && (!Arc::ptr_eq(&self.snapshot, &snapshot) || language.is_some())
        {
            return Err(CoreError::new(
                ErrorCode::InvalidConfig,
                "研究资源更新须重新编译并与新词典一同装载",
            ));
        }
        let mut replacement = Self::configured(
            snapshot,
            self.config.clone(),
            language,
            self.left_word.clone(),
        )?;
        replacement.research = self.research.clone();
        replacement.scheme = self.scheme.clone();
        replacement
            .transitions
            .set_enabled(self.transitions.is_enabled());
        Ok(replacement)
    }

    pub(crate) fn snapshot_version(&self) -> &str {
        self.snapshot.version()
    }

    pub(crate) fn snapshots(&self) -> (Arc<SearchSnapshot>, Option<Arc<LanguageSnapshot>>) {
        (self.snapshot.clone(), self.language.clone())
    }

    pub(crate) fn scheme(&self) -> &cloudleaf_pinyin::SchemeConfig {
        &self.scheme
    }
    pub(crate) fn set_scheme(&mut self, scheme: cloudleaf_pinyin::SchemeConfig) {
        self.transitions.clear();
        self.scheme = scheme;
    }

    pub(crate) fn validate_output_limit(&self, output_k: usize) -> Result<(), CoreError> {
        let config = Config {
            output_k,
            ..self.config.clone()
        };
        config
            .validate()
            .map_err(|_| CoreError::new(ErrorCode::InvalidConfig, "候选预算超过预留搜索容量"))
    }

    pub(crate) fn apply_preferences(
        &mut self,
        output_k: usize,
        scheme: cloudleaf_pinyin::SchemeConfig,
    ) {
        self.config.output_k = output_k;
        self.set_scheme(scheme);
    }

    pub(crate) fn clear_cache(&mut self) {
        self.transitions.clear();
    }

    pub(crate) fn cache_statistics(&self) -> cloudleaf_search::TransitionCacheStatistics {
        self.transitions.statistics()
    }

    pub(crate) fn set_cache_enabled(&mut self, enabled: bool) {
        self.transitions.set_enabled(enabled);
    }

    pub(crate) fn capacity_bounds(&self) -> SearchCapacityBounds {
        self.snapshot.bounds(self.config.output_k)
    }

    pub(crate) fn query(
        &mut self,
        input: &str,
        revision: u64,
    ) -> (Vec<Candidate>, SearchDiagnostic) {
        self.query_with_config(input, revision, None)
    }

    pub(crate) fn query_mixed(
        &mut self,
        raw: &str,
        revision: u64,
        frame_limit: usize,
    ) -> mixed::Output {
        let (input, punctuation) = mixed::body_and_punctuation(raw);
        if mixed::protected(raw) {
            return mixed::Output::literal(raw, None);
        }
        if raw.len() > self.config.max_input_bytes {
            let mut diagnostic = self.empty_diagnostic(raw, revision);
            diagnostic.error = Some(SearchError {
                kind: cloudleaf_search::ErrorKind::InvalidInput,
                line: None,
                message: "混排原文超过搜索输入预算",
            });
            return mixed::Output::literal(raw, Some(diagnostic));
        }
        let plan = if !self.scheme.is_default()
            && input
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
        {
            mixed::Plan::Chinese
        } else {
            mixed::plan(input, &self.snapshot.lexicon)
        };
        match plan {
            mixed::Plan::Literal => mixed::Output::literal(raw, None),
            mixed::Plan::Chinese => {
                let (mut candidates, mut diagnostic) = self.query(input, revision);
                if candidates.is_empty() {
                    diagnostic.raw_input = raw.into();
                    return mixed::Output::literal(raw, Some(diagnostic));
                }
                if let Some(mark) = punctuation {
                    mixed::with_punctuation(raw, mark, &mut candidates, &mut diagnostic.paths);
                    diagnostic.raw_input = raw.into();
                }
                if candidates.len() < frame_limit.min(self.config.output_k) {
                    candidates.push(mixed::raw_candidate(raw));
                }
                mixed::Output {
                    candidates,
                    diagnostic: Some(diagnostic),
                    has_literal: false,
                    trailing_literal: false,
                }
            }
            mixed::Plan::Mixed(spans) => {
                let mut remaining = self.config.clone();
                let mut statistics = Statistics::default();
                let mut runs = Vec::with_capacity(spans.len());
                let has_tail = spans.last().is_some_and(|range| range.end < raw.len());
                let mut diagnostic = self.empty_diagnostic(raw, revision);
                for span in spans {
                    if span.len() > remaining.max_letters {
                        diagnostic.statistics = statistics;
                        diagnostic.error = Some(SearchError {
                            kind: cloudleaf_search::ErrorKind::InvalidInput,
                            line: None,
                            message: "混排中文片段超过搜索字母总预算",
                        });
                        return mixed::Output::literal(raw, Some(diagnostic));
                    }
                    if remaining.max_edges == 0 || remaining.max_expansions == 0 {
                        statistics.budget = Some(if remaining.max_edges == 0 {
                            Budget::GraphEdges
                        } else {
                            Budget::Expansions
                        });
                        diagnostic.statistics = statistics;
                        return mixed::Output::literal(raw, Some(diagnostic));
                    }
                    let (candidates, subquery) =
                        self.query_with_config(&raw[span.clone()], revision, Some(&remaining));
                    statistics.graph_edges += subquery.statistics.graph_edges;
                    statistics.expansions += subquery.statistics.expansions;
                    statistics.beam_pruned += subquery.statistics.beam_pruned;
                    statistics.resident_high_water = statistics
                        .resident_high_water
                        .max(subquery.statistics.resident_high_water);
                    statistics.budget = subquery.statistics.budget;
                    if candidates.is_empty() || statistics.budget.is_some() {
                        diagnostic.statistics = statistics;
                        diagnostic.error = subquery.error;
                        return mixed::Output::literal(raw, Some(diagnostic));
                    }
                    remaining.max_edges -= subquery.statistics.graph_edges;
                    remaining.max_expansions -= subquery.statistics.expansions;
                    remaining.max_letters -= span.len();
                    runs.push((span, candidates, subquery.paths));
                }
                let limit = frame_limit.min(self.config.output_k);
                let (mut candidates, paths) = mixed::merge(raw, &runs, limit);
                if candidates.len() < limit {
                    candidates.push(mixed::raw_candidate(raw));
                }
                diagnostic.statistics = statistics;
                diagnostic.paths = paths;
                mixed::Output {
                    candidates,
                    diagnostic: Some(diagnostic),
                    has_literal: true,
                    trailing_literal: has_tail,
                }
            }
        }
    }

    fn empty_diagnostic(&self, input: &str, revision: u64) -> SearchDiagnostic {
        SearchDiagnostic {
            revision,
            snapshot_version: self.snapshot.version.clone(),
            model_version: self.scoring_version().map(str::to_owned),
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
            scheme_version: (!self.scheme.is_default()).then(|| self.scheme.version_id()),
            scheme_statistics: None,
        }
    }

    fn query_with_config(
        &mut self,
        input: &str,
        revision: u64,
        limited: Option<&Config>,
    ) -> (Vec<Candidate>, SearchDiagnostic) {
        let mut diagnostic = self.empty_diagnostic(input, revision);
        let config = limited.unwrap_or(&self.config);
        let graph =
            if limited.is_some() || self.scheme.is_default() || !self.transitions.is_enabled() {
                Lattice::build_with_scheme(&self.snapshot.lexicon, input, config, &self.scheme)
            } else {
                Lattice::build_with_scheme_cached(
                    &self.snapshot.lexicon,
                    input,
                    config,
                    &self.scheme,
                    &mut self.transitions,
                )
            };
        let graph = match graph {
            Ok(graph) => graph,
            Err(error) => {
                diagnostic.error = Some(error);
                return (Vec::new(), diagnostic);
            }
        };
        diagnostic.scheme_statistics = graph.scheme_statistics().cloned();
        let result = match if let Some(research) = &self.research {
            research.rerank(&self.snapshot.lexicon, &graph, config)
        } else {
            search_graph(
                &self.snapshot.lexicon,
                &graph,
                config,
                self.language.as_ref().map(|value| value.snapshot.model()),
                self.left_word.as_deref(),
            )
        } {
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
                self.scoring_version(),
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
                        interpretation: graph.interpretation(*index).cloned(),
                    }
                })
                .collect();
            let frequency = if path.edge_indices.len() == 1 {
                self.snapshot.lexicon.entries()[graph.edges()[path.edge_indices[0]].entry].frequency
            } else {
                None
            };
            let match_kind = edges
                .iter()
                .map(|edge| match edge.match_kind {
                    MatchKind::Exact => cloudleaf_types::MatchKind::Exact,
                    MatchKind::Prefix => cloudleaf_types::MatchKind::Prefix,
                    MatchKind::Abbreviation => cloudleaf_types::MatchKind::Abbreviation,
                    MatchKind::Fuzzy => cloudleaf_types::MatchKind::Fuzzy,
                    MatchKind::Correction => cloudleaf_types::MatchKind::Correction,
                })
                .max()
                .unwrap_or(cloudleaf_types::MatchKind::Exact);
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
    if let Some(version) = graph.scheme_version() {
        hash.update(&(version.len() as u64).to_le_bytes());
        hash.update(version.as_bytes());
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
        if let Some(evidence) = graph.interpretation(*index) {
            let source = format!("{evidence:?}");
            hash.update(&(source.len() as u64).to_le_bytes());
            hash.update(source.as_bytes());
        }
    }
    hex(hash.finish().as_ref())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn entry_fingerprint(text: &str, pinyin: &str) -> String {
    let mut hash = Context::new(&SHA256);
    hash.update(text.as_bytes());
    hash.update(&[0]);
    hash.update(pinyin.as_bytes());
    hex(hash.finish().as_ref())
}
