use cloudleaf_search::{Config,Lattice,Lexicon,MatchKind,Method,Ranker,search_graph};
use std::collections::BTreeMap;

fn quote(s:&str)->String {format!("{:?}",s)}

fn main(){
    let path=std::env::args().nth(1).unwrap();
    let text=std::fs::read_to_string(path).unwrap();
    let lex=Lexicon::from_tsv(&text).unwrap();
    println!("IDENTITY\tentries={}\tunknown={}\ttrie={}",lex.entries().len(),lex.entries().iter().filter(|e|e.frequency.is_none()).count(),lex.trie_nodes());
    for (input,expected) in [("nihao","你好"),("zhongguo","中国"),("woaini","我爱你"),("jintiantianqihenhao","今天天气很好"),("wozhengzaikaifayikuanzhongwenshurufa","我正在开发一款中文输入法"),("xian","西安"),("xi'an","西安"),("qz","qz")]{
        let mut cfg=Config{method:Method::PositionDag,ranker:Ranker::Structural,output_k:50,context_pool:50,..Config::default()};
        let graph=Lattice::build(&lex,input,&cfg).unwrap();
        let result=search_graph(&lex,&graph,&cfg,None,None).unwrap();
        println!("CASE\t{input}\texpected={}\tbudget={:?}\tguaranteed={}\texpansions={}\tedges={}",quote(expected),result.statistics.budget,result.guaranteed,result.statistics.expansions,graph.edges().len());
        for (rank,c) in result.candidates.iter().enumerate(){
            println!("CAND\t{input}\t{}\t{}\t{}\t{:?}\t{:?}",rank+1,quote(&c.text),c.score,c.features,c.entry_ids);
        }
        // 仅诊断预期正文是否由实际格中的连通路径构成，不用于生成或重排候选。
        let mut states:Vec<BTreeMap<usize,(i64,Vec<usize>,u64)>>=vec![BTreeMap::new();graph.letters().len()+1];
        states[0].insert(0,(0,Vec::new(),1));
        for position in 0..graph.letters().len(){
            let current=std::mem::take(&mut states[position]);
            for (offset,(score,path,count)) in current {
                for (ei,edge) in graph.edges().iter().enumerate().filter(|(_,e)|e.start==position){
                    let entry=&lex.entries()[edge.entry];
                    if expected[offset..].starts_with(&entry.text){
                        let end=offset+entry.text.len();
                        let next_score=score-100-if edge.kind==MatchKind::Prefix{300}else{0};
                        let mut next_path=path.clone();next_path.push(ei);
                        let slot=states[edge.end].entry(end).or_insert((i64::MIN,Vec::new(),0));
                        slot.2=slot.2.saturating_add(count);
                        if next_score>slot.0 {slot.0=next_score;slot.1=next_path;}
                    }
                }
            }
        }
        if let Some((score,path,count))=states.last().unwrap().get(&expected.len()){
            println!("TARGET\t{input}\tcovered=true\tstructural_score={score}\tpaths={count}\tfirst50={}",result.candidates.iter().position(|c|c.text==expected).map(|n|(n+1).to_string()).unwrap_or("null".into()));
            for ei in path{let e=&graph.edges()[*ei];let entry=&lex.entries()[e.entry];println!("TARGET_EDGE\t{input}\t{}\t{}\t{}\t{}\t{:?}\t{:?}",e.start,e.end,quote(&entry.text),entry.id,entry.frequency,e.kind);}
        }else{println!("TARGET\t{input}\tcovered=false");}
        cfg.ranker=Ranker::Weighted;
        let weighted=search_graph(&lex,&graph,&cfg,None,None).unwrap();
        println!("ABLATION\t{input}\tStructural_eq_Weighted={}",result.candidates==weighted.candidates);
    }
}
