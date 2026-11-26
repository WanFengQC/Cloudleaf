use super::{
    Noop, Path, Scorer, compare, expand_allowed, resident_allowed, search_graph_cancellable,
};
use crate::{
    BigramModel, Budget, Cancellation, Candidate, Config, ErrorKind, Lattice, Method, Ranker,
    SearchError, Statistics, WordContextModel,
};
use std::collections::{BTreeMap, BTreeSet};

const MAX_EXPANSIONS: usize = 200_000;
const MAX_RESIDENT: usize = 50_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecallCandidate {
    pub path: Candidate,
    /// 字符、词汇、结构三个来源的原始名次。上下文搜索没有分池名次。
    pub source_ranks: [Option<usize>; 3],
    pub word_gain: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecallResult {
    pub candidates: Vec<RecallCandidate>,
    pub raw_fallback: String,
    pub waiting: bool,
    pub exhaustive_for_score: bool,
    pub work: Statistics,
    /// 分池方案保留各次搜索的截断事实，预算在调用前分配。
    pub source_work: Vec<Statistics>,
}

fn validate(
    graph: &Lattice<'_>,
    config: &Config,
    character: &BigramModel,
) -> Result<(), SearchError> {
    config.validate()?;
    if config.max_expansions > MAX_EXPANSIONS
        || config.max_resident_paths > MAX_RESIDENT
        || graph.raw().len() > config.max_input_bytes
        || graph.letters().len() > config.max_letters
        || graph.edges().len() > config.max_edges
    {
        return Err(SearchError::new(
            ErrorKind::InvalidConfig,
            "研发召回工作预算越界",
        ));
    }
    if !character.is_compatible_lexicon(graph.lexicon) || !character.has_character_transitions() {
        return Err(SearchError::new(
            ErrorKind::InvalidModel,
            "研发召回需要已绑定字符模型",
        ));
    }
    Ok(())
}

fn initial(graph: &Lattice<'_>) -> RecallResult {
    RecallResult {
        candidates: Vec::new(),
        raw_fallback: graph.raw().to_owned(),
        waiting: graph.waiting(),
        exhaustive_for_score: false,
        work: Statistics {
            graph_edges: graph.edges().len(),
            budget: graph.budget(),
            ..Statistics::default()
        },
        source_work: Vec::new(),
    }
}

/// 多来源按固定份额执行，交错时移交路径所有权。
pub fn recall_interleaved(
    graph: &Lattice<'_>,
    config: &Config,
    character: &BigramModel,
    lexical: &BigramModel,
    cancellation: &Cancellation,
) -> Result<RecallResult, SearchError> {
    cancellation.check()?;
    validate(graph, config, character)?;
    if !lexical.is_compatible_lexicon(graph.lexicon) || !lexical.has_lexical_signal() {
        return Err(SearchError::new(
            ErrorKind::InvalidModel,
            "研发召回需要已绑定词汇模型",
        ));
    }

    let mut result = initial(graph);
    let mut retained = 0;
    let mut pools = Vec::with_capacity(3);
    let char_share = config.max_expansions * 65 / 100;
    let lex_share = config.max_expansions * 25 / 100;
    for (ranker, method, model, quota) in [
        (
            Ranker::CharacterBigram,
            Method::ExactDag,
            Some(character),
            char_share,
        ),
        (
            Ranker::Lexical,
            Method::PositionDag,
            Some(lexical),
            lex_share,
        ),
        (
            Ranker::Structural,
            Method::PositionDag,
            None,
            config.max_expansions - char_share - lex_share,
        ),
    ] {
        let resident = config.max_resident_paths - retained;
        let (mut budget, mut paths) = if quota == 0 || resident == 0 {
            (
                Statistics {
                    graph_edges: graph.edges().len(),
                    budget: Some(if quota == 0 {
                        Budget::Expansions
                    } else {
                        Budget::ResidentPaths
                    }),
                    ..Statistics::default()
                },
                Vec::new(),
            )
        } else {
            let source = Config {
                ranker,
                method,
                max_expansions: quota,
                max_resident_paths: resident,
                ..config.clone()
            };
            let searched =
                search_graph_cancellable(graph.lexicon, graph, &source, model, None, cancellation)?;
            (searched.statistics, searched.candidates)
        };
        if paths.len() > resident {
            paths.truncate(resident);
            budget.budget = Some(Budget::ResidentPaths);
        }
        result.work.resident_high_water = result
            .work
            .resident_high_water
            .max(retained + budget.resident_high_water.max(paths.len()));
        retained += paths.len();
        result.work.expansions += budget.expansions;
        result.work.beam_pruned += budget.beam_pruned;
        result.work.budget = result.work.budget.or(budget.budget);
        result.source_work.push(budget);
        pools.push(paths.into_iter().enumerate());
    }

    let mut positions = BTreeMap::new();
    for _ in 0..config.output_k {
        cancellation.check()?;
        for (source, pool) in pools.iter_mut().enumerate() {
            let Some((rank, path)) = pool.next() else {
                continue;
            };
            if let Some(&index) = positions.get(&path.text) {
                let item: &mut RecallCandidate = &mut result.candidates[index];
                item.source_ranks[source] = Some(rank + 1);
                if prefer_interpretation(&path, &item.path).is_lt() {
                    item.path = path;
                }
            } else {
                positions.insert(path.text.clone(), result.candidates.len());
                let mut source_ranks = [None; 3];
                source_ranks[source] = Some(rank + 1);
                result.candidates.push(RecallCandidate {
                    path,
                    source_ranks,
                    word_gain: 0,
                });
            }
        }
    }
    // 稳定排序在同一解释成本及用户优先级内保留交错顺序。
    result
        .candidates
        .sort_by(|left, right| prefer_interpretation(&left.path, &right.path));
    result.candidates.truncate(config.output_k);
    cancellation.check()?;
    Ok(result)
}

fn prefer_interpretation(left: &Candidate, right: &Candidate) -> std::cmp::Ordering {
    left.features
        .interpretation_cost
        .cmp(&right.features.interpretation_cost)
        .then_with(|| {
            right
                .features
                .user_preference_units
                .cmp(&left.features.user_preference_units)
        })
}

#[derive(Default)]
struct ContextPath {
    path: Path,
    previous: u32,
    gain: i64,
}

fn compare_context(left: &ContextPath, right: &ContextPath) -> std::cmp::Ordering {
    compare(&left.path, &right.path).then_with(|| left.path.edges.cmp(&right.path.edges))
}

fn accept(paths: &mut Vec<ContextPath>, next: ContextPath, k: usize, unique: bool) -> bool {
    let full = paths.len() == k;
    if unique && let Some(index) = paths.iter().position(|old| old.path.text == next.path.text) {
        if !compare_context(&next, &paths[index]).is_lt() {
            return false;
        }
        paths.remove(index);
    }
    let index = paths.partition_point(|old| compare_context(old, &next).is_le());
    if index < k {
        paths.insert(index, next);
        paths.truncate(k);
    }
    full
}

struct ContextScorer<'a> {
    base: Scorer<'a>,
    words: &'a WordContextModel,
}

