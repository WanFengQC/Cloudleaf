use crate::profiling::{Noop, Observer, Phase};
use crate::{
    BigramModel, Budget, Cancellation, Config, ErrorKind, InterpretationCost, Lattice, Lexicon,
    MatchKind, Method, Ranker, SearchError,
};
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};
use std::rc::Rc;

#[path = "recall.rs"]
mod recall;
pub use recall::{
    RecallCandidate, RecallResult, recall_context, recall_interleaved, reference_recall_context,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Features {
    pub interpretation_cost: InterpretationCost,
    pub user_preference_units: u32,
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
    interpreted: bool,
}

fn compare(a: &Path, b: &Path) -> Ordering {
    a.features
        .interpretation_cost
        .cmp(&b.features.interpretation_cost)
        .then_with(|| {
            b.features
                .user_preference_units
                .cmp(&a.features.user_preference_units)
        })
        .then_with(|| b.score.cmp(&a.score))
        .then_with(|| b.features.lexical_units.cmp(&a.features.lexical_units))
        .then_with(|| a.ids.cmp(&b.ids))
        .then_with(|| {
            if a.interpreted || b.interpreted {
                a.edges.cmp(&b.edges)
            } else {
                Ordering::Equal
            }
        })
}

struct Scorer<'a> {
    lexicon: &'a Lexicon,
    graph: &'a Lattice<'a>,
    ranker: Ranker,
    model: Option<&'a BigramModel>,
    cancellation: Option<&'a Cancellation>,
    lexical_cache: Option<Vec<Cell<i32>>>,
}

