struct CostCeiling {
    worst: InterpretationCost,
    suffix: Vec<Vec<Option<InterpretationCost>>>,
}

fn interpretation_ceiling<O: Observer>(
    scorer: &Scorer<'_>,
    config: &Config,
    stats: &mut Statistics,
    k: usize,
    observer: &mut O,
) -> Option<CostCeiling> {
    let seed_scorer = Scorer {
        lexicon: scorer.lexicon,
        graph: scorer.graph,
        ranker: Ranker::Structural,
        model: None,
        cancellation: scorer.cancellation,
    };
    let seed_config = Config {method: Method::PositionDag, ranker:Ranker::Structural,..config.clone()};
    let mut seeds = interpreted_dynamic(&seed_scorer, &seed_config, stats, k, observer, None);
    if stats.budget.is_some() { return None; }
    retain_unique(&mut seeds,k,observer,Phase::BucketSortDedup);
    if seeds.len()<k { return None; }
    let worst=seeds.last().unwrap().features.interpretation_cost;
    let end=scorer.graph.letters.len();
    let edits=usize::from(scorer.graph.max_edit_cost());
    let mut suffix=vec![vec![None;edits+1];end+1];
    for remaining in 0..=edits { suffix[end][remaining]=Some(InterpretationCost::default()); }
    for position in (0..end).rev() {
        for remaining in 0..=edits {
            for edge_index in &scorer.graph.outgoing[position] {
                if scorer.cancelled() || !expand_allowed(stats, config) { return None; }
                let edge=&scorer.graph.edges[*edge_index];
                let cost=scorer.graph.interpretation_cost(*edge_index);
                if usize::from(cost.edits)>remaining {continue;}
                if let Some(rest)=suffix[edge.end][remaining-usize::from(cost.edits)] {
                    let next=cost.plus(rest);
                    if suffix[position][remaining].is_none_or(|old|next<old){suffix[position][remaining]=Some(next);}
                }
            }
        }
    }
    Some(CostCeiling{worst,suffix})
}

