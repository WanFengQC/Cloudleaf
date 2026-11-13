use cloudleaf_pinyin::{PinyinScheme, SchemeConfig};
use cloudleaf_search::{
    Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, search_graph,
};
use std::{error::Error, fs, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let runtime = fs::read_to_string(&args[0])?;
    let lexicon = Lexicon::from_tsv(&runtime)?;
    let model =
        LanguageModelSnapshot::from_tsv(&fs::read_to_string(&args[2])?, &lexicon, &args[1])?;
    let scheme = SchemeConfig {
        scheme: if args.get(4).is_some_and(|v|v=="full"){PinyinScheme::FullPinyin}else{PinyinScheme::AbbreviatedPinyin},
        ..Default::default()
    };
    let config = Config {
        ranker: if model.metadata().kind=="lexical"{Ranker::Lexical}else{Ranker::CharacterBigram},
        method: if model.metadata().kind=="lexical"{Method::PositionDag}else{Method::ExactDag},
        output_k: 10,
        max_input_bytes: 64,
        max_letters: 64,
        ..Default::default()
    };
    for line in fs::read_to_string(&args[3])?.lines() {
        let (case, raw) = line.split_once('\t').unwrap();
        let now = Instant::now();
        let graph = Lattice::build_with_scheme(&lexicon, raw, &config, &scheme)?;
        let lattice_ns = now.elapsed().as_nanos();
        let now = Instant::now();
        let result = search_graph(&lexicon, &graph, &config, Some(model.model()), None)?;
        println!(
            "CASE\t{case}\t{raw}\tlattice_ns={lattice_ns}\tsearch_ns={}\t{result:?}",
            now.elapsed().as_nanos()
        );
    }
    Ok(())
}
