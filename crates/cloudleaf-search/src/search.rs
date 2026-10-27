use crate::profiling::{Noop, Observer, Phase};
use crate::{
    BigramModel, Budget, Config, ErrorKind, Lattice, Lexicon, MatchKind, Method, Ranker,
    SearchError,
};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Features {
    pub words: usize,
    pub prefixes: usize,
    pub frequency_units: i64,
    pub missing_frequencies: usize,
    pub bigram_units: i64,
    pub context_units: i64,
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
}

fn compare(a: &Path, b: &Path) -> Ordering {
    b.score.cmp(&a.score).then_with(|| a.ids.cmp(&b.ids))
}

struct Scorer<'a> {
    lexicon: &'a Lexicon,
    graph: &'a Lattice<'a>,
    ranker: Ranker,
    model: Option<&'a BigramModel>,
}

impl Scorer<'_> {
    fn extend(&self, path: &Path, edge_index: usize) -> Path {
        let edge = &self.graph.edges[edge_index];
        let entry = &self.lexicon.entries[edge.entry];
        let mut next = path.clone();
        next.text.push_str(&entry.text);
        next.features.words += 1;
        next.score -= 100;
        if edge.kind == MatchKind::Prefix {
            next.features.prefixes += 1;
            next.score -= 300;
        }
        if let Some(frequency) = entry.frequency {
            let units = i64::from(63 - (u64::from(frequency) + 1).leading_zeros());
            next.features.frequency_units += units;
            if self.ranker != Ranker::Structural {
                next.score += units * 10;
            }
        } else {
            next.features.missing_frequencies += 1;
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
            next.features.bigram_units += units;
            next.score += units;
        }
        next.ids.push(edge.entry);
        next.edges.push(edge_index);
        next
    }

    fn finish(&self, mut path: Path) -> Path {
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
        if position == scorer.graph.letters.len() {
            complete.push(scorer.finish(path));
            // 参考枚举所有路径，只将已完成输出压缩为当前最佳unique top-k。
            retain_unique(&mut complete, k, observer, Phase::BucketSortDedup);
            continue;
        }
        for edge in &scorer.graph.outgoing[position] {
            if !expand_allowed(stats, config) {
                return complete;
            }
            // 当前被pop的路径和extend新克隆同时存活。
            if !resident_allowed(stats, pending.len() + complete.len() + 2, config) {
                return complete;
            }
            pending.push((scorer.graph.edges[*edge].end, scorer.extend(&path, *edge)));
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
    // last词条是未来一阶评分的充分状态；文本去重在每个状态内进行。
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
                if !expand_allowed(stats, config) {
                    return Vec::new();
                }
                let edge = &scorer.graph.edges[*edge_index];
                let bucket = states[edge.end].entry(Some(edge.entry)).or_default();
                let old = bucket.len();
                if !resident_allowed(stats, resident + 1, config) {
                    return Vec::new();
                }
                bucket.push(scorer.extend(&path, *edge_index));
                retain_unique(bucket, k, observer, Phase::BucketSortDedup);
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
        .map(|path| scorer.finish(path))
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
                if !expand_allowed(stats, config) {
                    return Vec::new();
                }
                let edge = &scorer.graph.edges[*edge_index];
                let bucket = &mut states[edge.end];
                let old = bucket.len();
                if !resident_allowed(stats, resident + 1, config) {
                    return Vec::new();
                }
                bucket.push(scorer.extend(&path, *edge_index));
                retain_unique(bucket, k, observer, Phase::BucketSortDedup);
                resident = resident + bucket.len() - old;
            }
            resident -= 1;
        }
    }
    states.pop().unwrap_or_default()
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

pub(crate) fn search_observed<O: Observer>(
    lexicon: &Lexicon,
    graph: &Lattice<'_>,
    config: &Config,
    model: Option<&BigramModel>,
    left_word: Option<&str>,
    observer: &mut O,
) -> Result<SearchResult, SearchError> {
    let stamp = observer.start(Phase::SearchValidate);
    let validation = (|| {
        config.validate()?;
        if matches!(config.ranker, Ranker::Bigram | Ranker::ContextRerank) && model.is_none() {
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
    };
    let stamp = observer.start(Phase::Traversal);
    let mut paths = if graph.letters.is_empty() || graph.waiting {
        Vec::new()
    } else if config.method == Method::Reference {
        reference(&scorer, config, &mut statistics, k, observer)
    } else if config.method == Method::PositionDag {
        position_dynamic(&scorer, config, &mut statistics, k, observer)
    } else {
        dynamic(&scorer, config, &mut statistics, k, observer)
    };
    observer.stop(Phase::Traversal, stamp);
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
    Ok(result)
}
