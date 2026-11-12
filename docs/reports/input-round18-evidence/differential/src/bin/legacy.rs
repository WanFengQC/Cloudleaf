use cloudleaf_search::{Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, search_graph};
use std::collections::BTreeSet;

const SHA:&str="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn metadata(kind:&str)->String {format!("# cloudleaf-language-v1\nlexicon_sha256\t{SHA}\nsource_sha256\t{SHA}\nsource_id\tcloudleaf:mechanical-r16-diff\nlicense_id\tMIT\ntokenization\tunicode-scalar-v1\nusage\tmechanical\nkind\t{kind}\ntraining_split\ttrain\nprevious\tnext\tcount\n")}

fn lexicon(variant:usize)->Lexicon {
    let mut text=String::from("# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n");
    let mut rows=vec![("01","a","甲"),("02","a","乙"),("03","na","丙"),("04","na","丁"),("05","an","甲丙"),("06","a na","甲丙"),("07","a","甲乙"),("08","na","甲"),("09","an","𠮷甲"),("10","a na","乙丙"),("11","a","𠮷"),("12","na","𬺰"),("13","a","乙"),("14","an a","甲丙甲"),("15","xi","西"),("16","an","安"),("17","xian","先"),("18","xi an","西安"),("19","xian","西安"),("20","xi","😀")];
    if variant==1 {rows.retain(|r|!matches!(r.0,"06"|"10"|"14"|"18"|"19"));}
    if variant==2 {rows.extend([("21","a na a","乙丙甲"),("22","a","<"),("23","na","😀"),("24","xian a","西安甲")]);}
    for (id,pinyin,body) in rows {text.push_str(&format!("{id}\t{pinyin}\t{body}\t\tunknown\t\tfixture\t{id}\tMIT\n"));}
    Lexicon::from_tsv(&text).unwrap()
}

fn sequences(scheme:usize)->Vec<Vec<String>> {
    let base=["甲丁","乙丙","甲乙甲","丙甲","西安","先甲","丁丙乙","乙","甲","𠮷甲","😀丙","<甲"];
    let mut output=Vec::new();
    for (i,text) in base.iter().enumerate(){
        let count=if scheme==0{1}else{(scheme*7+i*11)%23};
        for _ in 0..count {output.push(text.chars().map(|c|c.to_string()).collect());}
    }
    if scheme==3 {output=vec![vec!["甲".into()];9];}
    if scheme==7 {output=vec![vec!["𠮷".into(),"😀".into()];7];}
    output
}

fn main(){
    let mut total_failed=0;
    for kind in ["character_bigram","character_bigram_witten_bell"] {
    println!("KIND\t{kind}");
    let mut inputs:BTreeSet<String>=["a","an","ana","anana","ananan","anaana","a'na","a'na'a","xi","xian","xi'an","xiana","xianana","xianxian","xi'ana","a'","qz","","m","x"].iter().map(|s|s.to_string()).collect();
    let mut seed=0x16b17a_u64;
    for _ in 0..50 {
        let mut raw=String::new();
        for _ in 0..3 {seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);raw.push_str(["a","an","na","xi","xian"][(seed>>32) as usize%5]);}
        inputs.insert(raw);
    }
    let mut queries=0;let mut comparisons=0;let mut failures=0;let mut incomplete=0;let mut pruned=0;let mut position_rejections=0;
    for variant in 0..3 {
        let lex=lexicon(variant);
        for scheme in 0..12 {
            let training=sequences(scheme);
            println!("MODEL\tvariant={variant}\tscheme={scheme}\tsequences={training:?}");
            let model=LanguageModelSnapshot::train(&metadata(kind),&lex,SHA,&training).unwrap();
            let restored=LanguageModelSnapshot::from_tsv(&model.to_tsv(),&lex,SHA).unwrap();
            if restored.to_tsv()!=model.to_tsv(){failures+=1;println!("FAIL\troundtrip\tvariant={variant}\tscheme={scheme}");}
            for raw in &inputs {
                for k in [1,2,5,10,20] {
                    queries+=1;
                    let cfg=Config{ranker:Ranker::CharacterBigram,output_k:k,context_pool:k.max(20),max_expansions:2_000_000,max_resident_paths:200_000,..Config::default()};
                    if (Config{method:Method::PositionDag,..cfg.clone()}).validate().is_err(){position_rejections+=1;}else{failures+=1;println!("FAIL\tposition-accepted");}
                    let graph=Lattice::build(&lex,raw,&cfg).unwrap();
                    let reference=search_graph(&lex,&graph,&Config{method:Method::Reference,..cfg.clone()},Some(model.model()),None).unwrap();
                    println!("REFERENCE\tvariant={variant}\tscheme={scheme}\traw={raw:?}\tk={k}\tguaranteed={}\tstatistics={:?}\tcandidates={:?}",reference.guaranteed,reference.statistics,reference.candidates);
                    if !reference.guaranteed{incomplete+=1;continue;}
                    for method in [Method::ExactDag,Method::Beam]{
                        let result=search_graph(&lex,&graph,&Config{method,beam_width:4096,..cfg.clone()},Some(model.model()),None).unwrap();
                        comparisons+=1;pruned+=usize::from(result.statistics.beam_pruned>0);
                        let same=result.candidates==reference.candidates;
                        if !same || !result.guaranteed || result.raw_fallback!=*raw {failures+=1;println!("FAIL\tvariant={variant}\tscheme={scheme}\traw={raw:?}\tk={k}\tmethod={method:?}\tresult={result:?}");}
                        println!("COMPARE\tvariant={variant}\tscheme={scheme}\traw={raw:?}\tk={k}\tmethod={method:?}\tsame={same}\tguaranteed={}\tstatistics={:?}",result.guaranteed,result.statistics);
                    }
                }
            }
        }
    }
    let lex=lexicon(0);
    let mut eos_training=vec![vec!["甲".into(),"丁".into()];20];eos_training.extend(vec![vec!["乙".into()];10]);
    let model=LanguageModelSnapshot::train(&metadata(kind),&lex,SHA,&eos_training).unwrap();
    let cfg=Config{ranker:Ranker::CharacterBigram,output_k:10,..Config::default()};
    let graph=Lattice::build(&lex,"a",&cfg).unwrap();
    let result=search_graph(&lex,&graph,&cfg,Some(model.model()),None).unwrap();
    println!("EOS\t{result:?}");
    // 两种平滑都必须在完整路径加入EOS后逆转BOS对甲的偏好。
    if result.candidates[0].text!="乙"{failures+=1;println!("FAIL\tEOS did not reverse BOS preference");}
    let unknown=result.candidates.iter().find(|c|c.text=="𠮷").unwrap();
    if unknown.features.character_unknown!=1||unknown.features.character_count!=1{failures+=1;println!("FAIL\tUnicode scalar count\t{unknown:?}");}
    println!("SUMMARY\tkind={kind}\tqueries={queries}\tcomparisons={comparisons}\tfailures={failures}\tincomplete_reference={incomplete}\tbeam_pruned_cases={pruned}\tposition_rejections={position_rejections}\tinputs={}",inputs.len());
    total_failed+=failures+incomplete+pruned;
    }
    if total_failed>0 {std::process::exit(1);}
}
