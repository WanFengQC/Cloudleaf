from pathlib import Path
import difflib
import subprocess

root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=Path(__file__).parent/'cancellation'
out.mkdir(exist_ok=False)
paths=['src/lib.rs','src/lattice.rs','src/search.rs']
old={p:subprocess.check_output(['git','show','75e0ace:crates/cloudleaf-search/'+p],cwd=root).decode('utf-8') for p in paths}
new=dict(old)
new['src/lib.rs']=new['src/lib.rs'].replace('mod lattice;','mod cancellation;\nmod lattice;').replace('pub use lattice::','pub use cancellation::Cancellation;\npub use lattice::').replace('Statistics, search_graph};','Statistics, search_graph, search_graph_cancellable};').replace('    InvalidModel,\n','    InvalidModel,\n    Cancelled,\n')
new['src/cancellation.rs']='''use crate::{ErrorKind, SearchError};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

/// 仅传递取消意图，不同步结果或输入上下文；取消后不能复用为新请求。
#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub(crate) fn check(&self) -> Result<(), SearchError> {
        if self.is_cancelled() {
            Err(SearchError::new(ErrorKind::Cancelled, "搜索已取消"))
        } else {
            Ok(())
        }
    }
}
'''
s=new['src/lattice.rs']
s=s.replace('use crate::{Budget, Config, ErrorKind, Lexicon, SearchError};','use crate::{Budget, Cancellation, Config, ErrorKind, Lexicon, SearchError};')
needle='''    pub(crate) fn build_observed<O: Observer>(
        lexicon: &'a Lexicon,
        input: &str,
        config: &Config,
        observer: &mut O,
    ) -> Result<Self, SearchError> {'''
replacement='''    pub fn build_cancellable(
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
    ) -> Result<Self, SearchError> {'''
assert needle in s;s=s.replace(needle,replacement)
s=s.replace('normalize(input, config);','normalize(input, config, cancellation);')
s=s.replace('            waiting,\n        );','            waiting,\n            cancellation,\n        );')
s=s.replace('        Ok(graph)\n','        graph\n',1)
s=s.replace('        waiting: bool,\n    ) -> Self {','        waiting: bool,\n        cancellation: Option<&Cancellation>,\n    ) -> Result<Self, SearchError> {')
s=s.replace('            return graph;','            return Ok(graph);')
s=s.replace("        'positions: for start in 0..graph.letters.len() {", "        'positions: for start in 0..graph.letters.len() {\n            if let Some(control) = cancellation { control.check()?; }")
s=s.replace('                for (entry_index, kind) in exact.chain(prefix) {','                for (entry_index, kind) in exact.chain(prefix) {\n                    if let Some(control) = cancellation { control.check()?; }')
s=s.replace('        graph\n    }\n\n    pub fn raw', '        if let Some(control) = cancellation { control.check()?; }\n        Ok(graph)\n    }\n\n    pub fn raw')
s=s.replace('fn normalize(input: &str, config: &Config) -> Result<Normalized, SearchError> {','fn normalize(input: &str, config: &Config, cancellation: Option<&Cancellation>) -> Result<Normalized, SearchError> {\n    if let Some(control) = cancellation { control.check()?; }')
s=s.replace('    for (offset, ch) in input.char_indices() {','    for (offset, ch) in input.char_indices() {\n        if let Some(control) = cancellation { control.check()?; }')
new['src/lattice.rs']=s
s=new['src/search.rs']
s=s.replace('    BigramModel, Budget, Config, ErrorKind,','    BigramModel, Budget, Cancellation, Config, ErrorKind,')
s=s.replace("    model: Option<&'a BigramModel>,\n}","    model: Option<&'a BigramModel>,\n    cancellation: Option<&'a Cancellation>,\n}")
s=s.replace("impl Scorer<'_> {", "impl Scorer<'_> {\n    fn cancelled(&self) -> bool {\n        self.cancellation.is_some_and(Cancellation::is_cancelled)\n    }\n")
s=s.replace('    while let Some((position, path)) = pending.pop() {','    while let Some((position, path)) = pending.pop() {\n        if scorer.cancelled() { return Vec::new(); }')
s=s.replace('            if !expand_allowed(stats, config) {','            if scorer.cancelled() { return Vec::new(); }\n            if !expand_allowed(stats, config) {')
needle='''pub(crate) fn search_observed<O: Observer>(
    lexicon: &Lexicon,
    graph: &Lattice<'_>,
    config: &Config,
    model: Option<&BigramModel>,
    left_word: Option<&str>,
    observer: &mut O,
) -> Result<SearchResult, SearchError> {'''
replacement='''pub fn search_graph_cancellable(
    lexicon: &Lexicon,
    graph: &Lattice<'_>,
    config: &Config,
    model: Option<&BigramModel>,
    left_word: Option<&str>,
    cancellation: &Cancellation,
) -> Result<SearchResult, SearchError> {
    search_controlled_observed(lexicon, graph, config, model, left_word, Some(cancellation), &mut Noop)
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
    if let Some(control) = cancellation { control.check()?; }'''
