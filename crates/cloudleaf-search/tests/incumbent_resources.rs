use cloudleaf_pinyin::{PinyinScheme, SchemeConfig};
use cloudleaf_search::{
    Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, search_graph,
};

#[test]
fn actual_complete_suffix_candidates_bound_resident_prefixes_without_losing_sources() {
    let mut data = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n".to_owned();
    let mut sequences = Vec::new();
    for i in 0..24 {
        let text = char::from_u32(0x4e00 + i).unwrap().to_string();
        data.push_str(&format!(
            "n{i:02}\tni\t{text}\t\tunknown\t\tmechanical\tn{i:02}\tMIT\n"
        ));
        sequences.push(vec![text.clone(), text.clone(), text]);
    }
    let lexicon = Lexicon::from_tsv(&data).unwrap();
    let sha = "7".repeat(64);
    let scheme = SchemeConfig {
        scheme: PinyinScheme::AbbreviatedPinyin,
        ..Default::default()
    };
    for kind in ["character_bigram", "character_bigram_witten_bell"] {
        let metadata = format!(
            "# cloudleaf-language-v1\nlexicon_sha256\t{sha}\nsource_sha256\t{sha}\nsource_id\tcloudleaf:mechanical-incumbent\nlicense_id\tMIT\ntokenization\tunicode-scalar-v1\nusage\tmechanical\nkind\t{kind}\ntraining_split\ttrain\nprevious\tnext\tcount\n"
        );
        let model = LanguageModelSnapshot::train(&metadata, &lexicon, &sha, &sequences).unwrap();
        let config = Config {
            ranker: Ranker::CharacterBigram,
            output_k: 10,
            max_expansions: 200000,
            max_resident_paths: 256,
            ..Default::default()
        };
        let graph = Lattice::build_with_scheme(&lexicon, "nnn", &config, &scheme).unwrap();
        let exact = search_graph(&lexicon, &graph, &config, Some(model.model()), None).unwrap();
        assert!(exact.guaranteed, "{kind}: {:?}", exact.statistics);
        assert_eq!(exact.candidates.len(), 10);
        let reference = search_graph(
            &lexicon,
            &graph,
            &Config {
                method: Method::Reference,
                ..config
            },
            Some(model.model()),
            None,
        )
        .unwrap();
        assert!(reference.guaranteed, "{kind}: {:?}", reference.statistics);
        assert_eq!(reference.candidates, exact.candidates);
    }
}
