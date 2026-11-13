#[derive(Clone, Debug, Eq, PartialEq)]
struct InterpretedSuffix {
    cost: InterpretationCost,
    preference: u32,
    score: i64,
    ids: Vec<usize>,
    edges: Vec<usize>,
}

fn suffix_order(a: &InterpretedSuffix, b: &InterpretedSuffix) -> Ordering {
    a.cost.cmp(&b.cost)
        .then_with(||b.preference.cmp(&a.preference))
        .then_with(||b.score.cmp(&a.score))
        .then_with(||a.ids.cmp(&b.ids))
        .then_with(||a.edges.cmp(&b.edges))
}

impl InterpretedSuffix {
    fn prefix(path: &Path) -> Self {
        Self {cost:path.features.interpretation_cost,preference:path.features.user_preference_units,
            score:path.score,ids:path.ids.clone(),edges:path.edges.clone()}
    }
    fn completed(path:&Path,rest:&Self)->Self {
        Self {cost:path.features.interpretation_cost.plus(rest.cost),
            preference:path.features.user_preference_units+rest.preference,
            score:path.score+rest.score,
            ids:path.ids.iter().chain(&rest.ids).copied().collect(),
            edges:path.edges.iter().chain(&rest.edges).copied().collect()}
    }
}

struct InterpretedPending {
    bound: InterpretedSuffix,
    order: usize,
    position: usize,
    path: Path,
}
impl PartialEq for InterpretedPending {fn eq(&self,other:&Self)->bool{self.order==other.order}}
impl Eq for InterpretedPending {}
impl PartialOrd for InterpretedPending {fn partial_cmp(&self,other:&Self)->Option<Ordering>{Some(self.cmp(other))}}
impl Ord for InterpretedPending {
    fn cmp(&self,other:&Self)->Ordering {
        suffix_order(&other.bound,&self.bound).then_with(||other.order.cmp(&self.order))
    }
}