impl Scorer<'_> {
    fn cancelled(&self) -> bool {
        self.cancellation.is_some_and(Cancellation::is_cancelled)
    }

    fn lexical_units(&self, edge: usize, id: &str) -> Option<i64> {
        let model = self.model?;
        let Some(cache) = &self.lexical_cache else {
            return model.lexical_units(id);
        };
        let cell = &cache[edge];
        let mut units = cell.get();
        if units == i32::MIN {
            // 单条词汇分至多 40 个二进制数量级，负值仅表示缺少观测。
            units = model.lexical_units(id).map_or(-1, |value| value as i32);
            cell.set(units);
        }
        (units >= 0).then_some(i64::from(units))
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
        features.interpretation_cost = features
            .interpretation_cost
            .plus(self.graph.interpretation_cost(edge_index));
        if features.interpretation_cost.edits > self.graph.max_edit_cost() {
            observer.stop(Phase::Scoring, stamp);
            return None;
        }
        features.user_preference_units += self.lexicon.user_preference(edge.entry);
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
            if let Some(units) = self.lexical_units(edge_index, &entry.id) {
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
            features
                .interpretation_cost
                .cmp(&worst.features.interpretation_cost)
                .then_with(|| {
                    worst
                        .features
                        .user_preference_units
                        .cmp(&features.user_preference_units)
                })
                .then_with(|| worst.score.cmp(&score))
                .then_with(|| worst.features.lexical_units.cmp(&features.lexical_units))
                .then_with(|| {
                    path.ids
                        .iter()
                        .copied()
                        .chain(std::iter::once(edge.entry))
                        .cmp(worst.ids.iter().copied())
                })
                .then_with(|| {
                    if self.graph.is_interpreted() {
                        path.edges
                            .iter()
                            .copied()
                            .chain(std::iter::once(edge_index))
                            .cmp(worst.edges.iter().copied())
                    } else {
                        Ordering::Equal
                    }
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
            interpreted: self.graph.is_interpreted(),
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
            if let Some(next) = scorer.extend(&path, *edge, None, observer) {
                pending.push((scorer.graph.edges[*edge].end, next));
            }
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

type InterpretationStates = BTreeMap<(Option<usize>, u16), Vec<Path>>;

/// 解释成本与语言历史分别保存；已用编辑量影响剩余可行后缀，不能按位置丢弃。
fn interpreted_dynamic<O: Observer>(
    scorer: &Scorer<'_>,
    config: &Config,
    stats: &mut Statistics,
    k: usize,
    observer: &mut O,
) -> Vec<Path> {
    let mut states: Vec<InterpretationStates> =
        vec![BTreeMap::new(); scorer.graph.letters.len() + 1];
    states[0].insert((None, 0), vec![Path::default()]);
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
                let used_edits = path.features.interpretation_cost.edits
                    + scorer.graph.interpretation_cost(*edge_index).edits;
                if used_edits > scorer.graph.max_edit_cost() {
                    continue;
                }
                let history = if config.method == Method::PositionDag {
                    None
                } else if scorer.ranker == Ranker::CharacterBigram {
                    Some(
                        scorer
                            .model
                            .unwrap()
                            .character
                            .as_ref()
                            .unwrap()
                            .entry(edge.entry)
                            .last as usize,
                    )
                } else {
                    Some(edge.entry)
                };
                let bucket = states[edge.end].entry((history, used_edits)).or_default();
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
    let positions = scorer.graph.letters.len() + 1;
    let suffix = scorer.graph.retained_suffix();
    let mut states: Vec<Vec<Path>> = vec![Vec::new(); positions];
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
                let edge = &scorer.graph.edges[*edge_index];
                // 图仍保留局部边；没有后缀的局部边不能参与完整候选搜索。
                if suffix.is_some_and(|reachable| !reachable.contains(edge.end)) {
                    continue;
                }
                if !expand_allowed(stats, config) {
                    return Vec::new();
                }
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
    tail: Rc<[usize]>,
    order: usize,
    position: usize,
    text: Rc<str>,
    ids: Rc<[usize]>,
    edges: Vec<usize>,
    score: i64,
    last_character: u32,
}

impl BoundedPath {
    fn best_ids(&self) -> impl Iterator<Item = usize> + '_ {
        self.ids.iter().chain(self.tail.iter()).copied()
    }
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
            .then_with(|| other.best_ids().cmp(self.best_ids()))
            .then_with(|| other.order.cmp(&self.order))
    }
}

#[derive(Clone)]
struct Suffix {
    score: i64,
    ids: Rc<[usize]>,
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
    let empty: Rc<[usize]> = Rc::from([]);
    for (&previous, value) in &mut suffix[end] {
        *value = Some(Suffix {
            score: model.finish(previous),
            ids: Rc::clone(&empty),
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
        tail: Rc::clone(&best.ids),
        order: 0,
        position: 0,
        text: Rc::from(""),
        ids: Rc::clone(&empty),
        edges: Vec::new(),
        score: 0,
        last_character: crate::character::BOS,
    }]);
    let mut order = 0;
    let mut complete: Vec<Path> = Vec::new();
    let mut seen = vec![BTreeMap::<Rc<str>, Rc<[usize]>>::new(); end + 1];
    seen[0].insert(Rc::from(""), empty);
    let mut seen_count = 1;
    let mut scratch = String::new();
    while let Some(current) = pending.pop() {
        if scorer.cancelled() {
            return Vec::new();
        }
        if seen[current.position].get(current.text.as_ref()) != Some(&current.ids) {
            continue;
        }
        if complete.len() == k
            && (current.bound < complete.last().unwrap().score
                || (current.bound == complete.last().unwrap().score
                    && !current
                        .best_ids()
                        .cmp(complete.last().unwrap().ids.iter().copied())
                        .is_lt()))
        {
            break;
        }
        if current.position == end {
            // 只有完成路径需要对外特征，使用同一评分器恢复完整诊断。
            let path = current
                .edges
                .into_iter()
                .fold(Path::default(), |path, edge| {
                    scorer.extend(&path, edge, None, observer).unwrap()
                });
            let finished = scorer.finish(path, observer);
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
            let score = current.score
                + model.transition(current.last_character, entry.first)
                + entry.internal;
            let bound = score + rest.score;
            observer.stop(Phase::Scoring, stamp);
            if complete.len() == k && bound < complete.last().unwrap().score {
                continue;
            }
            // 先复用正文暂存并借用ID比较，被淘汰分支不复制整条路径。
            scratch.clear();
            scratch.push_str(&current.text);
            scratch.push_str(&scorer.lexicon.entries[edge.entry].text);
            if seen[edge.end].get(scratch.as_str()).is_some_and(|ids| {
                current
                    .ids
                    .iter()
                    .copied()
                    .chain(std::iter::once(edge.entry))
                    .cmp(ids.iter().copied())
                    != Ordering::Less
            }) {
                continue;
            }
            let ids: Rc<[usize]> = current
                .ids
                .iter()
                .copied()
                .chain(std::iter::once(edge.entry))
                .collect();
            let text: Rc<str> = Rc::from(scratch.as_str());
            if seen[edge.end]
                .insert(Rc::clone(&text), Rc::clone(&ids))
                .is_none()
            {
                seen_count += 1;
            }
            let mut edges = Vec::with_capacity(current.edges.len() + 1);
            edges.extend_from_slice(&current.edges);
            edges.push(*edge_index);
            order += 1;
            pending.push(BoundedPath {
                bound,
                tail: Rc::clone(&rest.ids),
                order,
                position: edge.end,
                text,
                ids,
                edges,
                score,
                last_character: entry.last,
            });
        }
    }
    complete
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct InterpretedRank {
    cost: InterpretationCost,
    preference: u32,
    score: i64,
}

impl InterpretedRank {
    fn from_path(path: &Path) -> Self {
        Self {
            cost: path.features.interpretation_cost,
            preference: path.features.user_preference_units,
            score: path.score,
        }
    }

    fn then(self, other: Self) -> Self {
        Self {
            cost: self.cost.plus(other.cost),
            preference: self.preference + other.preference,
            score: self.score + other.score,
        }
    }

    fn order(self, other: Self) -> Ordering {
        self.cost
            .cmp(&other.cost)
            .then_with(|| other.preference.cmp(&self.preference))
            .then_with(|| other.score.cmp(&self.score))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InterpretedStep {
    entry: u32,
    edge: u32,
}

impl InterpretedStep {
    fn new(entry: usize, edge: usize) -> Self {
        // 词条载入上限 500 万，图边配置上限 10 万。
        Self {
            entry: entry as u32,
            edge: edge as u32,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InterpretedSuffix {
    rank: InterpretedRank,
    steps: Rc<[InterpretedStep]>,
}

fn step_order(
    left: impl Iterator<Item = InterpretedStep> + Clone,
    right: impl Iterator<Item = InterpretedStep> + Clone,
) -> Ordering {
    left.clone()
        .map(|step| step.entry)
        .cmp(right.clone().map(|step| step.entry))
        .then_with(|| left.map(|step| step.edge).cmp(right.map(|step| step.edge)))
}

fn suffix_order(a: &InterpretedSuffix, b: &InterpretedSuffix) -> Ordering {
    a.rank
        .order(b.rank)
        .then_with(|| step_order(a.steps.iter().copied(), b.steps.iter().copied()))
}

fn path_steps(path: &Path) -> impl Iterator<Item = InterpretedStep> + Clone + '_ {
    path.ids
        .iter()
        .zip(&path.edges)
        .map(|(&entry, &edge)| InterpretedStep::new(entry, edge))
}

impl InterpretedSuffix {
    fn matches(&self, path: &Path) -> bool {
        self.rank == InterpretedRank::from_path(path)
            && self.steps.iter().copied().eq(path_steps(path))
    }

    fn compare_completed(&self, other: &Path) -> Ordering {
        self.rank
            .order(InterpretedRank::from_path(other))
            .then_with(|| step_order(self.steps.iter().copied(), path_steps(other)))
    }
}

// 先借用原路径和后缀比较完整顺序，只有必须保留时才拷贝序列。
#[derive(Clone, Copy)]
struct InterpretedExtension<'a> {
    prefix: &'a Path,
    step: InterpretedStep,
    tail: &'a [InterpretedStep],
    rank: InterpretedRank,
}

impl InterpretedExtension<'_> {
    fn steps(&self) -> impl Iterator<Item = InterpretedStep> + Clone + '_ {
        path_steps(self.prefix)
            .chain(std::iter::once(self.step))
            .chain(self.tail.iter().copied())
    }

    fn order(&self, other: &InterpretedSuffix) -> Ordering {
        self.rank
            .order(other.rank)
            .then_with(|| step_order(self.steps(), other.steps.iter().copied()))
    }

    fn order_path(&self, other: &Path) -> Ordering {
        self.rank
            .order(InterpretedRank::from_path(other))
            .then_with(|| step_order(self.steps(), path_steps(other)))
    }

    fn save(self) -> InterpretedSuffix {
        InterpretedSuffix {
            rank: self.rank,
            steps: self.steps().collect(),
        }
    }
}

struct InterpretedPending {
    bound: InterpretedSuffix,
    order: usize,
    position: usize,
    path: Path,
}
impl PartialEq for InterpretedPending {
    fn eq(&self, other: &Self) -> bool {
        self.order == other.order
    }
}
impl Eq for InterpretedPending {}
impl PartialOrd for InterpretedPending {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for InterpretedPending {
    fn cmp(&self, other: &Self) -> Ordering {
        suffix_order(&other.bound, &self.bound).then_with(|| other.order.cmp(&self.order))
    }
}

/// 完整解释成本/偏好与字符历史共同决定后缀；稀疏转移避免历史和词条的笛卡尔展开。
fn interpreted_character_exact<O: Observer>(
    scorer: &Scorer<'_>,
    config: &Config,
    stats: &mut Statistics,
    k: usize,
    observer: &mut O,
) -> Vec<Path> {
    let model = scorer.model.unwrap().character.as_ref().unwrap();
    let end = scorer.graph.letters.len();
    let max_edits = scorer.graph.max_edit_cost();
    let mut suffix = vec![BTreeMap::<(u32, u16), Option<InterpretedSuffix>>::new(); end + 1];
    suffix[0].insert((crate::character::BOS, max_edits), None);
    let mut labels = 1;
    for edge in &scorer.graph.edges {
        for remaining in 0..=max_edits {
            if scorer.cancelled() || !expand_allowed(stats, config) {
                return Vec::new();
            }
            if suffix[edge.end]
                .insert((model.entry(edge.entry).last, remaining), None)
                .is_none()
            {
                labels += 1;
                if !resident_allowed(stats, labels + 1, config) {
                    return Vec::new();
                }
            }
        }
    }
    let empty: Rc<[InterpretedStep]> = Rc::from([]);
    for (&(previous, _), value) in &mut suffix[end] {
        *value = Some(InterpretedSuffix {
            rank: InterpretedRank {
                score: model.finish(previous),
                ..InterpretedRank::default()
            },
            steps: Rc::clone(&empty),
        });
    }
    let root = Path::default();
    for position in (0..end).rev() {
        for remaining in 0..=max_edits {
            let mut groups = BTreeMap::<u32, InterpretedSuffix>::new();
            for edge_index in &scorer.graph.outgoing[position] {
                if scorer.cancelled() || !expand_allowed(stats, config) {
                    return Vec::new();
                }
                let edge = &scorer.graph.edges[*edge_index];
                let cost = scorer.graph.interpretation_cost(*edge_index);
                if cost.edits > remaining {
                    continue;
                }
                let entry = model.entry(edge.entry);
                let Some(rest) = suffix[edge.end]
                    .get(&(entry.last, remaining - cost.edits))
                    .and_then(Option::as_ref)
                else {
                    continue;
                };
                let candidate = InterpretedExtension {
                    prefix: &root,
                    step: InterpretedStep::new(edge.entry, *edge_index),
                    tail: &rest.steps,
                    rank: InterpretedRank {
                        cost,
                        preference: scorer.lexicon.user_preference(edge.entry),
                        score: entry.internal,
                    }
                    .then(rest.rank),
                };
                if groups
                    .get(&entry.first)
                    .is_none_or(|old| candidate.order(old).is_lt())
                {
                    if !resident_allowed(stats, labels + groups.len() + 2, config) {
                        return Vec::new();
                    }
                    groups.insert(entry.first, candidate.save());
                }
            }
            let Some(class) = groups.values().min_by(|a, b| {
                a.rank
                    .cost
                    .cmp(&b.rank.cost)
                    .then_with(|| b.rank.preference.cmp(&a.rank.preference))
            }) else {
                continue;
            };
            let (cost, preference) = (class.rank.cost, class.rank.preference);
            groups.retain(|_, s| s.rank.cost == cost && s.rank.preference == preference);
            let mut fallback: Vec<_> = groups.iter().collect();
            fallback.sort_by(|(a_key, a), (b_key, b)| {
                (b.rank.score + model.unobserved_next(**b_key))
                    .cmp(&(a.rank.score + model.unobserved_next(**a_key)))
                    .then_with(|| step_order(a.steps.iter().copied(), b.steps.iter().copied()))
            });
            if !resident_allowed(stats, labels + groups.len() + fallback.len() + 1, config) {
                return Vec::new();
            }
            let histories: Vec<_> = suffix[position]
                .keys()
                .filter(|(_, r)| *r == remaining)
                .copied()
                .collect();
            for (previous, remaining) in histories {
                let stamp = observer.start(Phase::Scoring);
                let rows = model.transition_row(previous);
                let mut best: Option<(&InterpretedSuffix, i64)> = None;
                for row in rows {
                    if scorer.cancelled() || !expand_allowed(stats, config) {
                        observer.stop(Phase::Scoring, stamp);
                        return Vec::new();
                    }
                    if let Some(group) = groups.get(&(row.key as u32)) {
                        let score = group.rank.score + row.units;
                        if best.is_none_or(|(old, value)| {
                            score > value
                                || (score == value
                                    && step_order(
                                        group.steps.iter().copied(),
                                        old.steps.iter().copied(),
                                    )
                                    .is_lt())
                        }) {
                            best = Some((group, score));
                        }
                    }
                }
                for (next, group) in &fallback {
                    if scorer.cancelled() || !expand_allowed(stats, config) {
                        observer.stop(Phase::Scoring, stamp);
                        return Vec::new();
                    }
                    if rows
                        .binary_search_by_key(*next, |row| row.key as u32)
                        .is_err()
                    {
                        let score = group.rank.score
                            + model.unobserved_bias(previous)
                            + model.unobserved_next(**next);
                        if best.is_none_or(|(old, value)| {
                            score > value
                                || (score == value
                                    && step_order(
                                        group.steps.iter().copied(),
                                        old.steps.iter().copied(),
                                    )
                                    .is_lt())
                        }) {
                            best = Some((group, score));
                        }
                        break;
                    }
                }
                let best = best.map(|(value, score)| {
                    let mut result = value.clone();
                    result.rank.score = score;
                    result
                });
                observer.stop(Phase::Scoring, stamp);
                suffix[position].insert((previous, remaining), best);
            }
        }
    }
    let Some(best) = suffix[0]
        .get(&(crate::character::BOS, max_edits))
        .and_then(Option::as_ref)
    else {
        return Vec::new();
    };
    let mut complete = Vec::<Path>::new();
    // 后缀界本身给出有效完整路径；先建立实际TopK门槛，再展开有机会改善它的前缀。
    for edge_index in &scorer.graph.outgoing[0] {
        if scorer.cancelled() || !expand_allowed(stats, config) {
            return Vec::new();
        }
        let edge = &scorer.graph.edges[*edge_index];
        let used = scorer.graph.interpretation_cost(*edge_index).edits;
        if used > max_edits {
            continue;
        }
        let entry = model.entry(edge.entry);
        let Some(rest) = suffix[edge.end]
            .get(&(entry.last, max_edits - used))
            .and_then(Option::as_ref)
        else {
            continue;
        };
        if !resident_allowed(stats, labels + complete.len() + 3, config) {
            return Vec::new();
        }
        let extension = InterpretedExtension {
            prefix: &root,
            step: InterpretedStep::new(edge.entry, *edge_index),
            tail: &rest.steps,
            rank: InterpretedRank {
                cost: scorer.graph.interpretation_cost(*edge_index),
                preference: scorer.lexicon.user_preference(edge.entry),
                score: model.transition(crate::character::BOS, entry.first) + entry.internal,
            }
            .then(rest.rank),
        };
        if complete.len() == k && !extension.order_path(complete.last().unwrap()).is_lt() {
            continue;
        }
        let Some(mut candidate) = scorer.extend(&root, *edge_index, None, observer) else {
            continue;
        };
        // 后缀标签已按剩余编辑数验证，沿该链扩展必然可行。
        for step in rest.steps.iter() {
            if scorer.cancelled() || !expand_allowed(stats, config) {
                return Vec::new();
            }
            candidate = scorer
                .extend(&candidate, step.edge as usize, None, observer)
                .unwrap();
        }
        let candidate = scorer.finish(candidate, observer);
        debug_assert_eq!(extension.order_path(&candidate), Ordering::Equal);
        insert_unique(&mut complete, candidate, k, observer);
    }
    if !resident_allowed(stats, labels + complete.len() + 2, config) {
        return Vec::new();
    }
    let mut pending = BinaryHeap::from([InterpretedPending {
        bound: best.clone(),
        order: 0,
        position: 0,
        path: Path::default(),
    }]);
    let mut order = 0;
    let mut seen =
        vec![
            vec![BTreeMap::<String, InterpretedSuffix>::new(); usize::from(max_edits) + 1];
            end + 1
        ];
    seen[0][0].insert(
        String::new(),
        InterpretedSuffix {
            rank: InterpretedRank::default(),
            steps: empty,
        },
    );
    let mut seen_count = 1;
    let mut scratch = String::new();
    while let Some(current) = pending.pop() {
        if scorer.cancelled() {
            return Vec::new();
        }
        let edits = current.path.features.interpretation_cost.edits;
        if !seen[current.position][usize::from(edits)]
            .get(&current.path.text)
            .is_some_and(|old| old.matches(&current.path))
        {
            continue;
        }
        if complete.len() == k
            && current.bound.compare_completed(complete.last().unwrap()) != Ordering::Less
        {
            break;
        }
        if current.position == end {
            let finished = scorer.finish(current.path, observer);
            insert_unique(&mut complete, finished, k, observer);
            continue;
        }
        for edge_index in &scorer.graph.outgoing[current.position] {
            if scorer.cancelled() || !expand_allowed(stats, config) {
                return Vec::new();
            }
            let edge = &scorer.graph.edges[*edge_index];
            let used = edits + scorer.graph.interpretation_cost(*edge_index).edits;
            if used > max_edits {
                continue;
            }
            let entry = model.entry(edge.entry);
            let Some(rest) = suffix[edge.end]
                .get(&(entry.last, max_edits - used))
                .and_then(Option::as_ref)
            else {
                continue;
            };
            if !resident_allowed(
                stats,
                labels + seen_count + pending.len() + complete.len() + 3,
                config,
            ) {
                return Vec::new();
            }
            let prefix_rank = InterpretedRank::from_path(&current.path).then(InterpretedRank {
                cost: scorer.graph.interpretation_cost(*edge_index),
                preference: scorer.lexicon.user_preference(edge.entry),
                score: model.transition(
                    current.path.last_character.unwrap_or(crate::character::BOS),
                    entry.first,
                ) + entry.internal,
            });
            let extension = InterpretedExtension {
                prefix: &current.path,
                step: InterpretedStep::new(edge.entry, *edge_index),
                tail: &rest.steps,
                rank: prefix_rank.then(rest.rank),
            };
            if complete.len() == k && !extension.order_path(complete.last().unwrap()).is_lt() {
                continue;
            }
            let signature = InterpretedExtension {
                tail: &[],
                rank: prefix_rank,
                ..extension
            };
            let bucket = &mut seen[edge.end][usize::from(used)];
            scratch.clear();
            scratch.push_str(&current.path.text);
            scratch.push_str(&scorer.lexicon.entries[edge.entry].text);
            if bucket
                .get(scratch.as_str())
                .is_some_and(|old| !signature.order(old).is_lt())
            {
                continue;
            }
            let Some(next) = scorer.extend(&current.path, *edge_index, None, observer) else {
                continue;
            };
            let bound = extension.save();
            if bucket.insert(next.text.clone(), signature.save()).is_none() {
                seen_count += 1;
            }
            order += 1;
            pending.push(InterpretedPending {
                bound,
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
    let no_terminal_edge = graph.budget == Some(Budget::GraphEdges)
        && graph.is_interpreted()
        && graph
            .edges
            .iter()
            .all(|edge| edge.end < graph.letters.len());
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
        // 词汇分只依赖该边词条；每次查询清空，不跨版本、修订或输入方案复用。
        lexical_cache: (!no_terminal_edge
            && config.ranker == Ranker::Lexical
            && config.method == Method::PositionDag)
            .then(|| vec![Cell::new(i32::MIN); graph.edges.len()]),
    };
    let stamp = observer.start(Phase::Traversal);
    // 触顶且缺末端边时，任何方法都无法形成覆盖全部输入的候选。
    let mut paths = if graph.letters.is_empty() || graph.waiting || no_terminal_edge {
        Vec::new()
    } else if config.method == Method::Reference {
        reference(&scorer, config, &mut statistics, k, observer)
    } else if config.method == Method::ExactDag
        && config.ranker == Ranker::CharacterBigram
        && (graph.is_interpreted() || lexicon.has_user_preferences())
    {
        interpreted_character_exact(&scorer, config, &mut statistics, k, observer)
    } else if config.method == Method::PositionDag && graph.max_edit_cost() == 0 {
        // 无历史评分、无编辑记账时，每个位置只有一个桶。仍按逐弧来源比较。
        position_dynamic(&scorer, config, &mut statistics, k, observer)
    } else if graph.is_interpreted() || lexicon.has_user_preferences() {
        interpreted_dynamic(&scorer, config, &mut statistics, k, observer)
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

#[cfg(test)]
mod interpretation_regression {
    use super::*;
    use crate::InterpretationCost;

    fn path(text: &str, score: i64, ids: &[usize], cost: InterpretationCost) -> Path {
        Path {
            text: text.into(),
            score,
            ids: ids.to_vec(),
            features: Features {
                interpretation_cost: cost,
                ..Features::default()
            },
            ..Path::default()
        }
    }

    #[test]
    fn exact_interpretation_precedes_better_language_score_without_mixing_units() {
        let exact = path("甲", -10000, &[9], InterpretationCost::default());
        let edited = path(
            "乙",
            10000,
            &[0],
            InterpretationCost {
                edits: 1,
                ..InterpretationCost::default()
            },
        );
        assert_eq!(compare(&exact, &edited), Ordering::Less);
        assert_eq!(exact.score, -10000);
        assert_eq!(edited.score, 10000);
    }

    #[test]
    fn duplicate_text_keeps_best_interpretation_before_score_or_entry_id() {
        let mut bucket = Vec::new();
        let fuzzy = InterpretationCost {
            fuzzy: 1,
            ..InterpretationCost::default()
        };
        insert_unique(&mut bucket, path("甲", 1000, &[0], fuzzy), 5, &mut Noop);
        insert_unique(
            &mut bucket,
            path("甲", -1000, &[9], InterpretationCost::default()),
            5,
            &mut Noop,
        );
        assert_eq!(bucket.len(), 1);
        assert_eq!(bucket[0].ids, [9]);
        assert_eq!(
            bucket[0].features.interpretation_cost,
            InterpretationCost::default()
        );
    }

    #[test]
    fn cutoff_cannot_discard_an_exact_path_for_an_edited_high_score() {
        let lexicon = Lexicon::from_tsv("# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\ta\t甲\t\tunknown\t\tfixture\ta\tMIT\n").unwrap();
        let graph = Lattice::build(&lexicon, "a", &Config::default()).unwrap();
        let scorer = Scorer {
            lexicon: &lexicon,
            graph: &graph,
            ranker: Ranker::Structural,
            model: None,
            cancellation: None,
            lexical_cache: None,
        };
        let worst = path(
            "乙",
            10000,
            &[0],
            InterpretationCost {
                edits: 1,
                ..InterpretationCost::default()
            },
        );
        let result = scorer
            .extend(&Path::default(), 0, Some(&worst), &mut Noop)
            .unwrap();
        assert_eq!(result.text, "甲");
        assert_eq!(result.score, -100);
        assert_eq!(
            result.features.interpretation_cost,
            InterpretationCost::default()
        );
    }
}
