use crate::character::{BOS as CHARACTER_BOS, CharacterModel};
use crate::model::log2_fixed;
use crate::{BigramModel, ErrorKind, Lattice, Lexicon, SearchError, SearchResult};
use std::cmp::Ordering;
use std::sync::Arc;

const START: &str = "<s>";
const END: &str = "</s>";
const UNKNOWN: &str = "<unk>";
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_ROWS: usize = 200_000;
const MAX_COUNT: u64 = 1_100_000;
const PRIOR: u64 = 8;
pub const MAX_RESEARCH_POOL: usize = 50;

#[derive(Clone, Debug)]
pub struct WordContextModel {
    lexicon_sha256: String,
    corpus_sha256: String,
    identity: Arc<()>,
    words: Vec<String>,
    counts: Vec<u64>,
    entries: Vec<u32>,
    pairs: Vec<Pair>,
    starts: Vec<usize>,
    numerator_units: Vec<i64>,
    denominator_units: Vec<i64>,
    unigram_total: u64,
    unigram_total_units: i64,
    vocabulary: u64,
    bos: u32,
    eos: u32,
    unknown: u32,
}

#[derive(Clone, Debug)]
struct Pair {
    previous: u32,
    next: u32,
    observed: u64,
    units: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResearchRanker {
    Unigram,
    Word,
    Character,
    Mixed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResearchScore {
    pub pool_index: usize,
    pub base_units: i64,
    pub unigram_units: i64,
    pub word_units: i64,
    pub character_units: Option<i64>,
    pub boundary_units: i64,
    pub observed_words: usize,
    pub unknown_words: usize,
    pub final_units: i64,
}

fn invalid() -> SearchError {
    SearchError::new(ErrorKind::InvalidModel, "词语统计格式、词典或计数绑定无效")
}

fn sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn count(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value
        .parse::<u64>()
        .ok()
        .filter(|number| (1..=MAX_COUNT).contains(number))
}

fn metadata<'a>(line: Option<&'a str>, key: &str) -> Result<&'a str, SearchError> {
    let (actual, value) = line
        .and_then(|line| line.split_once('\t'))
        .ok_or_else(invalid)?;
    if actual != key || !sha(value) {
        return Err(invalid());
    }
    Ok(value)
}

fn id(words: &[String], value: &str) -> Option<u32> {
    words
        .binary_search_by(|word| word.as_str().cmp(value))
        .ok()
        .map(|index| index as u32)
}

