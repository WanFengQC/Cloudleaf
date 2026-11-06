use crate::profiling::{Noop, Observer, Phase};
use crate::{
    BigramModel, Budget, Cancellation, Config, ErrorKind, Lattice, Lexicon, MatchKind, Method,
    Ranker, SearchError,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Features {
    pub words: usize,
    pub prefixes: usize,
    pub frequency_units: i64,
    pub missing_frequencies: usize,
    pub lexical_units: i64,
    pub lexical_observed: usize,
    pub lexical_missing: usize,
    pub bigram_units: i64,
    pub context_units: i64,
    pub structural_units: i64,
    pub character_units: i64,
    pub character_count: usize,
    pub character_unknown: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub text: String,
    pub score: i64,
    pub entry_ids: Vec<String>,
    pub edge_indices: Vec<usize>,
    pub features: Features,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Statistics {
    pub graph_edges: usize,
    pub expansions: usize,
    pub resident_high_water: usize,
    pub beam_pruned: usize,
    pub budget: Option<Budget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchResult {
    pub candidates: Vec<Candidate>,
    pub raw_fallback: String,
    pub waiting: bool,
    /// 仅指本图、本评分、unique top-k；上下文重排保证限于召回池。
    pub guaranteed: bool,
    pub finite_pool_only: bool,
    pub statistics: Statistics,
}

#[derive(Clone, Debug, Default)]
struct Path {
    text: String,
    score: i64,
    ids: Vec<usize>,
    edges: Vec<usize>,
    features: Features,
    last_character: Option<u32>,
}

fn compare(a: &Path, b: &Path) -> Ordering {
    b.score
        .cmp(&a.score)
        .then_with(|| b.features.lexical_units.cmp(&a.features.lexical_units))
        .then_with(|| a.ids.cmp(&b.ids))
}

struct Scorer<'a> {
    lexicon: &'a Lexicon,
    graph: &'a Lattice<'a>,
    ranker: Ranker,
    model: Option<&'a BigramModel>,
    cancellation: Option<&'a Cancellation>,
}

impl Scorer<'_> {
    fn cancelled(&self) -> bool {
        self.cancellation.is_some_and(Cancellation::is_cancelled)
    }

    fn extend<O: Observer>(
        &self,
        path: &Path,
        edge_index: usize,
        cutoff: Option<&Path>,
        observer: &mut O,
    ) -> Option<Path> {
        let edge = &self.graph.edges[edge_index];
        let entry = &self.lexicon.entries[edge.entry];
        let stamp = observer.start(Phase::Scoring);
        let mut features = path.features.clone();
        let mut score = path.score;
        features.structural_units -= 100;
        if self.ranker != Ranker::CharacterBigram {
            score -= 100;
        }
        features.words += 1;
        if edge.kind == MatchKind::Prefix {
            features.prefixes += 1;
            features.structural_units -= 300;
            if self.ranker != Ranker::CharacterBigram {
                score -= 300;
            }
        }
        if let Some(frequency) = entry.frequency {
            let units = i64::from(63 - (u64::from(frequency) + 1).leading_zeros());
            features.frequency_units += units;
            if matches!(
                self.ranker,
                Ranker::Weighted | Ranker::Bigram | Ranker::ContextRerank
            ) {
                score += units * 10;
            }
        } else {
            features.missing_frequencies += 1;
        }
        if self.ranker == Ranker::Lexical {
            if let Some(units) = self.model.and_then(|model| model.lexical_units(&entry.id)) {
                features.lexical_units += units;
                features.lexical_observed += 1;
            } else {
                features.lexical_missing += 1;
            }
        }
        if let Some(model) = self
            .model
            .filter(|_| matches!(self.ranker, Ranker::Bigram | Ranker::ContextRerank))
        {
            let previous = path
                .ids
                .last()
                .map(|index| self.lexicon.entries[*index].id.as_str());
            let units = model.transition(previous, Some(&entry.id));
            features.bigram_units += units;
            score += units;
        }
        let mut last_character = path.last_character;
        if self.ranker == Ranker::CharacterBigram {
            let model = self.model.unwrap().character.as_ref().unwrap();
            let entry_score = model.entry(edge.entry);
            let units = model.transition(
                path.last_character.unwrap_or(crate::character::BOS),
                entry_score.first,
            ) + entry_score.internal;
            features.character_units += units;
            features.character_count += entry_score.characters;
            features.character_unknown += entry_score.unknown;
            score += units;
            last_character = Some(entry_score.last);
        }
        observer.stop(Phase::Scoring, stamp);
        // 满桶中严格劣于末项的扩展必被丢弃，先比较借用ID，避免复制整条路径。
        if cutoff.is_some_and(|worst| {
            worst
                .score
                .cmp(&score)
                .then_with(|| worst.features.lexical_units.cmp(&features.lexical_units))
                .then_with(|| {
                    path.ids
                        .iter()
                        .copied()
                        .chain(std::iter::once(edge.entry))
                        .cmp(worst.ids.iter().copied())
                })
                == Ordering::Greater
        }) {
            return None;
        }
        let mut text = String::with_capacity(path.text.len() + entry.text.len());
        text.push_str(&path.text);
        text.push_str(&entry.text);
        let mut ids = Vec::with_capacity(path.ids.len() + 1);
        ids.extend_from_slice(&path.ids);
        ids.push(edge.entry);
        let mut edges = Vec::with_capacity(path.edges.len() + 1);
        edges.extend_from_slice(&path.edges);
        edges.push(edge_index);
        Some(Path {
            text,
            score,
            ids,
            edges,
            features,
            last_character,
        })
    }

    fn finish<O: Observer>(&self, mut path: Path, observer: &mut O) -> Path {
        let stamp = observer.start(Phase::Scoring);
        if let Some(model) = self
            .model
            .filter(|_| matches!(self.ranker, Ranker::Bigram | Ranker::ContextRerank))
        {
            let previous = path
                .ids
                .last()
                .map(|index| self.lexicon.entries[*index].id.as_str());
            let units = model.transition(previous, None);
            path.features.bigram_units += units;
            path.score += units;
        }
        if self.ranker == Ranker::CharacterBigram {
            let units = self
                .model
                .unwrap()
                .character
                .as_ref()
                .unwrap()
                .finish(path.last_character.unwrap_or(crate::character::BOS));
            path.features.character_units += units;
            path.score += units;
        }
        observer.stop(Phase::Scoring, stamp);
        path
    }
}

fn retain_unique<O: Observer>(paths: &mut Vec<Path>, k: usize, observer: &mut O, phase: Phase) {
    let stamp = observer.start(phase);
    paths.sort_by(compare);
    let mut texts = std::collections::BTreeSet::new();
    paths.retain(|path| texts.insert(path.text.clone()));
    paths.truncate(k);
    observer.stop(phase, stamp);
}

fn insert_unique<O: Observer>(paths: &mut Vec<Path>, next: Path, k: usize, observer: &mut O) {
    let stamp = observer.start(Phase::BucketSortDedup);
    if let Some(index) = paths.iter().position(|path| path.text == next.text) {
        // 桶始终有序且正文唯一；相等时保留先到路径，含其原edge与features。
        if compare(&next, &paths[index]) != Ordering::Less {
            observer.stop(Phase::BucketSortDedup, stamp);
            return;
        }
        paths.remove(index);
    }
    let index = paths.partition_point(|path| compare(path, &next) != Ordering::Greater);
    paths.insert(index, next);
    paths.truncate(k);
    observer.stop(Phase::BucketSortDedup, stamp);
}

fn expand_allowed(statistics: &mut Statistics, config: &Config) -> bool {
    if statistics.expansions == config.max_expansions {
        statistics.budget = Some(Budget::Expansions);
        false
    } else {
        statistics.expansions += 1;
        true
    }
}

fn resident_allowed(statistics: &mut Statistics, resident: usize, config: &Config) -> bool {
    if resident > config.max_resident_paths {
        statistics.budget = Some(Budget::ResidentPaths);
        false
    } else {
        statistics.resident_high_water = statistics.resident_high_water.max(resident);
        true
    }
}

fn reference<O: Observer>(
    scorer: &Scorer<'_>,
    config: &Config,
    stats: &mut Statistics,
    k: usize,
    observer: &mut O,
) -> Vec<Path> {
    let mut pending = vec![(0, Path::default())];
    let mut complete = Vec::new();
    stats.resident_high_water = 1;
    while let Some((position, path)) = pending.pop() {
        if scorer.cancelled() {
            return Vec::new();
        }
        if position == scorer.graph.letters.len() {
            // 参考枚举所有路径，只将已完成输出压缩为当前最佳unique top-k。
            let finished = scorer.finish(path, observer);
            insert_unique(&mut complete, finished, k, observer);
            continue;
        }
        for edge in &scorer.graph.outgoing[position] {
            if scorer.cancelled() {
                return Vec::new();
            }
            if !expand_allowed(stats, config) {
                return complete;
            }
            // 当前被pop的路径和extend新克隆同时存活。
            if !resident_allowed(stats, pending.len() + complete.len() + 2, config) {
                return complete;
            }
            pending.push((
                scorer.graph.edges[*edge].end,
                scorer.extend(&path, *edge, None, observer).unwrap(),
            ));
        }
    }
    complete
}

fn dynamic<O: Observer>(
    scorer: &Scorer<'_>,
    config: &Config,
    stats: &mut Statistics,
    k: usize,
    observer: &mut O,
) -> Vec<Path> {
    // 词模型保留last词条；字符模型保留最后映射字符，不能按位置合并不同历史。
    let mut states: Vec<BTreeMap<Option<usize>, Vec<Path>>> =
        vec![BTreeMap::new(); scorer.graph.letters.len() + 1];
    states[0].insert(None, vec![Path::default()]);
    let mut resident = 1;
    stats.resident_high_water = 1;
    for position in 0..scorer.graph.letters.len() {
        let current = std::mem::take(&mut states[position]);
        let mut paths: Vec<_> = current.into_values().flatten().collect();
        if config.method == Method::Beam {
            paths.sort_by(compare);
            let pruned = paths.len().saturating_sub(config.beam_width);
            stats.beam_pruned += pruned;
            resident -= pruned;
            paths.truncate(config.beam_width);
        }
        for path in paths {
            for edge_index in &scorer.graph.outgoing[position] {
                if scorer.cancelled() {
                    return Vec::new();
                }
                if !expand_allowed(stats, config) {
                    return Vec::new();
                }
                let edge = &scorer.graph.edges[*edge_index];
                let history = if scorer.ranker == Ranker::CharacterBigram {
                    scorer
                        .model
                        .unwrap()
                        .character
                        .as_ref()
                        .unwrap()
                        .entry(edge.entry)
                        .last as usize
                } else {
                    edge.entry
                };
                let bucket = states[edge.end].entry(Some(history)).or_default();
                let old = bucket.len();
                if !resident_allowed(stats, resident + 1, config) {
                    return Vec::new();
                }
                let cutoff = (old == k).then(|| bucket.last().unwrap());
                if let Some(next) = scorer.extend(&path, *edge_index, cutoff, observer) {
                    insert_unique(bucket, next, k, observer);
                }
                resident = resident + bucket.len() - old;
            }
            resident -= 1;
        }
    }
    states
        .pop()
        .unwrap_or_default()
        .into_values()
        .flatten()
        .map(|path| scorer.finish(path, observer))
        .collect()
}

fn position_dynamic<O: Observer>(
    scorer: &Scorer<'_>,
    config: &Config,
    stats: &mut Statistics,
    k: usize,
    observer: &mut O,
) -> Vec<Path> {
    // 仅无历史评分可合并最后词条状态；相同位置具有相同后缀集合和后续增分。
    let mut states: Vec<Vec<Path>> = vec![Vec::new(); scorer.graph.letters.len() + 1];
    states[0].push(Path::default());
    let mut resident = 1;
    stats.resident_high_water = 1;
    for position in 0..scorer.graph.letters.len() {
        let current = std::mem::take(&mut states[position]);
        // 取出的整批路径仍计入resident，不能只计算尚留在states中的容器。
        for path in current {
            for edge_index in &scorer.graph.outgoing[position] {
                if scorer.cancelled() {
                    return Vec::new();
                }
                if !expand_allowed(stats, config) {
                    return Vec::new();
                }
                let edge = &scorer.graph.edges[*edge_index];
                let bucket = &mut states[edge.end];
                let old = bucket.len();
                if !resident_allowed(stats, resident + 1, config) {
                    return Vec::new();
                }
                let cutoff = (old == k).then(|| bucket.last().unwrap());
                if let Some(next) = scorer.extend(&path, *edge_index, cutoff, observer) {
                    insert_unique(bucket, next, k, observer);
                }
                resident = resident + bucket.len() - old;
            }
            resident -= 1;
        }
    }
    states.pop().unwrap_or_default()
}

struct BoundedPath {
    bound: i64,
    best_ids: Vec<usize>,
    order: usize,
    position: usize,
    path: Path,
}

impl PartialEq for BoundedPath {
    fn eq(&self, other: &Self) -> bool {
        self.order == other.order
    }
}
impl Eq for BoundedPath {}
impl PartialOrd for BoundedPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for BoundedPath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bound
            .cmp(&other.bound)
            .then_with(|| other.best_ids.cmp(&self.best_ids))
            .then_with(|| other.order.cmp(&self.order))
    }
}

