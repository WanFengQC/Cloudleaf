from pathlib import Path
p=Path(__file__).resolve().parent
s=p/'source/crates/cloudleaf-search/src/search.rs'
t=(p/'baseline-search.rs').read_text(encoding='utf-8')
start=t.index('fn interpreted_character_exact')
a,b=t[:start],t[start:]
at='    let mut pending = BinaryHeap::from([InterpretedPending {'
seed='''    let mut complete = Vec::<Path>::new();
    // 后缀界本身给出有效完整路径；先建立实际TopK门槛，再展开有机会改善它的前缀。
    for edge_index in &scorer.graph.outgoing[0] {
        if scorer.cancelled() || !expand_allowed(stats, config) {
            return Vec::new();
        }
        let edge = &scorer.graph.edges[*edge_index];
        let used = scorer.graph.interpretation_cost(*edge_index).edits;
        if used > max_edits {
            continue;
        }
        let entry = model.entry(edge.entry);
        let Some(rest) = suffix[edge.end]
            .get(&(entry.last, max_edits - used))
            .and_then(Option::as_ref)
        else { continue; };
        if !resident_allowed(stats, labels + complete.len() + 3, config) { return Vec::new(); }
        let Some(mut candidate) = scorer.extend(&Path::default(), *edge_index, None, observer) else { continue; };
        let bound = InterpretedSuffix::completed(&candidate, rest);
        if complete.len() == k && suffix_order(&bound, &InterpretedSuffix::prefix(complete.last().unwrap())) != Ordering::Less {
            continue;
        }
        for tail in &rest.edges {
            if scorer.cancelled() || !expand_allowed(stats, config) { return Vec::new(); }
            candidate = scorer.extend(&candidate, *tail, None, observer).unwrap();
        }
        let candidate = scorer.finish(candidate, observer);
        insert_unique(&mut complete, candidate, k, observer);
    }
'''
b=b.replace(at,seed+at,1).replace('    let mut complete = Vec::<Path>::new();\n    let mut seen =','    let mut seen =',1)
s.write_text(a+b,encoding='utf-8',newline='\n')
