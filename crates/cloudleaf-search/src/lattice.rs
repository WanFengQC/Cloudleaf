use crate::profiling::{Noop, Observer, Phase};
use crate::{Budget, Cancellation, Config, ErrorKind, Lexicon, SearchError};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchKind {
    Exact,
    Prefix,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edge {
    pub start: usize,
    pub end: usize,
    pub raw_start: usize,
    pub raw_end: usize,
    pub entry: usize,
    pub kind: MatchKind,
}

#[derive(Clone, Debug)]
pub struct Lattice<'a> {
    pub(crate) lexicon: &'a Lexicon,
    pub(crate) raw: String,
    pub(crate) letters: String,
    pub(crate) raw_offsets: Vec<usize>,
    pub(crate) waiting: bool,
    pub(crate) edges: Vec<Edge>,
    pub(crate) outgoing: Vec<Vec<usize>>,
    pub(crate) budget: Option<Budget>,
}

impl<'a> Lattice<'a> {
    pub fn build(lexicon: &'a Lexicon, input: &str, config: &Config) -> Result<Self, SearchError> {
        Self::build_observed(lexicon, input, config, &mut Noop)
    }

    pub fn build_cancellable(
        lexicon: &'a Lexicon,
        input: &str,
        config: &Config,
        cancellation: &Cancellation,
    ) -> Result<Self, SearchError> {
        Self::build_controlled_observed(lexicon, input, config, Some(cancellation), &mut Noop)
    }

    pub(crate) fn build_observed<O: Observer>(
        lexicon: &'a Lexicon,
        input: &str,
        config: &Config,
        observer: &mut O,
    ) -> Result<Self, SearchError> {
        Self::build_controlled_observed(lexicon, input, config, None, observer)
    }

    pub(crate) fn build_controlled_observed<O: Observer>(
        lexicon: &'a Lexicon,
        input: &str,
        config: &Config,
        cancellation: Option<&Cancellation>,
        observer: &mut O,
    ) -> Result<Self, SearchError> {
        let stamp = observer.start(Phase::Normalize);
        let normalized = normalize(input, config, cancellation);
        observer.stop(Phase::Normalize, stamp);
        let normalized = normalized?;
        let stamp = observer.start(Phase::Lattice);
        let graph = Self::from_normalized(lexicon, input, config, normalized, cancellation);
        observer.stop(Phase::Lattice, stamp);
        if let Some(control) = cancellation {
            control.check()?;
        }
        graph
    }

    fn from_normalized(
        lexicon: &'a Lexicon,
        input: &str,
        config: &Config,
        normalized: Normalized,
        cancellation: Option<&Cancellation>,
    ) -> Result<Self, SearchError> {
        let (letters, raw_offsets, separators, waiting) = normalized;
        if let Some(control) = cancellation {
            control.check()?;
        }
        let mut graph = Self {
            lexicon,
            raw: input.into(),
            outgoing: vec![Vec::new(); letters.len() + 1],
            letters,
            raw_offsets,
            waiting,
            edges: Vec::new(),
            budget: None,
        };
        if waiting {
            return Ok(graph);
        }
        'positions: for start in 0..graph.letters.len() {
            if let Some(control) = cancellation {
                control.check()?;
            }
            let mut node = 0;
            for end in start + 1..=graph.letters.len() {
                let byte = graph.letters.as_bytes()[end - 1];
                let Some(next) = lexicon.trie[node].next.get(&byte) else {
                    break;
                };
                node = *next;
                let exact = lexicon.trie[node]
                    .exact
                    .iter()
                    .map(|entry| (*entry, MatchKind::Exact));
                let prefix = lexicon.trie[node]
                    .prefix
                    .iter()
                    .filter(|_| end == graph.letters.len())
                    .map(|entry| (*entry, MatchKind::Prefix));
                for (entry_index, kind) in exact.chain(prefix) {
                    if let Some(control) = cancellation {
                        control.check()?;
                    }
                    let entry = &lexicon.entries[entry_index];
                    if separators
                        .range(start + 1..end)
                        .any(|boundary| !entry.boundaries.contains(&(boundary - start)))
                    {
                        continue;
                    }
                    if graph.edges.len() == config.max_edges {
                        graph.budget = Some(Budget::GraphEdges);
                        break 'positions;
                    }
                    let edge_index = graph.edges.len();
                    graph.edges.push(Edge {
                        start,
                        end,
                        raw_start: graph.raw_offsets[start],
                        raw_end: graph.raw_offsets[end],
                        entry: entry_index,
                        kind,
                    });
                    graph.outgoing[start].push(edge_index);
                }
            }
        }
        if let Some(control) = cancellation {
            control.check()?;
        }
        Ok(graph)
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn letters(&self) -> &str {
        &self.letters
    }
    pub fn raw_offsets(&self) -> &[usize] {
        &self.raw_offsets
    }
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }
    pub fn waiting(&self) -> bool {
        self.waiting
    }
    pub fn budget(&self) -> Option<Budget> {
        self.budget
    }
}

type Normalized = (String, Vec<usize>, BTreeSet<usize>, bool);

fn normalize(
    input: &str,
    config: &Config,
    cancellation: Option<&Cancellation>,
) -> Result<Normalized, SearchError> {
    if let Some(control) = cancellation {
        control.check()?;
    }
    config.validate()?;
    let invalid = || SearchError::new(ErrorKind::InvalidInput, "输入字符或长度无效");
    if input.len() > config.max_input_bytes {
        return Err(invalid());
    }
    let mut letters = String::new();
    let mut raw_offsets = vec![0];
    let mut separators = BTreeSet::new();
    let mut waiting = false;
    for (offset, ch) in input.char_indices() {
        if let Some(control) = cancellation {
            control.check()?;
        }
        match ch {
            '\'' => {
                if letters.is_empty() || waiting {
                    return Err(invalid());
                }
                separators.insert(letters.len());
                waiting = true;
            }
            'a'..='z' | 'A'..='Z' | 'ü' | 'Ü' => {
                letters.push(if ch == 'ü' || ch == 'Ü' {
                    'v'
                } else {
                    ch.to_ascii_lowercase()
                });
                raw_offsets.push(offset + ch.len_utf8());
                waiting = false;
            }
            _ => return Err(invalid()),
        }
        if letters.len() > config.max_letters {
            return Err(invalid());
        }
    }
    Ok((letters, raw_offsets, separators, waiting))
}

#[cfg(test)]
mod cancellation_regression {
    use super::*;

    struct CancelAtBoundary<'a> {
        control: &'a Cancellation,
        phase: Phase,
    }

    impl Observer for CancelAtBoundary<'_> {
        type Stamp = ();
        fn start(&mut self, _: Phase) {}
        fn stop(&mut self, phase: Phase, _: ()) {
            if phase == self.phase {
                self.control.cancel();
            }
        }
    }

    #[test]
    fn cancelled_empty_or_waiting_graph_is_not_published_after_phase_boundary() {
        let lexicon = Lexicon::from_tsv("# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\ta\t甲\t\tunknown\t\tfixture\ta\tMIT\n").unwrap();
        for raw in ["", "a'", "a"] {
            for phase in [Phase::Normalize, Phase::Lattice] {
                let control = Cancellation::default();
                let mut observer = CancelAtBoundary {
                    control: &control,
                    phase,
                };
                assert_eq!(
                    Lattice::build_controlled_observed(
                        &lexicon,
                        raw,
                        &Config::default(),
                        Some(&control),
                        &mut observer
                    )
                    .unwrap_err()
                    .kind,
                    ErrorKind::Cancelled
                );
            }
        }
    }
}