impl WordContextModel {
    pub fn from_tsv(
        text: &str,
        lexicon: &Lexicon,
        expected_lexicon: &str,
    ) -> Result<Self, SearchError> {
        if text.len() > MAX_BYTES || text.contains('\r') || !text.ends_with('\n') {
            return Err(invalid());
        }
        let mut lines = text.lines();
        if lines.next() != Some("# cloudleaf-word-context-v1") {
            return Err(invalid());
        }
        let lexicon_sha256 = metadata(lines.next(), "lexicon_sha256")?;
        if lexicon_sha256 != expected_lexicon {
            return Err(invalid());
        }
        let corpus_sha256 = metadata(lines.next(), "corpus_sha256")?;
        let mut words = Vec::<String>::new();
        let mut counts = Vec::new();
        let mut pairs = Vec::new();
        let mut seen_pairs = false;
        let surfaces: std::collections::BTreeSet<_> = lexicon
            .entries()
            .iter()
            .map(|entry| entry.text.as_str())
            .collect();
        for (index, line) in lines.enumerate() {
            if index >= MAX_ROWS {
                return Err(invalid());
            }
            let mut fields = line.split('\t');
            match (
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
            ) {
                (Some("W"), Some(word), Some(observed), None, None) if !seen_pairs => {
                    if !matches!(word, START | END | UNKNOWN) && !surfaces.contains(word) {
                        return Err(invalid());
                    }
                    if words.last().is_some_and(|last| last.as_str() >= word) {
                        return Err(invalid());
                    }
                    words.push(word.to_owned());
                    counts.push(count(observed).ok_or_else(invalid)?);
                }
                (Some("B"), Some(previous), Some(next), Some(observed), None) => {
                    if !seen_pairs && id(&words, UNKNOWN).is_none() {
                        let index = words.partition_point(|value| value.as_str() < UNKNOWN);
                        words.insert(index, UNKNOWN.to_owned());
                        counts.insert(index, 0);
                    }
                    seen_pairs = true;
                    let previous = id(&words, previous).ok_or_else(invalid)?;
                    let next = id(&words, next).ok_or_else(invalid)?;
                    if pairs
                        .last()
                        .is_some_and(|last: &Pair| (last.previous, last.next) >= (previous, next))
                    {
                        return Err(invalid());
                    }
                    pairs.push(Pair {
                        previous,
                        next,
                        observed: count(observed).ok_or_else(invalid)?,
                        units: 0,
                    });
                }
                _ => return Err(invalid()),
            }
        }
        let bos = id(&words, START).ok_or_else(invalid)?;
        let eos = id(&words, END).ok_or_else(invalid)?;
        let unknown = id(&words, UNKNOWN).ok_or_else(invalid)?;
        let mut outgoing = vec![0u64; words.len()];
        let mut incoming = vec![0u64; words.len()];
        for pair in &pairs {
            outgoing[pair.previous as usize] += pair.observed;
            incoming[pair.next as usize] += pair.observed;
        }
        let edge_count: u64 = outgoing.iter().sum();
        if pairs.is_empty()
            || edge_count > MAX_COUNT
            || counts[bos as usize] != counts[eos as usize]
            || counts[bos as usize] > 100_000
            || incoming[bos as usize] != 0
            || outgoing[eos as usize] != 0
            || counts.iter().enumerate().any(|(index, &observed)| {
                (index != eos as usize && outgoing[index] != observed)
                    || (index != bos as usize && incoming[index] != observed)
            })
        {
            return Err(invalid());
        }
        let vocabulary = (words.len() - 1) as u64;
        let unigram_total = edge_count + vocabulary;
        let mut starts = vec![0; words.len() + 1];
        for pair in &mut pairs {
            let previous = pair.previous as usize;
            let next = pair.next as usize;
            starts[previous + 1] += 1;
            pair.units = transition_units(
                pair.observed,
                outgoing[previous],
                counts[next],
                unigram_total,
            );
        }
        for index in 1..starts.len() {
            starts[index] += starts[index - 1];
        }
        let mut reached = vec![false; words.len()];
        reached[bos as usize] = true;
        let mut stack = vec![bos as usize];
        while let Some(previous) = stack.pop() {
            for pair in &pairs[starts[previous]..starts[previous + 1]] {
                let next = pair.next as usize;
                if !reached[next] {
                    reached[next] = true;
                    stack.push(next);
                }
            }
        }
        if counts
            .iter()
            .zip(reached)
            .any(|(&count, seen)| count != 0 && !seen)
        {
            return Err(invalid());
        }
        let numerator_units = counts.iter().map(|count| log2_fixed(count + 1)).collect();
        let denominator_units = outgoing
            .iter()
            .map(|count| log2_fixed((count + PRIOR) * unigram_total))
            .collect();
        let entries = lexicon
            .entries()
            .iter()
            .map(|entry| id(&words, &entry.text).unwrap_or(unknown))
            .collect();
        Ok(Self {
            lexicon_sha256: lexicon_sha256.to_owned(),
            corpus_sha256: corpus_sha256.to_owned(),
            identity: lexicon.identity.clone(),
            words,
            counts,
            entries,
            pairs,
            starts,
            numerator_units,
            denominator_units,
            unigram_total,
            unigram_total_units: log2_fixed(unigram_total),
            vocabulary,
            bos,
            eos,
            unknown,
        })
    }

    pub fn corpus_sha256(&self) -> &str {
        &self.corpus_sha256
    }

    pub fn lexicon_sha256(&self) -> &str {
        &self.lexicon_sha256
    }

    pub fn is_compatible_lexicon(&self, lexicon: &Lexicon) -> bool {
        Arc::ptr_eq(&self.identity, &lexicon.identity)
    }

    pub fn observed_count(&self, word: &str) -> Option<u64> {
        id(&self.words, word)
            .map(|index| self.counts[index as usize])
            .filter(|value| *value != 0)
    }

