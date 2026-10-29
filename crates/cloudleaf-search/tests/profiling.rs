use cloudleaf_search::{
    BigramModel, Budget, Config, Lattice, Lexicon, Method, Phase, Ranker, profile_query,
    search_graph,
};

fn lexicon() -> Lexicon {
    Lexicon::from_tsv("# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\ta\t甲\t\tunknown\t\tfixture\t1\tMIT\nb\ta\t乙\t\tunknown\t\tfixture\t2\tMIT\n").unwrap()
}

#[test]
fn profiled_query_preserves_all_results_and_incomplete_budgets() {
    let lexicon = lexicon();
    for method in [
        Method::Reference,
        Method::ExactDag,
        Method::PositionDag,
        Method::Beam,
    ] {
        for input in ["", "a'", "a", "aaa", "ni,hao"] {
            for (edges, expansions, resident) in [
                (16384, 200000, 50000),
                (1, 200000, 50000),
                (16384, 1, 50000),
                (16384, 200000, 1),
            ] {
                let config = Config {
                    method,
                    ranker: Ranker::Structural,
                    max_edges: edges,
                    max_expansions: expansions,
                    max_resident_paths: resident,
                    beam_width: 1,
                    ..Config::default()
                };
                let old = Lattice::build(&lexicon, input, &config)
                    .and_then(|graph| search_graph(&lexicon, &graph, &config, None, None));
                let (new, profile) = profile_query(&lexicon, input, &config, None, None);
                assert_eq!(new.as_ref().map(|(_, result)| result), old.as_ref());
                assert_eq!(profile.phase(Phase::QueryTotal).unwrap().calls, 1);
                assert_eq!(profile.phase(Phase::Normalize).unwrap().calls, 1);
                if old.is_err() {
                    assert!(profile.phase(Phase::Lattice).is_none());
                    assert!(profile.phase(Phase::Traversal).is_none());
                } else {
                    assert_eq!(profile.phase(Phase::Lattice).unwrap().calls, 1);
                    assert_eq!(profile.phase(Phase::Traversal).unwrap().calls, 1);
                    assert_eq!(profile.phase(Phase::FinalSortDedup).unwrap().calls, 1);
                    assert_eq!(profile.phase(Phase::Materialize).unwrap().calls, 1);
                }
                assert_eq!(
                    profile.clock_reads(),
                    profile
                        .measurements()
                        .map(|(_, measurement)| measurement.calls * 2)
                        .sum::<u64>()
                );
            }
        }
    }
}

#[test]
fn nested_sort_is_observed_even_on_reference_budget_failure_and_beam_pruning() {
    let lexicon = lexicon();
    let config = Config {
        method: Method::Reference,
        ranker: Ranker::Structural,
        max_expansions: 10,
        ..Config::default()
    };
    let (result, profile) = profile_query(&lexicon, "aaa", &config, None, None);
    assert_eq!(
        result.unwrap().1.statistics.budget,
        Some(Budget::Expansions)
    );
    assert!(profile.phase(Phase::BucketSortDedup).unwrap().calls > 0);
    let config = Config {
        method: Method::Beam,
        ranker: Ranker::Structural,
        beam_width: 1,
        ..Config::default()
    };
    let (result, profile) = profile_query(&lexicon, "aaa", &config, None, None);
    assert!(result.unwrap().1.statistics.beam_pruned > 0);
    assert!(profile.phase(Phase::BucketSortDedup).unwrap().calls > 0);
}

#[test]
fn configuration_and_model_failures_retain_the_actual_attempted_phase() {
    let lexicon = lexicon();
    let config = Config {
        ranker: Ranker::Bigram,
        ..Config::default()
    };
    let (result, profile) = profile_query(&lexicon, "a", &config, None, None);
    assert_eq!(
        result.unwrap_err().kind,
        cloudleaf_search::ErrorKind::InvalidModel
    );
    assert_eq!(profile.phase(Phase::SearchValidate).unwrap().calls, 1);
    assert!(profile.phase(Phase::Traversal).is_none());
    let config = Config {
        max_edges: 0,
        ..Config::default()
    };
    let (result, profile) = profile_query(&lexicon, "a", &config, None, None);
    assert_eq!(
        result.unwrap_err().kind,
        cloudleaf_search::ErrorKind::InvalidConfig
    );
    assert!(profile.phase(Phase::Normalize).is_some());
    assert!(profile.phase(Phase::Lattice).is_none());
}

#[test]
fn scoring_is_an_explicit_nested_observation_without_changing_complete_results() {
    let lexicon = lexicon();
    for method in [
        Method::Reference,
        Method::ExactDag,
        Method::PositionDag,
        Method::Beam,
    ] {
        let config = Config {
            method,
            ranker: Ranker::Structural,
            ..Config::default()
        };
        let graph = Lattice::build(&lexicon, "aaa", &config).unwrap();
        let plain = search_graph(&lexicon, &graph, &config, None, None).unwrap();
        let (profiled, profile) = profile_query(&lexicon, "aaa", &config, None, None);
        assert_eq!(profiled.unwrap().1, plain);
        assert!(
            profile
                .measurements()
                .any(|(phase, value)| phase.name() == "scoring" && value.calls > 0)
        );
    }
}

#[test]
fn scoring_calls_distinguish_extension_and_eos_and_all_rankers_keep_results() {
    let lexicon = lexicon();
    let model = BigramModel::train(&[vec!["a".into(), "b".into()], vec!["b".into()]]).unwrap();
    for method in [
        Method::Reference,
        Method::ExactDag,
        Method::PositionDag,
        Method::Beam,
    ] {
        for ranker in [
            Ranker::Structural,
            Ranker::Weighted,
            Ranker::Bigram,
            Ranker::ContextRerank,
        ] {
            if method == Method::PositionDag
                && matches!(ranker, Ranker::Bigram | Ranker::ContextRerank)
            {
                continue;
            }
            let config = Config {
                method,
                ranker,
                ..Config::default()
            };
            let graph = Lattice::build(&lexicon, "a", &config).unwrap();
            let plain = search_graph(&lexicon, &graph, &config, Some(&model), Some("a")).unwrap();
            let (observed, profile) =
                profile_query(&lexicon, "a", &config, Some(&model), Some("a"));
            assert_eq!(observed.unwrap().1, plain);
            let eos = if method == Method::PositionDag { 0 } else { 2 };
            assert_eq!(profile.phase(Phase::Scoring).unwrap().calls, 2 + eos);
        }
    }
    for input in ["", "a'", "invalid,"] {
        let (_, profile) = profile_query(&lexicon, input, &Config::default(), None, None);
        assert!(profile.phase(Phase::Scoring).is_none());
    }
}
