//! 输入音节图与既有词典Trie相交，不枚举完整拼音字符串或复制中文词库。
use crate::lexicon::SyllableBoundaries;
use crate::{Budget, Config, Edge, ErrorKind, Lattice, Lexicon, MatchKind, SearchError};
use cloudleaf_pinyin::{
    BaseMatch, ChangeKind, PhoneticArc, PhoneticHypotheses, PinyinScheme, SchemeConfig, interpret,
};
use std::cmp::Reverse;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound::{Excluded, Unbounded};
use std::sync::OnceLock;

const MAX_WORD_ARCS: usize = 16;
// 搜索配置限制归一键数为 512；第 513 位是最后一个合法终点。
const SUFFIX_WORDS: usize = 9;

#[derive(Clone, Debug, Default)]
pub(crate) struct SuffixReachability([u64; SUFFIX_WORDS]);

impl SuffixReachability {
    pub(crate) fn contains(&self, position: usize) -> bool {
        self.0[position / 64] & (1 << (position % 64)) != 0
    }

    fn insert(&mut self, position: usize) {
        self.0[position / 64] |= 1 << (position % 64);
    }
}

#[derive(Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
struct ArcTrace {
    // 解释器最多保留 16,384 条弧。索引加一让零填充保持 Vec 的逐项顺序。
    encoded: [u16; MAX_WORD_ARCS],
    len: usize,
}

impl ArcTrace {
    fn appended(mut self, index: usize) -> Self {
        self.encoded[self.len] = u16::try_from(index + 1).expect("音节弧索引在解释预算内");
        self.len += 1;
        self
    }