/// 完整解释成本/偏好与字符历史共同决定后缀；稀疏转移避免历史和词条的笛卡尔展开。
fn interpreted_character_exact<O:Observer>(
    scorer:&Scorer<'_>,config:&Config,stats:&mut Statistics,k:usize,observer:&mut O,
)->Vec<Path> {
    let model=scorer.model.unwrap().character.as_ref().unwrap();
    let end=scorer.graph.letters.len();
    let max_edits=scorer.graph.max_edit_cost();
    let mut suffix=vec![BTreeMap::<(u32,u16),Option<InterpretedSuffix>>::new();end+1];
    suffix[0].insert((crate::character::BOS,max_edits),None);
    let mut labels=1;
    for edge in &scorer.graph.edges {
        for remaining in 0..=max_edits {
            if suffix[edge.end].insert((model.entry(edge.entry).last,remaining),None).is_none() {
                labels+=1;
                if !resident_allowed(stats,labels+1,config) {return Vec::new();}
            }
        }
    }
    for (&(previous,_),value) in &mut suffix[end] {
        *value=Some(InterpretedSuffix {cost:InterpretationCost::default(),preference:0,
            score:model.finish(previous),ids:Vec::new(),edges:Vec::new()});
    }
    for position in (0..end).rev() {
        for remaining in 0..=max_edits {
            let mut groups=BTreeMap::<u32,InterpretedSuffix>::new();
            for edge_index in &scorer.graph.outgoing[position] {
                if scorer.cancelled()||!expand_allowed(stats,config) {return Vec::new();}
                let edge=&scorer.graph.edges[*edge_index];
                let cost=scorer.graph.interpretation_cost(*edge_index);
                if cost.edits>remaining {continue;}
                let entry=model.entry(edge.entry);
                let Some(rest)=suffix[edge.end].get(&(entry.last,remaining-cost.edits)).and_then(Option::as_ref) else {continue;};
                let candidate=InterpretedSuffix {cost:cost.plus(rest.cost),
                    preference:scorer.lexicon.user_preference(edge.entry)+rest.preference,
                    score:entry.internal+rest.score,
                    ids:std::iter::once(edge.entry).chain(rest.ids.iter().copied()).collect(),
                    edges:std::iter::once(*edge_index).chain(rest.edges.iter().copied()).collect()};
                if groups.get(&entry.first).is_none_or(|old|suffix_order(&candidate,old)==Ordering::Less) {
                    if !resident_allowed(stats,labels+groups.len()+2,config) {return Vec::new();}
                    groups.insert(entry.first,candidate);
                }
            }
            let Some(class)=groups.values().min_by(|a,b|a.cost.cmp(&b.cost).then_with(||b.preference.cmp(&a.preference))) else {continue;};
            let (cost,preference)=(class.cost,class.preference);
            groups.retain(|_,s|s.cost==cost&&s.preference==preference);
            let mut fallback:Vec<_>=groups.iter().collect();
            fallback.sort_by(|(a_key,a),(b_key,b)|
                (b.score+model.unobserved_next(**b_key)).cmp(&(a.score+model.unobserved_next(**a_key)))
                .then_with(||a.ids.cmp(&b.ids)).then_with(||a.edges.cmp(&b.edges)));
            if !resident_allowed(stats,labels+groups.len()+fallback.len()+1,config) {return Vec::new();}
            let histories:Vec<_>=suffix[position].keys().filter(|(_,r)|*r==remaining).copied().collect();
            for (previous,remaining) in histories {
                let stamp=observer.start(Phase::Scoring);
                let rows=model.transition_row(previous);
                let mut best:Option<(&InterpretedSuffix,i64)>=None;
                for row in rows {
                    if scorer.cancelled()||!expand_allowed(stats,config) {observer.stop(Phase::Scoring,stamp);return Vec::new();}
                    if let Some(group)=groups.get(&(row.key as u32)) {
                        let score=group.score+row.units;
                        if best.is_none_or(|(old,value)|score>value||(score==value&&(group.ids.as_slice(),group.edges.as_slice())<(old.ids.as_slice(),old.edges.as_slice()))) {best=Some((group,score));}
                    }
                }
                for (next,group) in &fallback {
                    if scorer.cancelled()||!expand_allowed(stats,config) {observer.stop(Phase::Scoring,stamp);return Vec::new();}
                    if rows.binary_search_by_key(*next,|row|row.key as u32).is_err() {
                        let score=group.score+model.unobserved_bias(previous)+model.unobserved_next(**next);
                        if best.is_none_or(|(old,value)|score>value||(score==value&&(group.ids.as_slice(),group.edges.as_slice())<(old.ids.as_slice(),old.edges.as_slice()))) {best=Some((group,score));}
                        break;
                    }
                }
                let best=best.map(|(value,score)|{let mut result=value.clone();result.score=score;result});
                observer.stop(Phase::Scoring,stamp);
                suffix[position].insert((previous,remaining),best);
            }
        }
    }
    let Some(best)=suffix[0].get(&(crate::character::BOS,max_edits)).and_then(Option::as_ref) else{return Vec::new();};
    let mut pending=BinaryHeap::from([InterpretedPending {bound:best.clone(),order:0,position:0,path:Path::default()}]);
    let mut order=0;
    let mut complete=Vec::<Path>::new();
    let mut seen=vec![vec![BTreeMap::<String,InterpretedSuffix>::new();usize::from(max_edits)+1];end+1];
    seen[0][0].insert(String::new(),InterpretedSuffix::prefix(&Path::default()));
    let mut seen_count=1;
    while let Some(current)=pending.pop() {
        if scorer.cancelled(){return Vec::new();}
        let edits=current.path.features.interpretation_cost.edits;
        let signature=InterpretedSuffix::prefix(&current.path);
        if seen[current.position][usize::from(edits)].get(&current.path.text)!=Some(&signature){continue;}
        if complete.len()==k&&suffix_order(&current.bound,&InterpretedSuffix::prefix(complete.last().unwrap()))!=Ordering::Less {break;}
        if current.position==end {
            let finished=scorer.finish(current.path,observer);
            insert_unique(&mut complete,finished,k,observer);
            continue;
        }
        for edge_index in &scorer.graph.outgoing[current.position] {
            if scorer.cancelled()||!expand_allowed(stats,config){return Vec::new();}
            let edge=&scorer.graph.edges[*edge_index];
            let used=edits+scorer.graph.interpretation_cost(*edge_index).edits;
            if used>max_edits {continue;}
            let entry=model.entry(edge.entry);
            let Some(rest)=suffix[edge.end].get(&(entry.last,max_edits-used)).and_then(Option::as_ref) else{continue;};
            if !resident_allowed(stats,labels+seen_count+pending.len()+complete.len()+3,config){return Vec::new();}
            let Some(next)=scorer.extend(&current.path,*edge_index,None,observer)else{continue;};
            let bound=InterpretedSuffix::completed(&next,rest);
            if complete.len()==k&&suffix_order(&bound,&InterpretedSuffix::prefix(complete.last().unwrap()))!=Ordering::Less{continue;}
            let signature=InterpretedSuffix::prefix(&next);
            let bucket=&mut seen[edge.end][usize::from(used)];
            if bucket.get(next.text.as_str()).is_some_and(|old|suffix_order(&signature,old)!=Ordering::Less){continue;}
            if bucket.insert(next.text.clone(),signature).is_none(){seen_count+=1;}
            order+=1;
            pending.push(InterpretedPending {bound,order,position:edge.end,path:next});
        }
    }
    complete
}