#[derive(Clone)]
struct Suffix {
    score: i64,
    ids: Vec<usize>,
}

/// 精确后缀分用于有界最佳优先枚举；同分上界不能提前结束稳定ID比较。
fn character_exact<O: Observer>(
    scorer: &Scorer<'_>,
    config: &Config,
    stats: &mut Statistics,
    k: usize,
    observer: &mut O,
) -> Vec<Path> {
    let model = scorer.model.unwrap().character.as_ref().unwrap();
    let end = scorer.graph.letters.len();
    let mut suffix = vec![BTreeMap::<u32, Option<Suffix>>::new(); end + 1];
    suffix[0].insert(crate::character::BOS, None);
    let mut labels = 1;
    for edge in &scorer.graph.edges {
        if suffix[edge.end]
            .insert(model.entry(edge.entry).last, None)
            .is_none()
        {
            labels += 1;
            if !resident_allowed(stats, labels + 1, config) {
                return Vec::new();
            }
        }
    }
    for (&previous, value) in &mut suffix[end] {
        *value = Some(Suffix {
            score: model.finish(previous),
            ids: Vec::new(),
        });
    }
    for position in (0..end).rev() {
        let histories: Vec<_> = suffix[position].keys().copied().collect();
        for previous in histories {
            let mut best: Option<Suffix> = None;
            for edge_index in &scorer.graph.outgoing[position] {
                if scorer.cancelled() {
                    return Vec::new();
                }
                if !expand_allowed(stats, config) {
                    return Vec::new();
                }
                let edge = &scorer.graph.edges[*edge_index];
                let entry = model.entry(edge.entry);
                if let Some(rest) = suffix[edge.end].get(&entry.last).and_then(Option::as_ref) {
                    let stamp = observer.start(Phase::Scoring);
                    let score =
                        model.transition(previous, entry.first) + entry.internal + rest.score;
                    let ids = std::iter::once(edge.entry).chain(rest.ids.iter().copied());
                    if best.as_ref().is_none_or(|value| {
                        score > value.score
                            || (score == value.score
                                && ids.clone().cmp(value.ids.iter().copied()) == Ordering::Less)
                    }) {
                        best = Some(Suffix {
                            score,
                            ids: ids.collect(),
                        });
                    }
                    observer.stop(Phase::Scoring, stamp);
                }
            }
            suffix[position].insert(previous, best);
        }
    }
    let Some(best) = &suffix[0][&crate::character::BOS] else {
        return Vec::new();
    };
    let mut pending = BinaryHeap::from([BoundedPath {
        bound: best.score,
        best_ids: best.ids.clone(),
        order: 0,
        position: 0,
        path: Path::default(),
    }]);
    let mut order = 0;
    let mut complete: Vec<Path> = Vec::new();
    let mut seen = vec![BTreeMap::<String, Vec<usize>>::new(); end + 1];
    seen[0].insert(String::new(), Vec::new());
    let mut seen_count = 1;
    let mut scratch = String::new();
    while let Some(current) = pending.pop() {
        if scorer.cancelled() {
            return Vec::new();
        }
        if seen[current.position].get(&current.path.text) != Some(&current.path.ids) {
            continue;
        }
        if complete.len() == k
            && (current.bound < complete.last().unwrap().score
                || (current.bound == complete.last().unwrap().score
                    && current.best_ids >= complete.last().unwrap().ids))
        {
            break;
        }
        if current.position == end {
            let finished = scorer.finish(current.path, observer);
            insert_unique(&mut complete, finished, k, observer);
            continue;
        }
        for edge_index in &scorer.graph.outgoing[current.position] {
            if scorer.cancelled() {
                return Vec::new();
            }
            if !expand_allowed(stats, config) {
                return Vec::new();
            }
            let edge = &scorer.graph.edges[*edge_index];
            let entry = model.entry(edge.entry);
            let Some(rest) = suffix[edge.end].get(&entry.last).and_then(Option::as_ref) else {
                continue;
            };
            if !resident_allowed(
                stats,
                labels + seen_count + pending.len() + complete.len() + 3,
                config,
            ) {
                return Vec::new();
            }
            let stamp = observer.start(Phase::Scoring);
            let bound = current.path.score
                + model.transition(
                    current.path.last_character.unwrap_or(crate::character::BOS),
                    entry.first,
                )
                + entry.internal
                + rest.score;
            observer.stop(Phase::Scoring, stamp);
            if complete.len() == k && bound < complete.last().unwrap().score {
                continue;
            }
            // 先复用正文暂存并借用ID比较，被淘汰分支不复制整条路径。
            scratch.clear();
            scratch.push_str(&current.path.text);
            scratch.push_str(&scorer.lexicon.entries[edge.entry].text);
            if seen[edge.end].get(scratch.as_str()).is_some_and(|ids| {
                current
                    .path
                    .ids
                    .iter()
                    .copied()
                    .chain(std::iter::once(edge.entry))
                    .cmp(ids.iter().copied())
                    != Ordering::Less
            }) {
                continue;
            }
            let next = scorer
                .extend(&current.path, *edge_index, None, observer)
                .unwrap();
            if seen[edge.end]
                .insert(next.text.clone(), next.ids.clone())
                .is_none()
            {
                seen_count += 1;
            }
            order += 1;
            let best_ids = next
                .ids
                .iter()
                .copied()
                .chain(rest.ids.iter().copied())
                .collect();
            pending.push(BoundedPath {
                bound,
                best_ids,
                order,
                position: edge.end,
                path: next,
            });
        }
    }
    complete
}

