use cloudleaf_pinyin::{CorrectionConfig,DoublePinyinLayout,FuzzyDirection,FuzzyPair,FuzzyRule,PinyinScheme,SchemeConfig};
use cloudleaf_search::{Config,LanguageModelSnapshot,Lattice,Lexicon,Method,Ranker,search_graph};
use std::{collections::BTreeMap,error::Error,fs,time::Instant};

fn main()->Result<(),Box<dyn Error>> {
    let args:Vec<_>=std::env::args().skip(1).collect();
    let runtime=fs::read_to_string(&args[0])?;
    let started=Instant::now();
    let base=Lexicon::from_tsv(&runtime)?;
    let original=LanguageModelSnapshot::from_tsv(&fs::read_to_string(&args[2])?,&base,&args[1])?;
    let loaded_ns=started.elapsed().as_nanos();
    let size=args[3].parse::<isize>()?;
    let mut user_tsv="# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n".to_owned();
    for i in 0..size.max(0) {
        let a=char::from_u32(0x4e00+i as u32).unwrap();
        user_tsv.push_str(&format!("user:{}\tni hao\t测{a}\t\tunknown\t\tuser\tuser:{}\tLicenseRef-User-Private\n",i+1,i+1));
    }
    let start=Instant::now();
    let lexicon=if size<0{base.clone()}else{base.with_user_layer(&user_tsv,&BTreeMap::new())?};
    let model=if size<0{original.clone()}else{original.for_extension(&base,&lexicon)?};
    let layer_ns=start.elapsed().as_nanos();
    println!("IDENTITY\tsize={size}\tbase_entries={}\tentries={}\tbase_bytes={}\tdelta_bytes={}\tmodel_bytes={}\tload_ns={loaded_ns}\tlayer_ns={layer_ns}\tseed=0\torder=ascending-userid\tsynthetic-only=true",base.entries().len(),lexicon.entries().len(),base.memory_estimate().total_estimated_bytes,lexicon.incremental_estimated_bytes(),model.estimated_bytes());
    println!("USER_TSV\t{user_tsv:?}");
    let config=Config{method:if model.metadata().kind=="lexical"{Method::PositionDag}else{Method::ExactDag},ranker:if model.metadata().kind=="lexical"{Ranker::Lexical}else{Ranker::CharacterBigram},output_k:10,max_input_bytes:64,max_letters:64,..Default::default()};
    for iteration in 1..=3 {
        for label in ["full","abbreviated","xiaohe","fuzzy-nl","correction"] {
            let mut scheme=SchemeConfig::default();
            match label {
                "full"=>{},
                "abbreviated"=>scheme.scheme=PinyinScheme::AbbreviatedPinyin,
                "xiaohe"=>scheme.scheme=PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
                "fuzzy-nl"=>scheme.fuzzy.push(FuzzyRule{pair:FuzzyPair::NL,direction:FuzzyDirection::Both}),
                "correction"=>scheme.correction=Some(CorrectionConfig::default()),
                _=>unreachable!(),
            }
            let cases=fs::read_to_string(format!("{}/{}-cases.tsv",args[4],label))?;
            for line in cases.lines() {
                let(case,raw)=line.split_once('\t').ok_or("固定案例缺字段")?;
                let start=Instant::now();
                let graph=Lattice::build_with_scheme(&lexicon,raw,&config,&scheme)?;
                let lattice_ns=start.elapsed().as_nanos();
                let start=Instant::now();
                let result=search_graph(&lexicon,&graph,&config,Some(model.model()),None)?;
                println!("CASE\t{iteration}\t{label}\t{case}\t{raw}\tlattice_ns={lattice_ns}\tsearch_ns={}\t{result:?}",start.elapsed().as_nanos());
            }
        }
    }
    Ok(())
}
