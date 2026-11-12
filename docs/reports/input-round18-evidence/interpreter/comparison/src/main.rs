pub use cloudleaf_types::{CoreError, ErrorCode};
pub use cloudleaf_pinyin::{is_syllable,legal_syllables};
#[allow(dead_code)]
#[path="../../pre-viability-scheme.rs"]
mod old;
use cloudleaf_pinyin as new;
use std::collections::BTreeSet;
#[derive(Clone,Debug,Eq,PartialEq)]
struct Arc { start:usize,end:usize,edits:u16,description:String }
fn paths(len:usize,arcs:Vec<Arc>)->(BTreeSet<String>,usize) {
    let mut pending=vec![(0,0,Vec::<String>::new())];let mut complete=BTreeSet::new();let mut expanded=0;
    while let Some((position,edits,path))=pending.pop() {
        expanded+=1;assert!(expanded<=200000,"reference enumeration budget exceeded");
        if position==len {complete.insert(format!("{path:?}"));continue;}
        for arc in arcs.iter().filter(|a|a.start==position&&edits+a.edits<=1) {
            let mut next=path.clone();next.push(arc.description.clone());pending.push((arc.end,edits+arc.edits,next));
        }
    }
    (complete,expanded)
}
fn main() {
    let mut inputs=BTreeSet::new();let alphabet=b"ahnixz";
    for a in alphabet { inputs.insert(String::from_utf8(vec![*a]).unwrap());
        for b in alphabet {inputs.insert(String::from_utf8(vec![*a,*b]).unwrap());
            for c in alphabet {inputs.insert(String::from_utf8(vec![*a,*b,*c]).unwrap());}
        }
    }
    inputs.extend(["zhnog","zhnogguo","nihso","nihoa","zhng","zongg","zhpng","ni'hoa"].map(str::to_owned));
    let mut total_paths=0;let mut runs=0;let mut failures=0;let mut incomplete=0;
    for input in inputs {
        let a=old::interpret(&input,&old::SchemeConfig{correction:Some(old::CorrectionConfig::default()),..old::SchemeConfig::default()}).unwrap();
        let b=new::interpret(&input,&new::SchemeConfig{correction:Some(new::CorrectionConfig::default()),..new::SchemeConfig::default()}).unwrap();
        if a.budget.is_some()||b.budget.is_some(){println!("INCOMPLETE\t{input}\told={:?}\tnew={:?}",a.budget,b.budget);incomplete+=1;continue;}
        let aa=a.arcs.iter().map(|arc|Arc{start:arc.start,end:arc.end,edits:arc.changes.iter().filter(|c|c.kind!=old::ChangeKind::Fuzzy).map(|c|c.cost).sum(),description:format!("{arc:?}")}).collect();
        let bb=b.arcs.iter().map(|arc|Arc{start:arc.start,end:arc.end,edits:arc.changes.iter().filter(|c|c.kind!=new::ChangeKind::Fuzzy).map(|c|c.cost).sum(),description:format!("{arc:?}")}).collect();
        let (ap,ae)=paths(a.keys.len(),aa);let(bp,be)=paths(b.keys.len(),bb);
        let same=ap==bp;runs+=1;failures+=usize::from(!same);total_paths+=ap.len();
        println!("COMPARE\t{input}\tpaths={}\told_arcs={}\tnew_arcs={}\told_states={ae}\tnew_states={be}\tequal={same}\told_work={}\tnew_work={}",ap.len(),a.arcs.len(),b.arcs.len(),a.work,b.work);
        if !same{println!("DIFF\t{input}\told={ap:?}\tnew={bp:?}");}
    }
    let input="jintiantiqnqihenhao";
    let a=old::interpret(input,&old::SchemeConfig{correction:Some(old::CorrectionConfig::default()),..old::SchemeConfig::default()}).unwrap();
    let b=new::interpret(input,&new::SchemeConfig{correction:Some(new::CorrectionConfig::default()),..new::SchemeConfig::default()}).unwrap();
    println!("RESOURCE\t{input}\told_budget={:?}\tnew_budget={:?}\told_work={}\tnew_work={}\told_arcs={}\tnew_arcs={}",a.budget,b.budget,a.work,b.work,a.arcs.len(),b.arcs.len());
    println!("SUMMARY\truns={runs}\tcomplete_paths={total_paths}\tfailed={failures}\tincomplete={incomplete}");
    assert_eq!(failures,0);assert_eq!(incomplete,0);assert!(a.budget.is_some());assert!(b.budget.is_none());
}