/// 在同一个只读词条图上比较搜索方法。输入/模型载入和评估IO由宿主负责。
pub fn search_graph(
    lexicon: &Lexicon,
    graph: &Lattice<'_>,
    config: &Config,
    model: Option<&BigramModel>,
    left_word: Option<&str>,
) -> Result<SearchResult, SearchError> {
    search_observed(lexicon, graph, config, model, left_word, &mut Noop)
}

pub fn search_graph_cancellable(
    lexicon: &Lexicon,
    graph: &Lattice<'_>,
    config: &Config,
    model: Option<&BigramModel>,
    left_word: Option<&str>,
    cancellation: &Cancellation,
) -> Result<SearchResult, SearchError> {
    search_controlled_observed(
        lexicon,
        graph,
        config,
        model,
        left_word,
        Some(cancellation),
        &mut Noop,
    )
}

pub(crate) fn search_observed<O: Observer>(
    lexicon: &Lexicon,
    graph: &Lattice<'_>,
    config: &Config,
    model: Option<&BigramModel>,
    left_word: Option<&str>,
    observer: &mut O,
) -> Result<SearchResult, SearchError> {
    search_controlled_observed(lexicon, graph, config, model, left_word, None, observer)
}

pub(crate) fn search_controlled_observed<O: Observer>(
    lexicon: &Lexicon,
    graph: &Lattice<'_>,
    config: &Config,
    model: Option<&BigramModel>,
    left_word: Option<&str>,
    cancellation: Option<&Cancellation>,
    observer: &mut O,
) -> Result<SearchResult, SearchError> {
    if let Some(control) = cancellation {
        control.check()?;
    }
    let stamp = observer.start(Phase::SearchValidate);
    let validation = (|| {
        config.validate()?;
        if (matches!(config.ranker, Ranker::Bigram | Ranker::ContextRerank)
            && model.is_none_or(|value| !value.has_transitions()))
            || (config.ranker == Ranker::Lexical
                && model.is_none_or(|value| !value.has_lexical_signal()))
            || (config.ranker == Ranker::CharacterBigram
                && model.is_none_or(|value| !value.has_character_transitions()))
            || model.is_some_and(|value| !value.is_compatible_lexicon(lexicon))
        {
            return Err(SearchError::new(
                ErrorKind::InvalidModel,
                "所选评分需要预加载模型",
            ));
        }
        // 图字段只读且借用原词典；拒绝把索引应用于另一份同长度词典。
        if !std::ptr::eq(lexicon, graph.lexicon) {
            return Err(SearchError::new(
                ErrorKind::InvalidInput,
                "词条图与词典不匹配",
            ));
        }
        if graph.raw.len() > config.max_input_bytes
            || graph.letters.len() > config.max_letters
            || graph.edges.len() > config.max_edges
        {
            return Err(SearchError::new(
                ErrorKind::InvalidConfig,
                "已有词条图超出本次搜索预算",
            ));
        }
        Ok(())
    })();
    observer.stop(Phase::SearchValidate, stamp);
    validation?;
    let mut statistics = Statistics {
        graph_edges: graph.edges.len(),
        budget: graph.budget,
        ..Statistics::default()
    };
    let k = if config.ranker == Ranker::ContextRerank {
        config.context_pool
    } else {
        config.output_k
    };
    let scorer = Scorer {
        lexicon,
        graph,
        ranker: config.ranker,
        model,
        cancellation,
    };
    let stamp = observer.start(Phase::Traversal);
    let mut paths = if graph.letters.is_empty() || graph.waiting {
        Vec::new()
    } else if config.method == Method::Reference {
        reference(&scorer, config, &mut statistics, k, observer)
    } else if config.method == Method::PositionDag {
        position_dynamic(&scorer, config, &mut statistics, k, observer)
    } else if config.method == Method::ExactDag && config.ranker == Ranker::CharacterBigram {
        character_exact(&scorer, config, &mut statistics, k, observer)
    } else {
        dynamic(&scorer, config, &mut statistics, k, observer)
    };
    observer.stop(Phase::Traversal, stamp);
    if let Some(control) = cancellation {
        control.check()?;
    }
    retain_unique(&mut paths, k, observer, Phase::FinalSortDedup);
    if config.ranker == Ranker::ContextRerank {
        let stamp = observer.start(Phase::ContextRerank);
        if let (Some(model), Some(left)) = (model, left_word) {
            for path in &mut paths {
                if let Some(first) = path.ids.first() {
                    let first = &lexicon.entries[*first].id;
                    let units = model.transition(Some(left), Some(first))
                        - model.transition(None, Some(first));
                    path.features.context_units = units;
                    path.score += units;
                }
            }
        }
        paths.sort_by(compare);
        observer.stop(Phase::ContextRerank, stamp);
    }
    paths.truncate(config.output_k);
    let guaranteed = statistics.budget.is_none() && statistics.beam_pruned == 0;
    let stamp = observer.start(Phase::Materialize);
    let result = SearchResult {
        candidates: paths
            .into_iter()
            .map(|path| Candidate {
                text: path.text,
                score: path.score,
                entry_ids: path
                    .ids
                    .iter()
                    .map(|index| lexicon.entries[*index].id.clone())
                    .collect(),
                edge_indices: path.edges,
                features: path.features,
            })
            .collect(),
        raw_fallback: graph.raw.clone(),
        waiting: graph.waiting,
        guaranteed,
        finite_pool_only: config.ranker == Ranker::ContextRerank,
        statistics,
    };
    observer.stop(Phase::Materialize, stamp);
    if let Some(control) = cancellation {
        control.check()?;
    }
    Ok(result)
}