    pub fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.lexicon_sha256.capacity()
            + self.corpus_sha256.capacity()
            + self.words.iter().map(String::capacity).sum::<usize>()
            + self.words.capacity() * std::mem::size_of::<String>()
            + self.counts.capacity() * std::mem::size_of::<u64>()
            + self.entries.capacity() * std::mem::size_of::<u32>()
            + self.pairs.capacity() * std::mem::size_of::<Pair>()
            + self.starts.capacity() * std::mem::size_of::<usize>()
            + (self.numerator_units.capacity() + self.denominator_units.capacity())
                * std::mem::size_of::<i64>()
    }

    fn unigram(&self, next: u32) -> i64 {
        self.numerator_units[next as usize] - self.unigram_total_units
    }

    fn transition(&self, previous: u32, next: u32) -> i64 {
        let previous_index = previous as usize;
        let rows = &self.pairs[self.starts[previous_index]..self.starts[previous_index + 1]];
        if let Ok(index) = rows.binary_search_by_key(&next, |pair| pair.next) {
            return rows[index].units;
        }
        self.numerator_units[next as usize] + log2_fixed(PRIOR)
            - self.denominator_units[previous_index]
    }

    fn literal_transition(&self, previous: u32, next: u32) -> i64 {
        let mut outgoing = 0;
        let mut observed = 0;
        for pair in &self.pairs {
            if pair.previous == previous {
                outgoing += pair.observed;
                if pair.next == next {
                    observed += pair.observed;
                }
            }
        }
        transition_units(
            observed,
            outgoing,
            self.counts[next as usize],
            self.unigram_total,
        )
    }

    fn score(
        &self,
        graph: &Lattice<'_>,
        pool: &SearchResult,
        index: usize,
        ranker: ResearchRanker,
        character: Option<&CharacterModel>,
        literal: bool,
    ) -> ResearchScore {
        let candidate = &pool.candidates[index];
        let transition = if literal {
            Self::literal_transition
        } else {
            Self::transition
        };
        let mut previous = self.bos;
        let mut observed_words = 0;
        let mut unknown_words = 0;
        let mut words = 0i64;
        let mut unigram_units = 0;
        let mut word_units = 0;
        let mut boundary_units = 0;
        let mut character_units = 0;
        let mut character_count = 0;
        let mut last_character = CHARACTER_BOS;
        for &edge in &candidate.edge_indices {
            let entry = graph.edges[edge].entry;
            let next = if literal {
                id(&self.words, &graph.lexicon.entries()[entry].text).unwrap_or(self.unknown)
            } else {
                self.entries[entry]
            };
            if next == self.unknown {
                unknown_words += 1;
            } else {
                observed_words += 1;
            }
            unigram_units += self.unigram(next);
            let current = transition(self, previous, next);
            if previous == self.bos {
                boundary_units += current;
            }
            word_units += current;
            previous = next;
            words += 1;
            if let Some(model) = character {
                let score = model.entry(entry);
                character_units += model.transition(last_character, score.first) + score.internal;
                character_count += score.characters as i64;
                last_character = score.last;
            }
        }
        let finish = transition(self, previous, self.eos);
        boundary_units += finish;
        word_units = (word_units + finish) / (words + 1);
        unigram_units /= words.max(1);
        let character_units = character
            .map(|model| (character_units + model.finish(last_character)) / (character_count + 1));
        let contents = match ranker {
            ResearchRanker::Unigram => unigram_units,
            ResearchRanker::Word => word_units,
            ResearchRanker::Character => character_units.unwrap_or(0),
            ResearchRanker::Mixed => (word_units + character_units.unwrap_or(0)) / 2,
        };
        ResearchScore {
            pool_index: index,
            base_units: candidate.score,
            unigram_units,
            word_units,
            character_units,
            boundary_units,
            observed_words,
            unknown_words,
            final_units: contents + candidate.features.structural_units,
        }
    }

    fn shape<'a>(
        &self,
        graph: &Lattice<'_>,
        pool: &SearchResult,
        ranker: ResearchRanker,
        character: Option<&'a BigramModel>,
    ) -> Result<Option<&'a CharacterModel>, SearchError> {
        if !Arc::ptr_eq(&self.identity, &graph.lexicon.identity)
            || pool.raw_fallback != graph.raw()
            || pool.candidates.len() > MAX_RESEARCH_POOL
            || pool.candidates.iter().any(|item| {
                item.edge_indices.is_empty()
                    || item
                        .edge_indices
                        .iter()
                        .any(|&index| index >= graph.edges.len())
            })
            || character.is_some_and(|model| !model.is_compatible_lexicon(graph.lexicon))
        {
            return Err(invalid());
        }
        let model = character.and_then(|model| model.character.as_ref());
        if model.is_none() && matches!(ranker, ResearchRanker::Character | ResearchRanker::Mixed) {
            return Err(invalid());
        }
        Ok(model)
    }

    /// 只重排现有去重候选池，返回的下标仍引用原候选正文和边。
    pub fn rank_pool(
        &self,
        graph: &Lattice<'_>,
        pool: &SearchResult,
        ranker: ResearchRanker,
        character: Option<&BigramModel>,
    ) -> Result<Vec<ResearchScore>, SearchError> {
        let model = self.shape(graph, pool, ranker, character)?;
        let mut scores: Vec<_> = (0..pool.candidates.len())
            .map(|index| self.score(graph, pool, index, ranker, model, false))
            .collect();
        scores.sort_by(|left, right| compare(pool, left, right));
        Ok(scores)
    }

    /// 小规模审计按原始计数重算，逐项插入，不读取编译边分值。
    pub fn reference_pool(
        &self,
        graph: &Lattice<'_>,
        pool: &SearchResult,
        ranker: ResearchRanker,
        character: Option<&BigramModel>,
    ) -> Result<Vec<ResearchScore>, SearchError> {
        let model = self.shape(graph, pool, ranker, character)?;
        if self.pairs.len().saturating_mul(
            pool.candidates
                .iter()
                .map(|item| item.edge_indices.len() + 1)
                .sum(),
        ) > 12_000_000
        {
            return Err(SearchError::new(
                ErrorKind::InvalidConfig,
                "小规模重排对照上限",
            ));
        }
        let mut scores = Vec::with_capacity(pool.candidates.len());
        for index in 0..pool.candidates.len() {
            let value = self.score(graph, pool, index, ranker, model, true);
            let insertion = scores
                .iter()
                .position(|current| compare(pool, &value, current).is_lt())
                .unwrap_or(scores.len());
            scores.insert(insertion, value);
        }
        Ok(scores)
    }

    pub fn vocabulary_size(&self) -> u64 {
        self.vocabulary
    }

    pub fn pair_count(&self) -> usize {
        self.pairs.len()
    }

    pub(crate) fn recall_start(&self) -> u32 {
        self.bos
    }

    pub(crate) fn recall_end(&self) -> u32 {
        self.eos
    }

    pub(crate) fn recall_word(&self, entry: usize) -> u32 {
        self.entries[entry]
    }

    pub(crate) fn recall_gain(&self, previous: u32, next: u32) -> i64 {
        if previous == self.unknown || next == self.unknown {
            return 0;
        }
        let index = previous as usize;
        let rows = &self.pairs[self.starts[index]..self.starts[index + 1]];
        let Some(row) = rows
            .binary_search_by_key(&next, |pair| pair.next)
            .ok()
            .map(|index| &rows[index])
            .filter(|row| row.observed >= 2)
        else {
            return 0;
        };
        (row.units - self.unigram(next)).clamp(0, 1024)
    }
}

