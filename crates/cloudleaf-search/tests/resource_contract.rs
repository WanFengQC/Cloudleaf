use cloudleaf_search::{Budget, Config, Lattice, Lexicon, Method, Ranker, search_graph};

const DATA: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\ta\t甲\t\tunknown\t\tfixture\ta\tMIT\nb\ta\t乙\t\tunknown\t\tfixture\tb\tMIT\nc\ta a\t丙\t\tunknown\t\tfixture\tc\tMIT\n";

#[test]
fn completed_partial_reference_paths_cover_the_entire_input() {
    let lexicon = Lexicon::from_tsv(DATA).unwrap();
    let mut saw_partial = false;
    for method in [
        Method::Reference,
        Method::ExactDag,
        Method::PositionDag,
        Method::Beam,
    ] {
        for limit in 1..20 {
            let config = Config {
                method,
                ranker: Ranker::Structural,
                max_expansions: limit,
                ..Config::default()
            };
            let graph = Lattice::build(&lexicon, "aaaa", &config).unwrap();
            let result = search_graph(&lexicon, &graph, &config, None, None).unwrap();
            assert_eq!(result.raw_fallback, "aaaa");
            assert!(result.statistics.expansions <= limit);
            if result.statistics.budget == Some(Budget::Expansions) {
                assert!(!result.guaranteed);
                if method == Method::Reference {
                    saw_partial |= !result.candidates.is_empty();
                } else {
                    assert!(result.candidates.is_empty());
                }
            }
            for candidate in result.candidates {
                let mut position = 0;
                for index in candidate.edge_indices {
                    let edge = &graph.edges()[index];
                    assert_eq!(edge.start, position);
                    position = edge.end;
                }
                assert_eq!(position, graph.letters().len());
            }
        }
    }
    assert!(saw_partial);
}

#[test]
fn graph_truncation_never_claims_complete_and_candidate_limit_is_preserved() {
    let lexicon = Lexicon::from_tsv(DATA).unwrap();
    for method in [
        Method::Reference,
        Method::ExactDag,
        Method::PositionDag,
        Method::Beam,
    ] {
        let config = Config {
            method,
            ranker: Ranker::Structural,
            max_edges: 1,
            output_k: 1,
            ..Config::default()
        };
        let graph = Lattice::build(&lexicon, "a", &config).unwrap();
        let result = search_graph(&lexicon, &graph, &config, None, None).unwrap();
        assert_eq!(result.statistics.budget, Some(Budget::GraphEdges));
        assert!(!result.guaranteed);
        assert!(result.candidates.len() <= 1);
        assert_eq!(result.raw_fallback, "a");
    }
}