    fn encoded(&self) -> &[u16] {
        &self.encoded[..self.len]
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct InterpretationCost {
    pub edits: u16,
    pub fuzzy: u16,
    pub abbreviations: u16,
    pub prefixes: u16,
}

impl InterpretationCost {
    pub fn plus(self, other: Self) -> Self {
        Self {
            edits: self.edits + other.edits,
            fuzzy: self.fuzzy + other.fuzzy,
            abbreviations: self.abbreviations + other.abbreviations,
            prefixes: self.prefixes + other.prefixes,
        }
    }
    fn arc(arc: &PhoneticArc) -> Self {
        let mut cost = Self {
            abbreviations: u16::from(arc.base == BaseMatch::Abbreviation),
            prefixes: u16::from(arc.base == BaseMatch::Prefix),
            ..Self::default()
        };
        for change in &arc.changes {
            if change.kind == ChangeKind::Fuzzy {
                cost.fuzzy += 1;
            } else {
                cost.edits += change.cost;
            }
        }
        cost
    }
    fn kind(self) -> MatchKind {
        if self.edits > 0 {
            MatchKind::Correction
        } else if self.fuzzy > 0 {
            MatchKind::Fuzzy
        } else if self.abbreviations > 0 {
            MatchKind::Abbreviation
        } else if self.prefixes > 0 {
            MatchKind::Prefix
        } else {
            MatchKind::Exact
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdgeInterpretation {
    pub cost: InterpretationCost,
    pub arcs: Vec<PhoneticArc>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchemeStatistics {
    pub phonetic_arcs: usize,
    pub interpreter_work: usize,
    pub intersection_states: usize,
    pub interpreter_budget: Option<cloudleaf_pinyin::InterpretationBudget>,
}

/// 显式剖析取得的候选格账本，触顶时只统计实际处理的起点和条目。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchemeGraphDiagnostics {
    pub dictionary_rows: usize,
    pub boundary_rejected: usize,
    pub duplicate_edges: usize,
    pub better_representatives: usize,
    pub first_rejected_start: Option<usize>,
    pub published_before_cap: usize,
    pub buffered_before_cap: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Interpreted {
    pub version: String,
    pub max_edits: u16,
    pub admission: SchemeAdmission,
    retained_suffix: Option<SuffixReachability>,
    phonetics: Vec<PhoneticArc>,
    source_indices: Vec<u16>,
    evidence: Vec<TracedInterpretation>,
    pub statistics: SchemeStatistics,
    pub diagnostics: Option<SchemeGraphDiagnostics>,
}

#[derive(Clone, Debug)]
struct TracedInterpretation {
    cost: InterpretationCost,
    start: u32,
    len: u8,
    materialized: OnceLock<Box<EdgeInterpretation>>,
}

impl Interpreted {
    fn push(&mut self, cost: InterpretationCost, source: &ArcTrace) {
        // 边最多 100,000 个，每边至多 16 个弧索引。
        let encoded = source.encoded();
        self.evidence.push(TracedInterpretation {
            cost,
            start: self
                .source_indices
                .len()
                .try_into()
                .expect("弧来源偏移受图预算约束"),
            len: encoded.len() as u8,
            materialized: OnceLock::new(),
        });
        self.source_indices.extend_from_slice(encoded);
    }

    fn evidence(&self, edge: usize) -> &EdgeInterpretation {
        let trace = &self.evidence[edge];
        trace
            .materialized
            .get_or_init(|| {
                let start = trace.start as usize;
                let end = start + usize::from(trace.len);
                Box::new(EdgeInterpretation {
                    cost: trace.cost,
                    arcs: self.source_indices[start..end]
                        .iter()
                        .map(|&index| self.phonetics[usize::from(index - 1)].clone())
                        .collect(),
                })
            })
            .as_ref()
    }
}

/// 诊断可比较旧顺序和只分摊容量的对照，输入引擎按输入方案选择。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemeAdmission {
    Forward,
    Balanced,
    ReachableSuffix,
}

impl SchemeAdmission {
    pub(crate) fn for_scheme(scheme: &SchemeConfig) -> Self {
        if scheme.scheme == PinyinScheme::AbbreviatedPinyin && scheme.correction.is_none() {
            Self::ReachableSuffix
        } else {
            Self::Forward
        }
    }
}

type WordKey = (usize, usize);
type WordTrace = (InterpretationCost, ArcTrace);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AdmissionRank {
    missing_suffix: bool,
    cost: InterpretationCost,
    covered: Reverse<usize>,
    key: WordKey,
}

impl AdmissionRank {
    fn new(start: usize, key: WordKey, value: WordTrace, suffix: &[bool]) -> Self {
        Self {
            missing_suffix: !suffix[key.0],
            cost: value.0,
            covered: Reverse(key.0 - start),
            key,
        }
    }
}

#[derive(Default)]
struct WordBucket {
    rows: BTreeMap<WordKey, WordTrace>,
    ranks: Option<BTreeSet<AdmissionRank>>,
}

impl WordBucket {
    fn rank_rows(&mut self, start: usize, suffix: &[bool]) {
        self.ranks.get_or_insert_with(|| {
            self.rows
                .iter()
                .map(|(&key, &trace)| AdmissionRank::new(start, key, trace, suffix))
                .collect()
        });
    }

    fn set(&mut self, start: usize, key: WordKey, value: WordTrace, suffix: &[bool]) {
        let previous = self.rows.insert(key, value);
        if let Some(ranks) = &mut self.ranks {
            if let Some(old) = previous {
                if old.0 == value.0 {
                    return;
                }
                ranks.remove(&AdmissionRank::new(start, key, old, suffix));
            }
            ranks.insert(AdmissionRank::new(start, key, value, suffix));
        }
    }
}

struct AdmissionTable {
    buckets: Vec<WordBucket>,
    suffix: Vec<bool>,
    occupied: usize,
    capacity: usize,
    priority: Option<BTreeSet<(usize, AdmissionRank, usize)>>,
}

impl AdmissionTable {
    fn new(positions: usize, capacity: usize) -> Self {
        let mut suffix = vec![false; positions + 1];
        suffix[positions] = true;
        Self {
            buckets: (0..positions).map(|_| WordBucket::default()).collect(),
            suffix,
            occupied: 0,
            capacity,
            priority: None,
        }
    }

    fn rank(bucket: &WordBucket, start: usize) -> Option<(usize, AdmissionRank, usize)> {
        bucket
            .ranks
            .as_ref()
            .and_then(|ranks| ranks.last())
            .map(|&worst| (bucket.rows.len(), worst, start))
    }

    fn set(&mut self, start: usize, key: WordKey, value: WordTrace) {
        let bucket = &mut self.buckets[start];
        let before = Self::rank(bucket, start);
        bucket.set(start, key, value, &self.suffix);
        if let Some(priority) = &mut self.priority {
            let after = Self::rank(bucket, start).expect("本轮写入了一条边");
            if before != Some(after) {
                if let Some(before) = before {
                    priority.remove(&before);
                }
                priority.insert(after);
            }
        }
    }

    fn admit(&mut self, start: usize, key: WordKey, value: WordTrace) {
        if self.occupied < self.capacity {
            self.occupied += 1;
        } else {
            let priority = self.priority.get_or_insert_with(|| {
                self.buckets
                    .iter_mut()
                    .enumerate()
                    .filter_map(|(position, bucket)| {
                        bucket.rank_rows(position, &self.suffix);
                        Self::rank(bucket, position)
                    })
                    .collect()
            });
            let arrival = AdmissionRank::new(start, key, value, &self.suffix);
            let current = &self.buckets[start];
            let count = current.rows.len();
            let local = Self::rank(current, start);
            let &(size, rank, victim) = priority.last().expect("已满的候选格至少有一个词条");
            let displaced = if victim != start && (count + 1, arrival) < (size, rank) {
                (size, rank, victim)
            } else if let Some(local) = local.filter(|&(_, worst, _)| arrival < worst) {
                local
            } else {
                return;
            };
            let (_, worst, position) = displaced;
            let bucket = &mut self.buckets[position];
            priority.remove(&displaced);
            bucket.rows.remove(&worst.key);
            bucket.ranks.as_mut().unwrap().remove(&worst);
            if position == start {
                bucket.set(start, key, value, &self.suffix);
                priority.insert(Self::rank(bucket, start).expect("本轮写入了一条边"));
                return;
            }
            if let Some(rank) = Self::rank(bucket, position) {
                priority.insert(rank);
            }
        }
        self.set(start, key, value);
    }

    fn retained_suffix(&self) -> SuffixReachability {
        let mut retained = SuffixReachability::default();
        retained.insert(self.buckets.len());
        for (start, bucket) in self.buckets.iter().enumerate().rev() {
            let Some((&(mut end, _), _)) = bucket.rows.last_key_value() else {
                continue;
            };
            loop {
                if retained.contains(end) {
                    retained.insert(start);
                    break;
                }
                // 同一终点可以有多个同音词，判断后缀时跳过该终点的所有词条。
                let Some((&(previous, _), _)) = bucket
                    .rows
                    .range((Unbounded, Excluded((end, 0))))
                    .next_back()
                else {
                    break;
                };
                end = previous;
            }
        }
        retained
    }
}

struct IntersectionState {
    node: usize,
    position: usize,
    canonical_len: usize,
    boundaries: SyllableBoundaries,
    arcs: ArcTrace,
    cost: InterpretationCost,
}

impl<'a> Lattice<'a> {
    pub fn build_with_scheme(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
    ) -> Result<Self, SearchError> {
        Self::build_with_scheme_controlled(lexicon, raw, config, scheme, None)
    }

    pub fn build_with_scheme_cached(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
        cache: &mut crate::TransitionCache,
    ) -> Result<Self, SearchError> {
        Self::build_with_scheme_controlled(lexicon, raw, config, scheme, Some(cache))
    }

    fn build_with_scheme_controlled(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
        cache: Option<&mut crate::TransitionCache>,
    ) -> Result<Self, SearchError> {
        if scheme.is_default() {
            return Self::build(lexicon, raw, config);
        }
        config.validate()?;
        if raw.len() > config.max_input_bytes {
            return Err(SearchError::new(
                ErrorKind::InvalidInput,
                "原始输入超过搜索预算",
            ));
        }
        let hypotheses = interpret(raw, scheme)
            .map_err(|_| SearchError::new(ErrorKind::InvalidInput, "输入方案配置或按键无效"))?;
        Self::from_phonetic_controlled::<false>(
            lexicon,
            raw,
            config,
            scheme,
            hypotheses,
            cache,
            SchemeAdmission::for_scheme(scheme),
        )
    }

    pub(crate) fn from_phonetic(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
        hypotheses: PhoneticHypotheses,
        admission: SchemeAdmission,
    ) -> Result<Self, SearchError> {
        Self::from_phonetic_controlled::<true>(
            lexicon, raw, config, scheme, hypotheses, None, admission,
        )
    }

    fn from_phonetic_controlled<const DIAGNOSTICS: bool>(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
        hypotheses: PhoneticHypotheses,
        mut cache: Option<&mut crate::TransitionCache>,
        admission: SchemeAdmission,
    ) -> Result<Self, SearchError> {
        if hypotheses.keys.len() > config.max_letters {
            return Err(SearchError::new(
                ErrorKind::InvalidInput,
                "归一按键超过搜索预算",
            ));
        }
        if let Some(cache) = cache.as_mut() {
            cache.bind(lexicon);
        }
        let max_edits = scheme.correction.as_ref().map_or(0, |c| c.max_edit_cost);
        let mut graph = Self {
            lexicon,
            raw: raw.into(),
            letters: hypotheses.keys,
            raw_offsets: hypotheses.raw_offsets,
            waiting: hypotheses.waiting,
            outgoing: Vec::new(),
            edges: Vec::new(),
            budget: hypotheses.budget.map(|_| Budget::Interpretation),
            interpreted: Some(Interpreted {
                version: scheme.version_id(),
                max_edits,
                admission,
                retained_suffix: None,
                phonetics: Vec::new(),
                source_indices: Vec::new(),
                evidence: Vec::new(),
                statistics: SchemeStatistics {
                    phonetic_arcs: hypotheses.arcs.len(),
                    interpreter_work: hypotheses.work,
                    intersection_states: 0,
                    interpreter_budget: hypotheses.budget,
                },
                diagnostics: DIAGNOSTICS.then(SchemeGraphDiagnostics::default),
            }),
        };
        graph.outgoing = vec![Vec::new(); graph.letters.len() + 1];
        if graph.budget.is_some() || graph.waiting {
            return Ok(graph);
        }
        // 解释器已按起点排序，连续的音节弧可用一对偏移定位。
        let mut arcs_at = vec![0; graph.outgoing.len()];
        for arc in &hypotheses.arcs {
            arcs_at[arc.start + 1] += 1;
        }
        for index in 1..arcs_at.len() {
            arcs_at[index] += arcs_at[index - 1];
        }
        let arc_costs: Vec<_> = hypotheses
            .arcs
            .iter()
            .map(InterpretationCost::arc)
            .collect();
        let mut visits = 0;
        let mut diagnostics = SchemeGraphDiagnostics::default();
        // 相交预算独立于搜索展开，原搜索上限不因方案扩展而增加。
        let max_states = config.max_expansions;
        let position_count = graph.letters.len();
        let mut table = AdmissionTable::new(position_count, config.max_edges);
        let mut unfinished = None;
        let mut pending = Vec::new();
        'starts: for index in 0..position_count {
            let start = if admission == SchemeAdmission::ReachableSuffix {
                position_count - index - 1
            } else {
                index
            };
            unfinished = Some(start);
            pending.clear();
            pending.push(IntersectionState {
                node: 0,
                position: start,
                canonical_len: 0,
                boundaries: SyllableBoundaries::default(),
                arcs: ArcTrace::default(),
                cost: InterpretationCost::default(),
            });
            let mut seen = BTreeMap::new();
            let mut first_seen = DIAGNOSTICS.then(BTreeMap::<WordKey, WordTrace>::new);
            'intersection: while let Some(state) = pending.pop() {
                visits += 1;
                if visits > max_states {
                    graph.budget = Some(Budget::Intersection);
                    break 'starts;
                }
                let start_arc = arcs_at[state.position];
                let end_arc = arcs_at[state.position + 1];
                let costs = &arc_costs[start_arc..end_arc];
                let arcs = &hypotheses.arcs[start_arc..end_arc];
                for (offset, (arc, &arc_cost)) in arcs.iter().zip(costs).enumerate() {
                    visits += 1;
                    if visits > max_states {
                        graph.budget = Some(Budget::Intersection);
                        break 'starts;
                    }
                    let arc_index = start_arc + offset;
                    let cost = state.cost.plus(arc_cost);
                    if cost.edits > max_edits {
                        continue;
                    }
                    let follow = || {
                        let mut node = state.node;
                        for key in arc.syllable.bytes() {
                            node = *lexicon.trie[node].next.get(&key)?;
                        }
                        Some(node)
                    };
                    let target = if let Some(cache) = cache.as_mut() {
                        cache.follow(lexicon, state.node, &arc.syllable)
                    } else {
                        follow()
                    };
                    let Some(node) = target else {
                        continue;
                    };
                    let trie_node = &lexicon.trie[node];
                    let source_arcs = state.arcs.appended(arc_index);
                    let canonical_len = state.canonical_len + arc.syllable.len();
                    for &entry in &trie_node.exact {
                        if DIAGNOSTICS {
                            diagnostics.dictionary_rows += 1;
                        }
                        let word = &lexicon.entries[entry];
                        if word.boundaries != state.boundaries {
                            if DIAGNOSTICS {
                                diagnostics.boundary_rejected += 1;
                            }
                            continue;
                        }
                        let key = (arc.end, entry);
                        let value = (cost, source_arcs);
                        if let Some(first_seen) = &mut first_seen {
                            match first_seen.entry(key) {
                                Entry::Occupied(mut old) => {
                                    diagnostics.duplicate_edges += 1;
                                    if value < *old.get() {
                                        diagnostics.better_representatives += 1;
                                        old.insert(value);
                                    }
                                }
                                Entry::Vacant(slot) => {
                                    slot.insert(value);
                                }
                            }
                        }
                        if let Some(&previous) = table.buckets[start].rows.get(&key) {
                            if value < previous {
                                table.set(start, key, value);
                            }
                            continue;
                        }
                        if table.occupied == config.max_edges {
                            if DIAGNOSTICS && diagnostics.first_rejected_start.is_none() {
                                let buffered = table.buckets[start].rows.len();
                                diagnostics.first_rejected_start = Some(start);
                                diagnostics.published_before_cap = table.occupied - buffered;
                                diagnostics.buffered_before_cap = buffered;
                            }
                            graph.budget = Some(Budget::GraphEdges);
                            if admission == SchemeAdmission::Forward {
                                break 'intersection;
                            }
                        }
                        table.admit(start, key, value);
                        if admission == SchemeAdmission::ReachableSuffix {
                            table.suffix[start] |= table.suffix[arc.end];
                        }
                    }
                    // Trie 叶节点无法再接受任何完整音节，跳过已证实无后继的状态。
                    if arc.end < position_count
                        && source_arcs.len < MAX_WORD_ARCS
                        && (admission == SchemeAdmission::Forward || !trie_node.next.is_empty())
                    {
                        let mut boundaries = state.boundaries;
                        boundaries.insert(canonical_len);
                        let value = (cost, source_arcs);
                        let key = (arc.end, node, cost.edits, boundaries);
                        match seen.entry(key) {
                            Entry::Occupied(old) if value >= *old.get() => continue,
                            Entry::Occupied(mut old) => {
                                old.insert(value);
                            }
                            Entry::Vacant(slot) => {
                                slot.insert(value);
                            }
                        }
                        if pending.len() >= config.max_resident_paths {
                            graph.budget = Some(Budget::Intersection);
                            break 'starts;
                        }
                        pending.push(IntersectionState {
                            node,
                            position: arc.end,
                            canonical_len,
                            boundaries,
                            arcs: source_arcs,
                            cost,
                        });
                    }
                }
            }
            unfinished = None;
            if graph.budget.is_some() && admission == SchemeAdmission::Forward {
                break;
            }
        }
        if admission == SchemeAdmission::Forward
            && let Some(start) = unfinished
        {
            // 旧相交预算只发布处理完整的起点；图边触顶已在内部退出。
            table.buckets[start].rows.clear();
        }
        if graph.has_reachable_suffix_graph_cap() {
            let interpreted = graph.interpreted.as_mut().unwrap();
            interpreted.retained_suffix = Some(table.retained_suffix());
            // 触顶的逆序图已知保留边数，发布时一次预留对应空间。
            graph.edges.reserve_exact(table.occupied);
            interpreted.evidence.reserve_exact(table.occupied);
        }
        let reserve_outgoing = graph.has_reachable_suffix_graph_cap();
        for (start, bucket) in table.buckets.into_iter().enumerate() {
            if reserve_outgoing {
                graph.outgoing[start].reserve_exact(bucket.rows.len());
            }
            for ((end, entry), (cost, source_arcs)) in bucket.rows {
                let index = graph.edges.len();
                graph.edges.push(Edge {
                    start,
                    end,
                    raw_start: graph.raw_offsets[start],
                    raw_end: graph.raw_offsets[end],
                    entry,
                    kind: cost.kind(),
                });
                graph.interpreted.as_mut().unwrap().push(cost, &source_arcs);
                graph.outgoing[start].push(index);
            }
        }
        let interpreted = graph.interpreted.as_mut().unwrap();
        interpreted.phonetics = hypotheses.arcs;
        interpreted.statistics.intersection_states = visits;
        if DIAGNOSTICS {
            interpreted.diagnostics = Some(diagnostics);
        }
        Ok(graph)
    }

    pub fn interpretation(&self, edge: usize) -> Option<&EdgeInterpretation> {
        self.interpreted.as_ref().map(|s| s.evidence(edge))
    }
    pub fn interpretation_cost(&self, edge: usize) -> InterpretationCost {
        self.interpreted
            .as_ref()
            .map_or(InterpretationCost::default(), |s| s.evidence[edge].cost)
    }
    pub fn is_interpreted(&self) -> bool {
        self.interpreted.is_some()
    }
    pub fn max_edit_cost(&self) -> u16 {
        self.interpreted.as_ref().map_or(0, |s| s.max_edits)
    }
    pub fn scheme_version(&self) -> Option<&str> {
        self.interpreted.as_ref().map(|s| s.version.as_str())
    }
    pub fn scheme_statistics(&self) -> Option<&SchemeStatistics> {
        self.interpreted.as_ref().map(|s| &s.statistics)
    }

    pub fn scheme_graph_diagnostics(&self) -> Option<&SchemeGraphDiagnostics> {
        self.interpreted.as_ref()?.diagnostics.as_ref()
    }

    pub(crate) fn retained_suffix(&self) -> Option<&SuffixReachability> {
        self.interpreted.as_ref()?.retained_suffix.as_ref()
    }

    /// 继续报告图边截断；其保留的完整路径可供按位置搜索使用。
    pub fn has_reachable_suffix_graph_cap(&self) -> bool {
        self.budget == Some(Budget::GraphEdges)
            && self
                .interpreted
                .as_ref()
                .is_some_and(|graph| graph.admission == SchemeAdmission::ReachableSuffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudleaf_pinyin::PinyinScheme;

    fn paths_to_end(rows: &[Vec<WordKey>], start: usize) -> bool {
        if start == rows.len() {
            return true;
        }
        rows[start].iter().any(|&(end, _)| paths_to_end(rows, end))
    }

    #[test]
    fn retained_suffix_matches_exhaustive_paths_after_capacity_evictions() {
        let pairs: Vec<_> = (0..4)
            .flat_map(|start| (start + 1..=4).map(move |end| (start, end)))
            .collect();
        let mut evictions_invalidate_discovery = 0;
        let mut dead_end_partials = 0;
        for mask in 0..1 << pairs.len() {
            for capacity in [1, 3, 7, 15, 31] {
                let mut table = AdmissionTable::new(4, capacity);
                for (pair, &(start, end)) in pairs.iter().enumerate().rev() {
                    if mask & (1 << pair) == 0 {
                        continue;
                    }
                    for row in 0..3 {
                        table.admit(start, (end, pair * 3 + row), WordTrace::default());
                    }
                    table.suffix[start] |= table.suffix[end];
                }
                let rows: Vec<Vec<_>> = table
                    .buckets
                    .iter()
                    .map(|bucket| bucket.rows.keys().copied().collect())
                    .collect();
                let retained = table.retained_suffix();
                for start in 0..=rows.len() {
                    assert_eq!(
                        retained.contains(start),
                        paths_to_end(&rows, start),
                        "mask={mask} capacity={capacity} start={start}"
                    );
                    if table.suffix[start] && !retained.contains(start) {
                        evictions_invalidate_discovery += 1;
                    }
                }
                for group in &rows {
                    dead_end_partials += group
                        .iter()
                        .filter(|&&(end, _)| !retained.contains(end))
                        .count();
                }
            }
        }
        assert!(evictions_invalidate_discovery > 0);
        assert!(dead_end_partials > 0);
        let last = AdmissionTable::new(512, 1).retained_suffix();
        assert!(last.contains(512));
        assert!(!last.contains(511));
    }

    #[test]
    fn abbreviations_materialize_only_the_requested_edge_with_its_original_boundaries() {
        let lexicon = Lexicon::from_tsv(concat!(
            "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n",
            "a\txian\t先\t\tunknown\t\tfixture\ta\tMIT\n",
            "b\txi an\t西安\t\tunknown\t\tfixture\tb\tMIT\n",
        ))
        .unwrap();
        let scheme = SchemeConfig {
            scheme: PinyinScheme::AbbreviatedPinyin,
            ..SchemeConfig::default()
        };
        let graph =
            Lattice::build_with_scheme(&lexicon, "xa", &Config::default(), &scheme).unwrap();
        let edge = graph
            .edges
            .iter()
            .position(|edge| edge.start == 0 && edge.end == 2 && edge.entry == 1)
            .unwrap();
        let traces = &graph.interpreted.as_ref().unwrap().evidence;
        assert!(traces.len() > 1);
        assert_eq!(graph.interpretation_cost(edge).abbreviations, 1);
        assert!(
            traces
                .iter()
                .all(|trace| trace.materialized.get().is_none())
        );
        let evidence = graph.interpretation(edge).unwrap();
        assert_eq!(evidence.cost, graph.interpretation_cost(edge));
        assert_eq!(
            evidence
                .arcs
                .iter()
                .map(|arc| (arc.start, arc.end, arc.syllable.as_str()))
                .collect::<Vec<_>>(),
            [(0, 1, "xi"), (1, 2, "an")]
        );
        assert!(std::ptr::eq(evidence, graph.interpretation(edge).unwrap()));
        assert_eq!(
            traces
                .iter()
                .filter(|trace| trace.materialized.get().is_some())
                .count(),
            1
        );
    }
}
