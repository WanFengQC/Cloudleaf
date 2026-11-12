from pathlib import Path
import difflib,json,subprocess,os,hashlib
root=Path(__file__).resolve().parent
src=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
p=root/'crates/cloudleaf-search/src/search.rs';base=p.read_text(encoding='utf-8')
tests='''

#[cfg(test)]
mod interpretation_regression {
    use super::*;
    use crate::InterpretationCost;

    fn path(text: &str, score: i64, ids: &[usize], cost: InterpretationCost) -> Path {
        Path {
            text: text.into(), score, ids: ids.to_vec(),
            features: Features { interpretation_cost: cost, ..Features::default() },
            ..Path::default()
        }
    }

    #[test]
    fn exact_interpretation_precedes_better_language_score_without_mixing_units() {
        let exact = path("甲", -10000, &[9], InterpretationCost::default());
        let edited = path("乙", 10000, &[0], InterpretationCost { edits: 1, ..InterpretationCost::default() });
        assert_eq!(compare(&exact, &edited), Ordering::Less);
        assert_eq!(exact.score, -10000);
        assert_eq!(edited.score, 10000);
    }

    #[test]
    fn duplicate_text_keeps_best_interpretation_before_score_or_entry_id() {
        let mut bucket = Vec::new();
        let fuzzy = InterpretationCost { fuzzy: 1, ..InterpretationCost::default() };
        insert_unique(&mut bucket, path("甲", 1000, &[0], fuzzy), 5, &mut Noop);
        insert_unique(&mut bucket, path("甲", -1000, &[9], InterpretationCost::default()), 5, &mut Noop);
        assert_eq!(bucket.len(), 1);
        assert_eq!(bucket[0].ids, [9]);
        assert_eq!(bucket[0].features.interpretation_cost, InterpretationCost::default());
    }

    #[test]
    fn cutoff_cannot_discard_an_exact_path_for_an_edited_high_score() {
        let lexicon = Lexicon::from_tsv("# cloudleaf-lexicon-v2\\nid\\tpinyin\\ttext\\tfrequency\\tfrequency_kind\\tannotation\\tsource\\tsource_record_id\\tlicense_id\\na\\ta\\t甲\\t\\tunknown\\t\\tfixture\\ta\\tMIT\\n").unwrap();
        let graph = Lattice::build(&lexicon, "a", &Config::default()).unwrap();
        let scorer = Scorer { lexicon: &lexicon, graph: &graph, ranker: Ranker::Structural, model: None, cancellation: None };
        let worst = path("乙", 10000, &[0], InterpretationCost { edits: 1, ..InterpretationCost::default() });
        let result = scorer.extend(&Path::default(), 0, Some(&worst), &mut Noop).unwrap();
        assert_eq!(result.text, "甲");
        assert_eq!(result.score, -100);
        assert_eq!(result.features.interpretation_cost, InterpretationCost::default());
    }
}
'''
red=base+tests
(root/'search-tests-only.rs').write_text(red,encoding='utf-8',newline='\n')
s=base.replace('Ranker, SearchError,','Ranker, SearchError, InterpretationCost,')
s=s.replace('pub struct Features {','pub struct Features {\n    pub interpretation_cost: InterpretationCost,')
s=s.replace('    b.score\n        .cmp(&a.score)','    a.features.interpretation_cost\n        .cmp(&b.features.interpretation_cost)\n        .then_with(|| b.score.cmp(&a.score))')
s=s.replace('        let mut features = path.features.clone();','''        let mut features = path.features.clone();
        features.interpretation_cost = features.interpretation_cost.plus(self.graph.interpretation_cost(edge_index));
        if features.interpretation_cost.edits > self.graph.max_edit_cost() {
            observer.stop(Phase::Scoring, stamp);
            return None;
        }''')
s=s.replace('''            worst
                .score
                .cmp(&score)''','''            features.interpretation_cost
                .cmp(&worst.features.interpretation_cost)
                .then_with(|| worst.score.cmp(&score))''')
