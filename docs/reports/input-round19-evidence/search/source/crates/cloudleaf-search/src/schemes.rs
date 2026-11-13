//! 输入音节图与既有词典Trie相交，不枚举完整拼音字符串或复制中文词库。
use crate::{Budget, Config, Edge, ErrorKind, Lattice, Lexicon, MatchKind, SearchError};
use cloudleaf_pinyin::{
    BaseMatch, ChangeKind, PhoneticArc, PhoneticHypotheses, SchemeConfig, interpret,
};
use std::collections::BTreeMap;

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

#[derive(Clone, Debug)]
pub(crate) struct Interpreted {
    pub version: String,
    pub max_edits: u16,
    pub evidence: Vec<EdgeInterpretation>,
    pub statistics: SchemeStatistics,
}

#[derive(Clone)]
struct IntersectionState {
    node: usize,
    position: usize,
    canonical_len: usize,
    boundaries: Vec<usize>,
    arcs: Vec<usize>,
    cost: InterpretationCost,
}

impl<'a> Lattice<'a> {
    pub fn build_with_scheme(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
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
        Self::from_phonetic(lexicon, raw, config, scheme, hypotheses)
    }

    pub(crate) fn from_phonetic(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
        hypotheses: PhoneticHypotheses,
    ) -> Result<Self, SearchError> {
        if hypotheses.keys.len() > config.max_letters {
            return Err(SearchError::new(
                ErrorKind::InvalidInput,
                "归一按键超过搜索预算",
            ));
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
                evidence: Vec::new(),
                statistics: SchemeStatistics {
                    phonetic_arcs: hypotheses.arcs.len(),
                    interpreter_work: hypotheses.work,
                    intersection_states: 0,
                    interpreter_budget: hypotheses.budget,
                },
            }),
        };
        graph.outgoing = vec![Vec::new(); graph.letters.len() + 1];
        if graph.budget.is_some() || graph.waiting {
            return Ok(graph);
        }
        let mut arcs_at = vec![Vec::new(); graph.outgoing.len()];
        for (index, arc) in hypotheses.arcs.iter().enumerate() {
            arcs_at[arc.start].push(index);
        }
        let mut visits = 0;
        // 相交预算独立于搜索展开，原搜索上限不因方案扩展而增加。
        let max_states = config.max_expansions;
        'starts: for start in 0..graph.letters.len() {
            let mut pending = vec![IntersectionState {
                node: 0,
                position: start,
                canonical_len: 0,
                boundaries: Vec::new(),
                arcs: Vec::new(),
                cost: InterpretationCost::default(),
            }];
            let mut seen = BTreeMap::new();
            let mut words = BTreeMap::<(usize, usize), (InterpretationCost, Vec<usize>)>::new();
            while let Some(state) = pending.pop() {
                visits += 1;
                if visits > max_states {
                    graph.budget = Some(Budget::Intersection);
                    break 'starts;
                }
                for &arc_index in &arcs_at[state.position] {
                    visits += 1;
                    if visits > max_states {
                        graph.budget = Some(Budget::Intersection);
                        break 'starts;
                    }
                    let arc = &hypotheses.arcs[arc_index];
                    let cost = state.cost.plus(InterpretationCost::arc(arc));
                    if cost.edits > max_edits {
                        continue;
                    }
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
                    let mut source_arcs = state.arcs.clone();
                    source_arcs.push(arc_index);
                    let canonical_len = state.canonical_len + arc.syllable.len();
                    for &entry in &lexicon.trie[node].exact {
                        let word = &lexicon.entries[entry];
                        if !word
                            .boundaries
                            .iter()
                            .copied()
                            .eq(state.boundaries.iter().copied())
                        {
                            continue;
                        }
                        let key = (arc.end, entry);
                        let value = (cost, source_arcs.clone());
                        if !words.contains_key(&key)
                            && words.len() + graph.edges.len() == config.max_edges
                        {
                            graph.budget = Some(Budget::GraphEdges);
                            break 'starts;
                        }
                        if words.get(&key).is_none_or(|old| value < *old) {
                            words.insert(key, value);
                        }
                    }
                    if arc.end < graph.letters.len() && source_arcs.len() < 16 {
                        let mut boundaries = state.boundaries.clone();
                        boundaries.push(canonical_len);
                        let key = (arc.end, node, cost.edits, boundaries.clone());
                        let value = (cost, source_arcs.clone());
                        if seen.get(&key).is_none_or(|old| value < *old) {
                            if pending.len() >= config.max_resident_paths {
                                graph.budget = Some(Budget::Intersection);
                                break 'starts;
                            }
                            seen.insert(key, value);
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
            }
            for ((end, entry), (cost, source_arcs)) in words {
                if graph.edges.len() == config.max_edges {
                    graph.budget = Some(Budget::GraphEdges);
                    break 'starts;
                }
                let index = graph.edges.len();
                graph.edges.push(Edge {
                    start,
                    end,
                    raw_start: graph.raw_offsets[start],
                    raw_end: graph.raw_offsets[end],
                    entry,
                    kind: cost.kind(),
                });
                graph
                    .interpreted
                    .as_mut()
                    .unwrap()
                    .evidence
                    .push(EdgeInterpretation {
                        cost,
                        arcs: source_arcs
                            .iter()
                            .map(|&i| hypotheses.arcs[i].clone())
                            .collect(),
                    });
                graph.outgoing[start].push(index);
            }
        }
        graph
            .interpreted
            .as_mut()
            .unwrap()
            .statistics
            .intersection_states = visits;
        Ok(graph)
    }

    pub fn interpretation(&self, edge: usize) -> Option<&EdgeInterpretation> {
        self.interpreted.as_ref().map(|s| &s.evidence[edge])
    }
    pub fn interpretation_cost(&self, edge: usize) -> InterpretationCost {
        self.interpretation(edge)
            .map_or(InterpretationCost::default(), |v| v.cost)
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
}
