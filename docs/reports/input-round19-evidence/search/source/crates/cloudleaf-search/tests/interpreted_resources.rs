use cloudleaf_pinyin::{PinyinScheme, SchemeConfig};
use cloudleaf_search::{
    Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, search_graph,
};

fn fixture(kind: &str) -> (Lexicon, String, Vec<Vec<String>>) {
    let mut tsv="# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n".to_owned();
    let mut sequences = Vec::new();
    for i in 0..24 {
        let a = char::from_u32(0x4e00 + i).unwrap().to_string();
        let b = char::from_u32(0x4f00 + i).unwrap().to_string();
        tsv.push_str(&format!(
            "n{i:02}\tni\t{a}\t\tunknown\t\tmechanical\tn{i:02}\tMIT\n"
        ));
        tsv.push_str(&format!(
            "h{i:02}\thao\t{b}\t\tunknown\t\tmechanical\th{i:02}\tMIT\n"
        ));
        sequences.push(vec![a, b]);
    }
    let sha = "7".repeat(64);
    let metadata = format!(
        "# cloudleaf-language-v1\nlexicon_sha256\t{sha}\nsource_sha256\t{sha}\nsource_id\tcloudleaf:mechanical-resource\nlicense_id\tMIT\ntokenization\tunicode-scalar-v1\nusage\tmechanical\nkind\t{kind}\ntraining_split\ttrain\nprevious\tnext\tcount\n"
    );
    (Lexicon::from_tsv(&tsv).unwrap(), metadata, sequences)
}

#[test]
fn interpreted_character_topk_avoids_repeating_every_history_edge_pair() {
    let scheme = SchemeConfig {
        scheme: PinyinScheme::AbbreviatedPinyin,
        ..Default::default()
    };
    for kind in ["character_bigram", "character_bigram_witten_bell"] {
        let (lexicon, metadata, sequences) = fixture(kind);
        let model =
            LanguageModelSnapshot::train(&metadata, &lexicon, &"7".repeat(64), &sequences).unwrap();
        let config = Config {
            ranker: Ranker::CharacterBigram,
            output_k: 10,
            max_expansions: 900,
            ..Default::default()
        };
        let graph = Lattice::build_with_scheme(&lexicon, "nihao", &config, &scheme).unwrap();
        let result = search_graph(&lexicon, &graph, &config, Some(model.model()), None).unwrap();
        assert!(result.guaranteed, "{kind}: {:?}", result.statistics);
        assert_eq!(result.candidates.len(), 10);
        // 完整参考对照另用两算法完全相同的预算，不能拿截断Reference当正确答案。
        let complete = Config {
            max_expansions: 10000,
            ..config
        };
        let reference = search_graph(
            &lexicon,
            &graph,
            &Config {
                method: Method::Reference,
                ..complete.clone()
            },
            Some(model.model()),
            None,
        )
        .unwrap();
        let exact = search_graph(&lexicon, &graph, &complete, Some(model.model()), None).unwrap();
        assert!(reference.guaranteed && exact.guaranteed);
        assert_eq!(reference.candidates, exact.candidates);
        assert_eq!(reference.candidates, result.candidates);
    }
}
