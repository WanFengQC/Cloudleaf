use cloudleaf_pinyin::{PinyinScheme, SchemeConfig};
use cloudleaf_search::{Config, Lattice, Lexicon, Method, Ranker, search_graph};

fn dictionary() -> Lexicon {
    let mut tsv = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n".to_owned();
    for (id, pinyin, text) in [
        ("ni", "ni", "你"),
        ("hao", "hao", "好"),
        ("nihao", "ni hao", "你好"),
        ("nonghu", "nong hu", "农户"),
        ("zhong", "zhong", "中"),
        ("guoren", "guo ren", "国人"),
        ("zhongguoren", "zhong guo ren", "中国人"),
        ("beijing", "bei jing", "北京"),
        ("daxue", "da xue", "大学"),
        ("beida", "bei jing da xue", "北京大学"),
        ("xian", "xian", "先"),
        ("xi-an", "xi an", "西安"),
        ("xing", "xing", "行"),
    ] {
        tsv.push_str(&format!(
            "{id}\t{pinyin}\t{text}\t\tunknown\t\tmechanical\t{id}\tMIT\n"
        ));
    }
    Lexicon::from_tsv(&tsv).unwrap()
}

#[test]
fn mixed_abbreviations_keep_complete_sentence_paths_and_raw_offsets() {
    let lex = dictionary();
    let scheme = SchemeConfig {
        scheme: PinyinScheme::AbbreviatedPinyin,
        ..Default::default()
    };
    let config = Config {
        ranker: Ranker::Structural,
        output_k: 10,
        ..Default::default()
    };
    for (raw, expected) in [
        ("zgr", "中国人"),
        ("zhonggr", "中国人"),
        ("bjdx", "北京大学"),
        ("nh", "你好"),
        ("nihao", "你好"),
    ] {
        let graph = Lattice::build_with_scheme(&lex, raw, &config, &scheme).unwrap();
        let result = search_graph(&lex, &graph, &config, None, None).unwrap();
        assert!(result.guaranteed, "{raw}: {:?}", result.statistics);
        assert!(
            result.candidates.iter().any(|c| c.text == expected),
            "{raw}: {result:?}"
        );
        assert_eq!(result.raw_fallback, raw);
        for edge in graph.edges() {
            assert!(edge.raw_start < edge.raw_end && edge.raw_end <= raw.len());
        }
    }
}

#[test]
fn default_full_graph_and_all_candidate_objects_remain_identical() {
    let lex = dictionary();
    for raw in [
        "nihao",
        "NIHAO",
        "zhongguoren",
        "xi'an",
        "xian",
        "ni'",
        "qz",
    ] {
        for method in [
            Method::Reference,
            Method::ExactDag,
            Method::PositionDag,
            Method::Beam,
        ] {
            let config = Config {
                method,
                ranker: Ranker::Structural,
                ..Default::default()
            };
            let old = Lattice::build(&lex, raw, &config).unwrap();
            let new =
                Lattice::build_with_scheme(&lex, raw, &config, &SchemeConfig::default()).unwrap();
            assert_eq!(old.edges(), new.edges());
            assert_eq!(old.raw_offsets(), new.raw_offsets());
            assert_eq!(
                search_graph(&lex, &old, &config, None, None).unwrap(),
                search_graph(&lex, &new, &config, None, None).unwrap()
            );
        }
    }
}