fn transition_units(pair_count: u64, prior_count: u64, word_count: u64, total: u64) -> i64 {
    log2_fixed(pair_count * total + PRIOR * (word_count + 1))
        - log2_fixed((prior_count + PRIOR) * total)
}

fn compare(pool: &SearchResult, left: &ResearchScore, right: &ResearchScore) -> Ordering {
    let left_path = &pool.candidates[left.pool_index];
    let right_path = &pool.candidates[right.pool_index];
    left_path
        .features
        .interpretation_cost
        .cmp(&right_path.features.interpretation_cost)
        .then_with(|| {
            right_path
                .features
                .user_preference_units
                .cmp(&left_path.features.user_preference_units)
        })
        .then_with(|| right.final_units.cmp(&left.final_units))
        .then_with(|| left.pool_index.cmp(&right.pool_index))
}

/// 只对声明正文做有界图遍历。None 表示图未完成或独立诊断超出工作预算。
pub fn expected_reachability(graph: &Lattice<'_>, expected: &str, limit: usize) -> Option<bool> {
    if graph.budget().is_some() {
        return None;
    }
    if expected.is_empty() || graph.waiting() || graph.letters().is_empty() {
        return Some(false);
    }
    let mut reached = vec![std::collections::BTreeSet::new(); graph.letters().len() + 1];
    reached[0].insert((0, 0u16));
    let mut visited = 0;
    for (pos, outgoing) in graph.outgoing.iter().enumerate() {
        let positions = std::mem::take(&mut reached[pos]);
        if pos == graph.letters().len() {
            return Some(
                positions
                    .iter()
                    .any(|(offset, _)| *offset == expected.len()),
            );
        }
        for (offset, cost) in positions {
            for &index in outgoing {
                visited += 1;
                if visited > limit {
                    return None;
                }
                let edge = &graph.edges[index];
                let text = graph.lexicon.entries()[edge.entry].text.as_str();
                let cost = cost + graph.interpretation_cost(index).edits;
                if cost <= graph.max_edit_cost() && expected[offset..].starts_with(text) {
                    reached[edge.end].insert((offset + text.len(), cost));
                }
            }
        }
    }
    Some(false)
}
