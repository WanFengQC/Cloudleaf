use cloudleaf_search::{Config,LanguageModelSnapshot,Lattice,Lexicon,MatchKind,Method,Ranker,search_graph};

fn target_paths(graph:&Lattice<'_>,lex:&Lexicon,target:&str,position:usize,offset:usize,path:&mut Vec<usize>,output:&mut Vec<Vec<usize>>){
    if position==graph.letters().len(){if offset==target.len(){output.push(path.clone());}return;}
    for (index,edge) in graph.edges().iter().enumerate().filter(|(_,e)|e.start==position){
        let entry=&lex.entries()[edge.entry];
        if target[offset..].starts_with(&entry.text){
            path.push(index);target_paths(graph,lex,target,edge.end,offset+entry.text.len(),path,output);path.pop();
        }
    }
}
fn main(){
    let args:Vec<String>=std::env::args().collect();
    let lex=Lexicon::from_tsv(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let artifact=LanguageModelSnapshot::from_tsv(&std::fs::read_to_string(&args[2]).unwrap(),&lex,"03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431").unwrap();
    println!("IDENTITY\tentries={}\tunknown={}\tmodel={:?}",lex.entries().len(),lex.entries().iter().filter(|e|e.frequency.is_none()).count(),artifact.metadata());
    for (input,targets) in [("woaini",vec!["我爱你","我爱昵"]),("zheshiyigebijiaofuzadewenti",vec!["这是一个比较复杂的问题","这时一个比较复杂的问题"]) ]{
        for ranker in [Ranker::Structural,Ranker::Lexical]{
            let cfg=Config{method:Method::PositionDag,ranker,output_k:50,context_pool:50,..Config::default()};
            let graph=Lattice::build(&lex,input,&cfg).unwrap();
            let result=search_graph(&lex,&graph,&cfg,Some(artifact.model()),None).unwrap();
            println!("CASE\t{input}\t{ranker:?}\tbudget={:?}\tguaranteed={}\tbeam_pruned={}\texpansions={}\tresident={}\tedges={}",result.statistics.budget,result.guaranteed,result.statistics.beam_pruned,result.statistics.expansions,result.statistics.resident_high_water,graph.edges().len());
            for (rank,c) in result.candidates.iter().take(10).enumerate(){println!("CAND\t{input}\t{ranker:?}\t{}\t{:?}\t{}\t{:?}\t{:?}",rank+1,c.text,c.score,c.features,c.entry_ids);}
            for target in &targets{
                let mut paths=Vec::new();target_paths(&graph,&lex,target,0,0,&mut Vec::new(),&mut paths);
                println!("TARGET\t{input}\t{ranker:?}\t{target:?}\tpaths={}\tfirst50={:?}",paths.len(),result.candidates.iter().position(|c|c.text==*target).map(|n|n+1));
                for (number,path) in paths.iter().enumerate(){
                    let score:i64=path.iter().map(|i|-100-if graph.edges()[*i].kind==MatchKind::Prefix{300}else{0}).sum();
                    let lexical:i64=path.iter().filter_map(|i|artifact.model().lexical_units(&lex.entries()[graph.edges()[*i].entry].id)).sum();
                    println!("PATH\t{input}\t{ranker:?}\t{target:?}\t{number}\tstructural={score}\tlexical={lexical}");
                    for index in path{let edge=&graph.edges()[*index];let entry=&lex.entries()[edge.entry];println!("EDGE\t{input}\t{number}\t{}\t{}\t{:?}\t{}\t{:?}\tlexical={:?}",edge.start,edge.end,entry.text,entry.id,edge.kind,artifact.model().lexical_units(&entry.id));}
                }
            }
        }
    }
}