#[cfg(test)]
mod bucket_regression {
    use super::*;

    fn path(text: &str, score: i64, ids: &[usize], edge: usize) -> Path {
        Path {
            text: text.into(),
            score,
            ids: ids.to_vec(),
            edges: vec![edge],
            ..Path::default()
        }
    }

    fn insert(bucket: &mut Vec<Path>, next: Path, k: usize) {
        insert_unique(bucket, next, k, &mut Noop);
    }

    #[test]
    fn same_text_keeps_strict_best_and_equal_first_edges() {
        let mut bucket = Vec::new();
        for next in [
            path("甲", 1, &[2], 10),
            path("甲", 0, &[0], 11),
            path("甲", 1, &[1], 12),
            path("甲", 1, &[1], 13),
        ] {
            insert(&mut bucket, next, 5);
        }
        assert_eq!(bucket.len(), 1);
        assert_eq!(bucket[0].score, 1);
        assert_eq!(bucket[0].ids, [1]);
        assert_eq!(bucket[0].edges, [12]);
    }

    #[test]
    fn different_text_equal_order_is_stable_and_truncation_follows_order() {
        let mut bucket = Vec::new();
        for next in [
            path("甲", 1, &[1], 1),
            path("乙", 1, &[1], 2),
            path("丙", 1, &[1], 3),
            path("丁", 2, &[9], 4),
        ] {
            insert(&mut bucket, next, 2);
        }
        assert_eq!(
            bucket.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
            ["丁", "甲"]
        );
        insert(&mut bucket, path("甲", 3, &[8], 5), 2);
        assert_eq!(
            bucket.iter().map(|p| p.edges[0]).collect::<Vec<_>>(),
            [5, 4]
        );
        insert(&mut bucket, path("戊", 2, &[9], 6), 2);
        assert_eq!(
            bucket.iter().map(|p| p.edges[0]).collect::<Vec<_>>(),
            [5, 4]
        );
    }
}

