//! 输入音节图与既有词典Trie相交，不枚举完整拼音字符串或复制中文词库。
use crate::lexicon::SyllableBoundaries;
use crate::{Budget, Config, Edge, ErrorKind, Lattice, Lexicon, MatchKind, SearchError};
use cloudleaf_pinyin::{
    BaseMatch, ChangeKind, PhoneticArc, PhoneticHypotheses, SchemeConfig, interpret,
};
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::OnceLock;

const MAX_WORD_ARCS: usize = 16;

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
        Self::from_phonetic_controlled::<false>(lexicon, raw, config, scheme, hypotheses, cache)
    }

    pub(crate) fn from_phonetic(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
        hypotheses: PhoneticHypotheses,
    ) -> Result<Self, SearchError> {
        Self::from_phonetic_controlled::<true>(lexicon, raw, config, scheme, hypotheses, None)
    }

    fn from_phonetic_controlled<const DIAGNOSTICS: bool>(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
        hypotheses: PhoneticHypotheses,
        mut cache: Option<&mut crate::TransitionCache>,
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
        let mut arcs_at = vec![Vec::new(); graph.outgoing.len()];
        for (index, arc) in hypotheses.arcs.iter().enumerate() {
            arcs_at[arc.start].push((index, InterpretationCost::arc(arc)));
        }
        let mut visits = 0;
        let mut diagnostics = SchemeGraphDiagnostics::default();
        // 相交预算独立于搜索展开，原搜索上限不因方案扩展而增加。
        let max_states = config.max_expansions;
        'starts: for start in 0..graph.letters.len() {
            let mut pending = vec![IntersectionState {
                node: 0,
                position: start,
                canonical_len: 0,
                boundaries: SyllableBoundaries::default(),
                arcs: ArcTrace::default(),
                cost: InterpretationCost::default(),
            }];
            let mut seen = BTreeMap::new();
            let mut words = BTreeMap::<(usize, usize), (InterpretationCost, ArcTrace)>::new();
            'intersection: while let Some(state) = pending.pop() {
                visits += 1;
                if visits > max_states {
                    graph.budget = Some(Budget::Intersection);
                    break 'starts;
                }
                for &(arc_index, arc_cost) in &arcs_at[state.position] {
                    visits += 1;
                    if visits > max_states {
                        graph.budget = Some(Budget::Intersection);
                        break 'starts;
                    }
                    let arc = &hypotheses.arcs[arc_index];
                    let cost = state.cost.plus(arc_cost);
                    if cost.edits > max_edits {
                        continue;
                    }
                    let node = if let Some(cache) = cache.as_mut() {
                        let Some(node) = cache.follow(lexicon, state.node, &arc.syllable) else {
                            continue;
                        };
                        node
                    } else {
                        let mut node = state.node;
                        let mut found = true;
                        for key in arc.syllable.bytes() {
                            if let Some(&next) = lexicon.trie[node].next.get(&key) {
                                node = next;
                            } else {
                                found = false;
                                break;
                            }
                        }
                        if !found {
                            continue;
                        }
                        node
                    };
                    let source_arcs = state.arcs.appended(arc_index);
                    let canonical_len = state.canonical_len + arc.syllable.len();
                    for &entry in &lexicon.trie[node].exact {
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
                        let buffered = words.len();
                        let value = (cost, source_arcs);
                        match words.entry((arc.end, entry)) {
                            Entry::Occupied(mut old) => {
                                if DIAGNOSTICS {
                                    diagnostics.duplicate_edges += 1;
                                }
                                if value < *old.get() {
                                    if DIAGNOSTICS {
                                        diagnostics.better_representatives += 1;
                                    }
                                    old.insert(value);
                                }
                            }
                            Entry::Vacant(slot) => {
                                if buffered + graph.edges.len() == config.max_edges {
                                    if DIAGNOSTICS {
                                        diagnostics.first_rejected_start = Some(start);
                                        diagnostics.published_before_cap = graph.edges.len();
                                        diagnostics.buffered_before_cap = buffered;
                                    }
                                    graph.budget = Some(Budget::GraphEdges);
                                    break 'intersection;
                                }
                                slot.insert(value);
                            }
                        }
                    }
                    if arc.end < graph.letters.len() && source_arcs.len < MAX_WORD_ARCS {
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
            for ((end, entry), (cost, source_arcs)) in words {
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
            if graph.budget.is_some() {
                break;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudleaf_pinyin::PinyinScheme;

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
