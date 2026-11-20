use cloudleaf_pinyin::{PinyinScheme,SchemeConfig};
use cloudleaf_search::{Lexicon,LanguageModelSnapshot,Config,Ranker,Method,Lattice,search_graph};
fn main(){
    for width in [4,8,12,24] {
        let mut data="# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n".to_owned();
        let mut sequences=Vec::new();
        for i in 0..width {let c=char::from_u32(0x4e00+i).unwrap().to_string();data.push_str(&format!("n{i:02}\tni\t{c}\t\tunknown\t\tmechanical\tn{i:02}\tMIT\n"));sequences.push(vec![c.clone(),c.clone(),c]);}
        let lex=Lexicon::from_tsv(&data).unwrap();let sha="7".repeat(64);
        let scheme=SchemeConfig{scheme:PinyinScheme::AbbreviatedPinyin,..Default::default()};
        for kind in ["character_bigram","character_bigram_witten_bell"] {
            let metadata=format!("# cloudleaf-language-v1\nlexicon_sha256\t{sha}\nsource_sha256\t{sha}\nsource_id\tcloudleaf:mechanical-resource\nlicense_id\tMIT\ntokenization\tunicode-scalar-v1\nusage\tmechanical\nkind\t{kind}\ntraining_split\ttrain\nprevious\tnext\tcount\n");
            let model=LanguageModelSnapshot::train(&metadata,&lex,&sha,&sequences).unwrap();
            for length in [2,3,4,8,16,32] {
                let raw="n".repeat(length);
                let config=Config{ranker:Ranker::CharacterBigram,output_k:10,max_expansions:200000,max_resident_paths:50000,..Default::default()};
                let graph=Lattice::build_with_scheme(&lex,&raw,&config,&scheme).unwrap();
                let result=search_graph(&lex,&graph,&config,Some(model.model()),None).unwrap();
                println!("CASE\t{width}\t{kind}\t{length}\t{:?}\t{:?}",result.statistics,result.candidates.iter().map(|c|&c.text).collect::<Vec<_>>());
                if length<=4 && width<=8 {
                    let reference=search_graph(&lex,&graph,&Config{method:Method::Reference,..config},Some(model.model()),None).unwrap();
                    assert!(reference.guaranteed);assert_eq!(reference.candidates,result.candidates);
                }
            }
        }
    }
}