#[cfg(test)]
mod cancellation_regression {
    use super::*;

    struct CancelDuringScoring<'a> {
        control: &'a Cancellation,
        scored: usize,
    }
    impl Observer for CancelDuringScoring<'_> {
        type Stamp = ();
        fn start(&mut self, phase: Phase) {
            if phase == Phase::Scoring {
                self.scored += 1;
                if self.scored == 2 {
                    self.control.cancel();
                }
            }
        }
        fn stop(&mut self, _: Phase, _: ()) {}
    }

    const DATA: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\ta\t甲\t\tunknown\t\tfixture\ta\tMIT\nb\ta\t乙\t\tunknown\t\tfixture\tb\tMIT\n";

    #[test]
    fn pre_cancelled_graph_and_search_return_cancelled_not_budget() {
        let lexicon = Lexicon::from_tsv(DATA).unwrap();
        let config = Config::default();
        let control = Cancellation::default();
        let clone = control.clone();
        clone.cancel();
        assert!(control.is_cancelled());
        assert_eq!(
            Lattice::build_cancellable(&lexicon, "aa", &config, &control)
                .unwrap_err()
                .kind,
            ErrorKind::Cancelled
        );
        let graph = Lattice::build(&lexicon, "aa", &config).unwrap();
        assert_eq!(
            search_graph_cancellable(&lexicon, &graph, &config, None, None, &control)
                .unwrap_err()
                .kind,
            ErrorKind::Cancelled
        );
    }

    #[test]
    fn actual_traversal_cancellation_discards_partial_paths_for_every_method() {
        let lexicon = Lexicon::from_tsv(DATA).unwrap();
        for method in [
            Method::Reference,
            Method::ExactDag,
            Method::PositionDag,
            Method::Beam,
        ] {
            let config = Config {
                method,
                ranker: Ranker::Structural,
                ..Config::default()
            };
            let graph = Lattice::build(&lexicon, "aaaa", &config).unwrap();
            let control = Cancellation::default();
            let mut observer = CancelDuringScoring {
                control: &control,
                scored: 0,
            };
            assert_eq!(
                search_controlled_observed(
                    &lexicon,
                    &graph,
                    &config,
                    None,
                    None,
                    Some(&control),
                    &mut observer
                )
                .unwrap_err()
                .kind,
                ErrorKind::Cancelled
            );
            assert_eq!(observer.scored, 2);
        }
    }

    struct CancelAfterMaterialize<'a>(&'a Cancellation);
    impl Observer for CancelAfterMaterialize<'_> {
        type Stamp = ();
        fn start(&mut self, _: Phase) {}
        fn stop(&mut self, phase: Phase, _: ()) {
            if phase == Phase::Materialize {
                self.0.cancel();
            }
        }
    }

    #[test]
    fn cancellation_before_result_publication_returns_no_completed_candidates() {
        let lexicon = Lexicon::from_tsv(DATA).unwrap();
        let config = Config::default();
        let graph = Lattice::build(&lexicon, "a", &config).unwrap();
        let control = Cancellation::default();
        let mut observer = CancelAfterMaterialize(&control);
        assert_eq!(
            search_controlled_observed(
                &lexicon,
                &graph,
                &config,
                None,
                None,
                Some(&control),
                &mut observer
            )
            .unwrap_err()
            .kind,
            ErrorKind::Cancelled
        );
    }

    #[test]
    fn uncancelled_entry_preserves_results_and_original_budget_semantics() {
        let lexicon = Lexicon::from_tsv(DATA).unwrap();
        for method in [
            Method::Reference,
            Method::ExactDag,
            Method::PositionDag,
            Method::Beam,
        ] {
            for budget in [1, 200000] {
                let config = Config {
                    method,
                    ranker: Ranker::Structural,
                    max_expansions: budget,
                    ..Config::default()
                };
                let control = Cancellation::default();
                let graph =
                    Lattice::build_cancellable(&lexicon, "aaaa", &config, &control).unwrap();
                let full = search_graph(&lexicon, &graph, &config, None, None).unwrap();
                let controlled =
                    search_graph_cancellable(&lexicon, &graph, &config, None, None, &control)
                        .unwrap();
                assert_eq!(full, controlled);
                if budget == 1 {
                    assert_eq!(controlled.statistics.budget, Some(Budget::Expansions));
                }
            }
        }
    }
}
