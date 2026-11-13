use cloudleaf_search::{BigramModel, Config, Lattice, Lexicon, Method, Ranker, search_graph};

#[test]
fn many_dominated_homophones_keep_reference_top_k_and_best_duplicate_path() {
    let mut text = String::from(
        "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n",
    );
    for i in 0..24 {
        text.push_str(&format!(
            "word-{i:02}\ta\t候选{}\t{}\tmanual\t\tmechanical\trecord-{i}\tMIT\n",
            i / 2,
            i * 100
        ));
    }
    let lexicon = Lexicon::from_tsv(&text).unwrap();
    let model = BigramModel::train(&[
        vec!["word-03".into(), "word-04".into(), "word-01".into()],
        vec!["word-04".into(), "word-01".into()],
    ])
    .unwrap();
    for k in [1, 5, 10] {
        for ranker in [
            Ranker::Structural,
            Ranker::Weighted,
            Ranker::Bigram,
            Ranker::ContextRerank,
        ] {
            let config = Config {
                method: Method::Reference,
                ranker,
                output_k: k,
                ..Config::default()
            };
            let graph = Lattice::build(&lexicon, "aaa", &config).unwrap();
            let reference =
                search_graph(&lexicon, &graph, &config, Some(&model), Some("word-04")).unwrap();
            assert!(reference.guaranteed);
            for method in [Method::ExactDag, Method::PositionDag] {
                if method == Method::PositionDag
                    && matches!(ranker, Ranker::Bigram | Ranker::ContextRerank)
                {
                    continue;
                }
                let config = Config {
                    method,
                    ..config.clone()
                };
                let result =
                    search_graph(&lexicon, &graph, &config, Some(&model), Some("word-04")).unwrap();
                assert!(result.guaranteed);
                assert_eq!(result.candidates, reference.candidates);
            }
        }
    }
}