impl ContextScorer<'_> {
    fn first(&self) -> ContextPath {
        ContextPath {
            previous: self.words.recall_start(),
            ..ContextPath::default()
        }
    }

    fn extend(&self, path: &ContextPath, edge: usize) -> Option<ContextPath> {
        let mut next = self.base.extend(&path.path, edge, None, &mut Noop)?;
        let previous = self.words.recall_word(self.base.graph.edges[edge].entry);
        let gain = self.words.recall_gain(path.previous, previous);
        next.score += gain;
        Some(ContextPath {
            path: next,
            previous,
            gain: path.gain + gain,
        })
    }

    fn finish(&self, mut path: ContextPath) -> ContextPath {
        let extra = self
            .words
            .recall_gain(path.previous, self.words.recall_end());
        path.path = self.base.finish(path.path, &mut Noop);
        path.path.score += extra;
        path.gain += extra;
        path
    }

    fn beam(&self, config: &Config, work: &mut Statistics) -> Vec<ContextPath> {
        let end = self.base.graph.letters.len();
        let mut buckets: Vec<Vec<_>> = (0..end).map(|_| Vec::new()).collect();
        buckets[0].push(self.first());
        work.resident_high_water = 1;
        let mut resident = 1;
        let mut complete = Vec::new();
        for position in 0..end {
            for path in std::mem::take(&mut buckets[position]) {
                for &edge in &self.base.graph.outgoing[position] {
                    if self.base.cancelled()
                        || !expand_allowed(work, config)
                        || !resident_allowed(work, resident + 1, config)
                    {
                        return complete;
                    }
                    let Some(next) = self.extend(&path, edge) else {
                        continue;
                    };
                    let target = self.base.graph.edges[edge].end;
                    let before;
                    let after;
                    if target == end {
                        before = complete.len();
                        accept(&mut complete, self.finish(next), config.output_k, true);
                        after = complete.len();
                    } else {
                        let bucket = &mut buckets[target];
                        before = bucket.len();
                        work.beam_pruned +=
                            usize::from(accept(bucket, next, config.beam_width, false));
                        after = bucket.len();
                    }
                    resident = resident - before + after;
                }
                resident -= 1;
            }
        }
        complete
    }

    fn reference(&self, config: &Config, work: &mut Statistics) -> Vec<ContextPath> {
        let end = self.base.graph.letters.len();
        let mut pending = vec![(0, self.first())];
        let mut finished = Vec::new();
        work.resident_high_water = 1;
        'paths: while let Some((position, path)) = pending.pop() {
            if position == end {
                finished.push(self.finish(path));
                continue;
            }
            for &edge in self.base.graph.outgoing[position].iter().rev() {
                if self.base.cancelled()
                    || !expand_allowed(work, config)
                    || !resident_allowed(work, 2 + pending.len() + finished.len(), config)
                {
                    break 'paths;
                }
                if let Some(next) = self.extend(&path, edge) {
                    pending.push((self.base.graph.edges[edge].end, next));
                }
            }
        }
        finished.sort_by(compare_context);
        let mut seen = BTreeSet::new();
        finished
            .into_iter()
            .filter(|path| seen.insert(path.path.text.clone()))
            .take(config.output_k)
            .collect()
    }
}

