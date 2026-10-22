//! 全输入词条格实验内核。词典与模型由宿主预加载，查询不访问外部资源。
mod lattice;
mod lexicon;
mod model;
mod search;

pub use lattice::{Edge, Lattice, MatchKind};
pub use lexicon::{Entry, FrequencyKind, Lexicon, LoadLimits};
pub use model::BigramModel;
pub use search::{Candidate, Features, SearchResult, Statistics, search_graph};

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    InvalidLexicon,
    InvalidInput,
    InvalidConfig,
    InvalidModel,
}

/// 错误不包含输入原文、词条正文或上下文。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchError {
    pub kind: ErrorKind,
    pub line: Option<usize>,
    pub message: &'static str,
}

impl SearchError {
    pub(crate) fn new(kind: ErrorKind, message: &'static str) -> Self {
        Self {
            kind,
            line: None,
            message,
        }
    }

    pub(crate) fn at_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)?;
        if let Some(line) = self.line {
            write!(f, " (line {line})")?;
        }
        Ok(())
    }
}

impl std::error::Error for SearchError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Method {
    Reference,
    ExactDag,
    Beam,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ranker {
    Structural,
    Weighted,
    Bigram,
    ContextRerank,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Budget {
    GraphEdges,
    Expansions,
    ResidentPaths,
}

/// 确定性工作预算；墙钟时间不参与剪枝。上界控制调用者配置的最坏资源范围。
#[derive(Clone, Debug)]
pub struct Config {
    pub method: Method,
    pub ranker: Ranker,
    pub output_k: usize,
    pub context_pool: usize,
    pub beam_width: usize,
    pub max_input_bytes: usize,
    pub max_letters: usize,
    pub max_edges: usize,
    pub max_expansions: usize,
    pub max_resident_paths: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            method: Method::ExactDag,
            ranker: Ranker::Weighted,
            output_k: 5,
            context_pool: 20,
            beam_width: 64,
            max_input_bytes: 512,
            max_letters: 256,
            max_edges: 16_384,
            max_expansions: 200_000,
            max_resident_paths: 50_000,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), SearchError> {
        let valid = (1..=50).contains(&self.output_k)
            && (self.output_k..=100).contains(&self.context_pool)
            && (1..=4096).contains(&self.beam_width)
            && (1..=1024).contains(&self.max_input_bytes)
            && (1..=512).contains(&self.max_letters)
            && (1..=100_000).contains(&self.max_edges)
            && (1..=2_000_000).contains(&self.max_expansions)
            && (1..=200_000).contains(&self.max_resident_paths);
        if valid {
            Ok(())
        } else {
            Err(SearchError::new(
                ErrorKind::InvalidConfig,
                "搜索预算超出允许范围",
            ))
        }
    }
}