s=s.replace('''            pending.push((
                scorer.graph.edges[*edge].end,
                scorer.extend(&path, *edge, None, observer).unwrap(),
            ));''','''            if let Some(next) = scorer.extend(&path, *edge, None, observer) {
                pending.push((scorer.graph.edges[*edge].end, next));
            }''')
where=s.index('\nfn position_dynamic')
new='''
/// 解释成本与语言历史分别保存；已用编辑量影响剩余可行后缀，不能按位置丢弃。
fn interpreted_dynamic<O: Observer>(
    scorer: &Scorer<'_>,
    config: &Config,
    stats: &mut Statistics,
    k: usize,
    observer: &mut O,
) -> Vec<Path> {
    let mut states: Vec<BTreeMap<(Option<usize>, u16), Vec<Path>>> =
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
                if scorer.cancelled() { return Vec::new(); }
                if !expand_allowed(stats, config) { return Vec::new(); }
                let edge = &scorer.graph.edges[*edge_index];
                let used_edits = path.features.interpretation_cost.edits
                    + scorer.graph.interpretation_cost(*edge_index).edits;
                if used_edits > scorer.graph.max_edit_cost() { continue; }
                let history = if config.method == Method::PositionDag {
                    None
                } else if scorer.ranker == Ranker::CharacterBigram {
                    Some(scorer.model.unwrap().character.as_ref().unwrap().entry(edge.entry).last as usize)
                } else {
                    Some(edge.entry)
                };
                let bucket = states[edge.end].entry((history, used_edits)).or_default();
                let old = bucket.len();
                if !resident_allowed(stats, resident + 1, config) { return Vec::new(); }
                let cutoff = (old == k).then(|| bucket.last().unwrap());
                if let Some(next) = scorer.extend(&path, *edge_index, cutoff, observer) {
                    insert_unique(bucket, next, k, observer);
                }
                resident = resident + bucket.len() - old;
            }
            resident -= 1;
        }
    }
    states.pop().unwrap_or_default().into_values().flatten()
        .map(|path| scorer.finish(path, observer)).collect()
}
'''
s=s[:where]+new+s[where:]
s=s.replace('''    } else if config.method == Method::PositionDag {
        position_dynamic''','''    } else if graph.is_interpreted() {
        interpreted_dynamic(&scorer, config, &mut statistics, k, observer)
    } else if config.method == Method::PositionDag {
        position_dynamic''')
s+=tests
p.write_text(s,encoding='utf-8',newline='\n')
# 临时编译桩只在本目录，不是交付补丁的一部分。
stub='''
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct InterpretationCost { pub edits: u16, pub fuzzy: u16, pub abbreviations: u16, pub prefixes: u16 }
impl InterpretationCost { pub fn plus(self, other: Self) -> Self { Self { edits: self.edits+other.edits, fuzzy:self.fuzzy+other.fuzzy, abbreviations:self.abbreviations+other.abbreviations, prefixes:self.prefixes+other.prefixes } } }
impl Lattice<'_> {
    pub fn interpretation_cost(&self, _edge: usize) -> InterpretationCost { InterpretationCost::default() }
    pub fn is_interpreted(&self) -> bool { false }
    pub fn max_edit_cost(&self) -> u16 { 1 }
}
'''
lib=root/'crates/cloudleaf-search/src/lib.rs';lib.write_text(lib.read_text(encoding='utf-8')+stub,encoding='utf-8',newline='\n')
def patch(before,after):
    return ''.join(difflib.unified_diff(before.splitlines(keepends=True),after.splitlines(keepends=True),fromfile='a/crates/cloudleaf-search/src/search.rs',tofile='b/crates/cloudleaf-search/src/search.rs'))
(root/'tests-only.patch').write_text(patch(base,red),encoding='utf-8',newline='\n')
(root/'implementation-after-tests.patch').write_text(patch(red,s),encoding='utf-8',newline='\n')
(root/'complete.patch').write_text(patch(base,s),encoding='utf-8',newline='\n')
print(json.dumps({'patches':{n:hashlib.sha256((root/n).read_bytes()).hexdigest() for n in ['tests-only.patch','implementation-after-tests.patch','complete.patch']}}))