/// 保留不同分段历史到终点，位置桶满时作显式近似剪枝。
pub fn recall_context(
    graph: &Lattice<'_>,
    config: &Config,
    character: &BigramModel,
    words: &WordContextModel,
    cancellation: &Cancellation,
) -> Result<RecallResult, SearchError> {
    context(graph, config, character, words, cancellation, false)
}

/// 小图审计只在原有预算内枚举完整路径。预算终止时不得当作完整参考。
pub fn reference_recall_context(
    graph: &Lattice<'_>,
    config: &Config,
    character: &BigramModel,
    words: &WordContextModel,
    cancellation: &Cancellation,
) -> Result<RecallResult, SearchError> {
    context(graph, config, character, words, cancellation, true)
}

fn context(
    graph: &Lattice<'_>,
    config: &Config,
    character: &BigramModel,
    words: &WordContextModel,
    cancellation: &Cancellation,
    reference: bool,
) -> Result<RecallResult, SearchError> {
    cancellation.check()?;
    validate(graph, config, character)?;
    if !words.is_compatible_lexicon(graph.lexicon) {
        return Err(SearchError::new(
            ErrorKind::InvalidModel,
            "研发召回需要已绑定词语模型",
        ));
    }
    let mut result = initial(graph);
    if graph.waiting() || graph.letters().is_empty() {
        result.exhaustive_for_score = result.work.budget.is_none();
        return Ok(result);
    }

    let scorer = ContextScorer {
        base: Scorer {
            lexicon: graph.lexicon,
            graph,
            ranker: Ranker::CharacterBigram,
            model: Some(character),
            cancellation: Some(cancellation),
        },
        words,
    };
    let paths = if reference {
        scorer.reference(config, &mut result.work)
    } else {
        scorer.beam(config, &mut result.work)
    };
    cancellation.check()?;
    result.exhaustive_for_score = result.work.budget.is_none() && result.work.beam_pruned == 0;
    result.candidates = paths
        .into_iter()
        .map(|found| {
            let path = found.path;
            RecallCandidate {
                path: Candidate {
                    text: path.text,
                    score: path.score,
                    entry_ids: path
                        .ids
                        .into_iter()
                        .map(|index| graph.lexicon.entries[index].id.clone())
                        .collect(),
                    edge_indices: path.edges,
                    features: path.features,
                },
                source_ranks: [None; 3],
                word_gain: found.gain,
            }
        })
        .collect();
    Ok(result)
}
