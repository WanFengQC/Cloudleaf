use crate::{Budget, Config, ErrorKind, Lexicon, SearchError};
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
