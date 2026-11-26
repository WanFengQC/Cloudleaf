use cloudleaf_search::{
    Config, Lattice, Lexicon, Method, Ranker, ResearchRanker, WordContextModel,
    expected_reachability, search_graph,
};

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEADER: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";

fn lexicon() -> Lexicon {
    Lexicon::from_tsv(&format!("{HEADER}a\tma\t甲\t\tunknown\t\tfixture\ta\tMIT\nb\tma\t乙\t\tunknown\t\tfixture\tb\tMIT\nc\tla\t丙\t\tunknown\t\tfixture\tc\tMIT\nd\tna\t丁\t\tunknown\t\tfixture\td\tMIT\n")).unwrap()
}

fn text() -> String {
    format!(
        "# cloudleaf-word-context-v1\nlexicon_sha256\t{SHA}\ncorpus_sha256\t{SHA}\nW\t</s>\t16\nW\t<s>\t16\nW\t丁\t8\nW\t丙\t8\nW\t乙\t8\nW\t甲\t8\nB\t<s>\t乙\t8\nB\t<s>\t甲\t8\nB\t丁\t</s>\t8\nB\t丙\t</s>\t8\nB\t乙\t丁\t8\nB\t甲\t丙\t8\n"
    )
}

#[test]
fn actual_lattice_retains_both_surfaces_and_word_context_changes_order() {
    let lex = lexicon();
    let config = Config {
        ranker: Ranker::Structural,
        method: Method::PositionDag,
        ..Config::default()
    };
    let graph = Lattice::build(&lex, "mana", &config).unwrap();
    let pool = search_graph(&lex, &graph, &config, None, None).unwrap();
    assert!(pool.guaranteed);
    let model = WordContextModel::from_tsv(&text(), &lex, SHA).unwrap();
    let words = model
        .rank_pool(&graph, &pool, ResearchRanker::Word, None)
        .unwrap();
    let lexical = model
        .rank_pool(&graph, &pool, ResearchRanker::Unigram, None)
        .unwrap();
    let full: Vec<_> = words
        .iter()
        .map(|row| pool.candidates[row.pool_index].text.as_str())
        .collect();
    assert_eq!(full, ["乙丁", "甲丁"]);
    assert_eq!(pool.candidates[lexical[0].pool_index].text, "甲丁");
    assert_eq!(
        words,
        model
            .reference_pool(&graph, &pool, ResearchRanker::Word, None)
            .unwrap()
    );
    assert_eq!(
        lexical,
        model
            .reference_pool(&graph, &pool, ResearchRanker::Unigram, None)
            .unwrap()
    );
    assert_eq!(expected_reachability(&graph, "甲丁", 200_000), Some(true));
    assert_eq!(expected_reachability(&graph, "乙丙", 200_000), Some(false));
    assert_eq!(expected_reachability(&graph, "乙丁", 1), None);
    assert_eq!(model.observed_count("你"), None);
    assert!(lex.entries().iter().all(|entry| entry.frequency.is_none()));
}

#[test]
fn strict_schema_rejects_corruption_and_untrusted_resources() {
    let lex = lexicon();
    let model = WordContextModel::from_tsv(&text(), &lex, SHA).unwrap();
    for invalid in [
        text().replace("W\t乙\t8", "W\t乙\t7"),
        text().replace("W\t丙", "W\t无"),
        text().replace("W\t丙\t8\nW\t乙\t8", "W\t乙\t8\nW\t丙\t8"),
        format!("{}B\t甲\t丙\t8\n", text()),
        text().replace("B\t<s>\t甲", "B\t</s>\t甲"),
        format!(
            "# cloudleaf-word-context-v1\nlexicon_sha256\t{SHA}\ncorpus_sha256\t{SHA}\nW\t</s>\t1\nW\t<s>\t1\nW\t乙\t2\nW\t甲\t1\nB\t<s>\t甲\t1\nB\t乙\t乙\t2\nB\t甲\t</s>\t1\n"
        ),
        text().replace("\n", "\r\n"),
        text().replace(SHA, &"b".repeat(64)),
    ] {
        assert!(WordContextModel::from_tsv(&invalid, &lex, SHA).is_err());
    }
    let lex2 = lexicon();
    let config = Config::default();
    let graph = Lattice::build(&lex2, "mana", &config).unwrap();
    let pool = search_graph(&lex2, &graph, &config, None, None).unwrap();
    assert!(
        model
            .rank_pool(&graph, &pool, ResearchRanker::Word, None)
            .is_err()
    );
}

#[test]
fn indexed_and_literal_reference_match_every_bounded_candidate() {
    let lex = lexicon();
    let model = WordContextModel::from_tsv(&text(), &lex, SHA).unwrap();
    let mut compared = 0;
    for n in 1..=5 {
        for bits in 0..1 << n {
            let raw: String = (0..n)
                .map(|index| if bits & (1 << index) == 0 { "ma" } else { "na" })
                .collect();
            for k in [1, 3, 10, 50] {
                let config = Config {
                    ranker: Ranker::Structural,
                    method: Method::PositionDag,
                    output_k: k,
                    context_pool: k.max(20),
                    ..Config::default()
                };
                let graph = Lattice::build(&lex, &raw, &config).unwrap();
                let pool = search_graph(&lex, &graph, &config, None, None).unwrap();
                let reference = search_graph(
                    &lex,
                    &graph,
                    &Config {
                        method: Method::Reference,
                        ..config
                    },
                    None,
                    None,
                )
                .unwrap();
                assert_eq!(pool.candidates, reference.candidates);
                for mode in [ResearchRanker::Unigram, ResearchRanker::Word] {
                    let result = model.rank_pool(&graph, &pool, mode, None).unwrap();
                    let literal = model
                        .reference_pool(&graph, &reference, mode, None)
                        .unwrap();
                    assert_eq!(result, literal);
                    compared += literal.len();
                }
            }
        }
    }
    assert!(compared > 1_000);
    println!("完整候选重排对照数：{compared}");
}
