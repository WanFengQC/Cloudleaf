"""仅在临时基线副本上试验已登记的成本上限。"""
from pathlib import Path
p=Path(__file__).resolve().parent
target=p/'source/crates/cloudleaf-search/src/search.rs'
s=target.read_text(encoding='utf-8')
(p/'baseline-search.rs').write_text(s,encoding='utf-8')
s=s.replace('type InterpretationStates =', (p/'cheap-bound.rs').read_text(encoding='utf-8')+'type InterpretationStates =',1)
start=s.index('fn interpreted_dynamic<')
tail=s.index('fn position_dynamic<',start)
part=s[start:tail]
part=part.replace('observer: &mut O,\n) -> Vec<Path>', 'observer: &mut O,\n    ceiling: Option<&CostCeiling>,\n) -> Vec<Path>',1)
part=part.replace('''                let history = if config.method == Method::PositionDag {''','''                if let Some(ceiling)=ceiling {
                    let remaining=usize::from(scorer.graph.max_edit_cost()-used_edits);
                    if ceiling.suffix[edge.end][remaining].is_none_or(|rest|path.features.interpretation_cost.plus(scorer.graph.interpretation_cost(*edge_index)).plus(rest)>ceiling.worst) {continue;}
                }
                let history = if config.method == Method::PositionDag {''',1)
s=s[:start]+part+s[tail:]
s=s.replace('''        interpreted_dynamic(&scorer, config, &mut statistics, k, observer)''','''        let ceiling=if config.method==Method::ExactDag && scorer.ranker==Ranker::CharacterBigram {interpretation_ceiling(&scorer,config,&mut statistics,k,observer)} else {None};
        if statistics.budget.is_some(){Vec::new()}else{interpreted_dynamic(&scorer, config, &mut statistics, k, observer,ceiling.as_ref())}''',1)
target.write_text(s,encoding='utf-8')
