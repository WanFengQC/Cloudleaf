//! 仅显式研究入口读钟；默认入口使用零状态观察器，共用全部算法实现。
use crate::{BigramModel, Config, Lattice, Lexicon, SearchError, SearchResult};
use std::{collections::BTreeMap, time::Instant};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Phase {
    QueryTotal,
    Normalize,
    Lattice,
    SearchValidate,
    Traversal,
    Scoring,
    BucketSortDedup,
    FinalSortDedup,
    ContextRerank,
    Materialize,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Self::QueryTotal => "query_total",
            Self::Normalize => "normalize",
            Self::Lattice => "lattice",
            Self::SearchValidate => "search_validate",
            Self::Traversal => "traversal",
            Self::Scoring => "scoring",
            Self::BucketSortDedup => "bucket_sort_dedup",
            Self::FinalSortDedup => "final_sort_dedup",
            Self::ContextRerank => "context_rerank",
            Self::Materialize => "materialize",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Measurement {
    pub nanos: u128,
    pub calls: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Profile {
    phases: BTreeMap<Phase, Measurement>,
    clock_reads: u64,
}

impl Profile {
    pub fn phase(&self, phase: Phase) -> Option<&Measurement> {
        self.phases.get(&phase)
    }
    pub fn measurements(&self) -> impl Iterator<Item = (&Phase, &Measurement)> {
        self.phases.iter()
    }
    pub fn clock_reads(&self) -> u64 {
        self.clock_reads
    }
}

pub(crate) trait Observer {
    type Stamp;
    fn start(&mut self, phase: Phase) -> Self::Stamp;
    fn stop(&mut self, phase: Phase, stamp: Self::Stamp);
}

pub(crate) struct Noop;
impl Observer for Noop {
    type Stamp = ();
    #[inline(always)]
    fn start(&mut self, _: Phase) {}
    #[inline(always)]
    fn stop(&mut self, _: Phase, _: ()) {}
}

impl Observer for Profile {
    type Stamp = Instant;
    fn start(&mut self, _: Phase) -> Instant {
        self.clock_reads += 1;
        Instant::now()
    }
    fn stop(&mut self, phase: Phase, stamp: Instant) {
        let nanos = stamp.elapsed().as_nanos();
        self.clock_reads += 1;
        let measurement = self.phases.entry(phase).or_default();
        measurement.nanos += nanos;
        measurement.calls += 1;
    }
}

/// 显式离线阶段观测。桶排序嵌套于路径遍历，不能将所有阶段相加。
pub fn profile_query<'a>(
    lexicon: &'a Lexicon,
    input: &str,
    config: &Config,
    model: Option<&BigramModel>,
    left_word: Option<&str>,
) -> (Result<(Lattice<'a>, SearchResult), SearchError>, Profile) {
    let mut profile = Profile::default();
    let total = profile.start(Phase::QueryTotal);
    let result = Lattice::build_observed(lexicon, input, config, &mut profile).and_then(|graph| {
        crate::search::search_observed(lexicon, &graph, config, model, left_word, &mut profile)
            .map(|result| (graph, result))
    });
    profile.stop(Phase::QueryTotal, total);
    (result, profile)
}