assert needle in s;s=s.replace(needle,replacement)
s=s.replace('        ranker: config.ranker,\n        model,\n','        ranker: config.ranker,\n        model,\n        cancellation,\n')
s=s.replace('    retain_unique(&mut paths, k, observer, Phase::FinalSortDedup);','    if let Some(control) = cancellation { control.check()?; }\n    retain_unique(&mut paths, k, observer, Phase::FinalSortDedup);')
s=s.replace('    observer.stop(Phase::Materialize, stamp);\n    Ok(result)','    observer.stop(Phase::Materialize, stamp);\n    if let Some(control) = cancellation { control.check()?; }\n    Ok(result)')
s+='''
#[cfg(test)]
mod cancellation_regression {
    use super::*;

    struct CancelDuringScoring<'a> { control: &'a Cancellation, scored: usize }
    impl Observer for CancelDuringScoring<'_> {
        type Stamp = ();
        fn start(&mut self, phase: Phase) {
            if phase == Phase::Scoring {
                self.scored += 1;
                if self.scored == 2 { self.control.cancel(); }
            }
        }
        fn stop(&mut self, _: Phase, _: ()) {}
    }

    const DATA: &str = "# cloudleaf-lexicon-v2\\nid\\tpinyin\\ttext\\tfrequency\\tfrequency_kind\\tannotation\\tsource\\tsource_record_id\\tlicense_id\\na\\ta\\t甲\\t\\tunknown\\t\\tfixture\\ta\\tMIT\\nb\\ta\\t乙\\t\\tunknown\\t\\tfixture\\tb\\tMIT\\n";

    #[test]
    fn pre_cancelled_graph_and_search_return_cancelled_not_budget() {
        let lexicon = Lexicon::from_tsv(DATA).unwrap();
        let config = Config::default();
        let control = Cancellation::default();
        let clone = control.clone();
        clone.cancel();
        assert!(control.is_cancelled());
        assert_eq!(Lattice::build_cancellable(&lexicon, "aa", &config, &control).unwrap_err().kind, ErrorKind::Cancelled);
        let graph = Lattice::build(&lexicon, "aa", &config).unwrap();
        assert_eq!(search_graph_cancellable(&lexicon, &graph, &config, None, None, &control).unwrap_err().kind, ErrorKind::Cancelled);
    }

    #[test]
    fn actual_traversal_cancellation_discards_partial_paths_for_every_method() {
        let lexicon = Lexicon::from_tsv(DATA).unwrap();
        for method in [Method::Reference, Method::ExactDag, Method::PositionDag, Method::Beam] {
            let config = Config { method, ranker: Ranker::Structural, ..Config::default() };
            let graph = Lattice::build(&lexicon, "aaaa", &config).unwrap();
            let control = Cancellation::default();
            let mut observer = CancelDuringScoring { control: &control, scored: 0 };
            assert_eq!(search_controlled_observed(&lexicon, &graph, &config, None, None, Some(&control), &mut observer).unwrap_err().kind, ErrorKind::Cancelled);
            assert_eq!(observer.scored, 2);
        }
    }

    #[test]
    fn uncancelled_entry_preserves_results_and_original_budget_semantics() {
        let lexicon = Lexicon::from_tsv(DATA).unwrap();
        for method in [Method::Reference, Method::ExactDag, Method::PositionDag, Method::Beam] {
            for budget in [1, 200000] {
                let config = Config { method, ranker: Ranker::Structural, max_expansions: budget, ..Config::default() };
                let control = Cancellation::default();
                let graph = Lattice::build_cancellable(&lexicon, "aaaa", &config, &control).unwrap();
                let full = search_graph(&lexicon, &graph, &config, None, None).unwrap();
                let controlled = search_graph_cancellable(&lexicon, &graph, &config, None, None, &control).unwrap();
                assert_eq!(full, controlled);
                if budget == 1 { assert_eq!(controlled.statistics.budget, Some(Budget::Expansions)); }
            }
        }
    }
}
'''
new['src/search.rs']=s
for file in (root/'crates/cloudleaf-search/src').iterdir():
    if file.is_file() and 'src/'+file.name not in new: new['src/'+file.name]=file.read_text(encoding='utf-8')
for p,s in new.items():
    file=out/p;file.parent.mkdir(parents=True,exist_ok=True);file.write_text(s,encoding='utf-8',newline='\n')
(out/'Cargo.toml').write_text('''[package]
name = "r15-cancellation-review"
version = "0.1.0"
edition = "2024"
[dependencies]
cloudleaf-pinyin = { path = "C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf/crates/cloudleaf-pinyin" }
[lints.clippy]
all = "warn"
''',encoding='utf-8')

def diff():
    patch=[]
    for p in paths+['src/cancellation.rs']:
        patch.extend(difflib.unified_diff(old.get(p,'').splitlines(keepends=True),new[p].splitlines(keepends=True),fromfile='a/crates/cloudleaf-search/'+p,tofile='b/crates/cloudleaf-search/'+p))
    (out.parent/'cancellation.patch').write_text(''.join(patch),encoding='utf-8',newline='\n')
diff()
