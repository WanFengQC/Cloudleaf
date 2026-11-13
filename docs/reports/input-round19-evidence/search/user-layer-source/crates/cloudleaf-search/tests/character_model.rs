use cloudleaf_search::{
    Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, search_graph,
};

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn lexicon(with_phrase: bool) -> Lexicon {
    let mut text = String::from(
        "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\tma\t甲\t\tunknown\t\tfixture\ta\tMIT\nb\tma\t乙\t\tunknown\t\tfixture\tb\tMIT\nc\tna\t丙\t\tunknown\t\tfixture\tc\tMIT\nd\tna\t𠮷\t\tunknown\t\tfixture\td\tMIT\n",
    );
    if with_phrase {
        text.push_str("z\tma na\t乙丙\t\tunknown\t\tfixture\tz\tMIT\n");
    }
    Lexicon::from_tsv(&text).unwrap()
}

fn metadata() -> String {
    format!(
        "# cloudleaf-language-v1\nlexicon_sha256\t{SHA}\nsource_sha256\t{SHA}\nsource_id\tmechanical\nlicense_id\tMIT\ntokenization\tunicode-scalar-v1\nusage\tmechanical\nkind\tcharacter_bigram\ntraining_split\ttrain\nprevious\tnext\tcount\n"
    )
}

fn sequences() -> Vec<Vec<String>> {
    let mut sequences = vec![vec!["甲".into(), "丁".into()]; 20];
    sequences.extend(vec![vec!["乙".into(), "丙".into()]; 10]);
    sequences
}

#[test]
fn complete_character_score_is_independent_of_dictionary_segmentation() {
    let mut scores = Vec::new();
    for with_phrase in [false, true] {
        let lex = lexicon(with_phrase);
        let model = LanguageModelSnapshot::train(&metadata(), &lex, SHA, &sequences()).unwrap();
        let restored = LanguageModelSnapshot::from_tsv(&model.to_tsv(), &lex, SHA).unwrap();
        assert_eq!(model.to_tsv(), restored.to_tsv());
        let config = Config {
            ranker: Ranker::CharacterBigram,
            output_k: 10,
            ..Config::default()
        };
        let graph = Lattice::build(&lex, "mana", &config).unwrap();
        let reference = search_graph(
            &lex,
            &graph,
            &Config {
                method: Method::Reference,
                ..config.clone()
            },
            Some(model.model()),
            None,
        )
        .unwrap();
        assert!(reference.guaranteed);
        assert_eq!(reference.candidates[0].text, "乙丙");
        assert_eq!(reference.candidates[0].features.character_count, 2);
        assert_eq!(
            reference.candidates[0].score,
            reference.candidates[0].features.character_units
        );
        scores.push(reference.candidates[0].score);
        for method in [Method::ExactDag, Method::Beam] {
            let actual = search_graph(
                &lex,
                &graph,
                &Config {
                    method,
                    ..config.clone()
                },
                Some(model.model()),
                None,
            )
            .unwrap();
            assert_eq!(actual.candidates, reference.candidates);
            assert!(actual.guaranteed);
        }
        assert!(
            Config {
                method: Method::PositionDag,
                ..config
            }
            .validate()
            .is_err()
        );
    }
    assert_eq!(scores[0], scores[1]);
}

#[test]
fn lower_prefix_with_a_different_character_history_survives_exact_search() {
    let lex = lexicon(false);
    let model = LanguageModelSnapshot::train(&metadata(), &lex, SHA, &sequences()).unwrap();
    for k in [1, 2, 5] {
        let cfg = Config {
            ranker: Ranker::CharacterBigram,
            output_k: k,
            ..Config::default()
        };
        for raw in ["ma", "mana", "mama", "manama", "mamanana", "ma'na", "qz"] {
            let graph = Lattice::build(&lex, raw, &cfg).unwrap();
            let reference = search_graph(
                &lex,
                &graph,
                &Config {
                    method: Method::Reference,
                    ..cfg.clone()
                },
                Some(model.model()),
                None,
            )
            .unwrap();
            let exact = search_graph(&lex, &graph, &cfg, Some(model.model()), None).unwrap();
            assert_eq!(exact.candidates, reference.candidates, "{raw} k={k}");
            assert!(exact.guaranteed);
            assert_eq!(exact.raw_fallback, raw);
            assert!(exact.candidates.iter().all(|c| c.score <= 0));
        }
    }
}

#[test]
fn character_reader_rejects_word_tokens_scope_and_disconnected_counts() {
    let lex = lexicon(false);
    let model = LanguageModelSnapshot::train(&metadata(), &lex, SHA, &sequences()).unwrap();
    let text = model.to_tsv();
    for invalid in [
        text.replace("unicode-scalar-v1", "stable-entry-ids"),
        text.replace("research_only", "production")
            .replace("usage\tmechanical", "usage\tproduction"),
        text.replace("\t甲\t", "\t甲乙\t"),
        format!("{text}戊\t戊\t1\n"),
    ] {
        assert!(LanguageModelSnapshot::from_tsv(&invalid, &lex, SHA).is_err());
    }
    let cfg = Config {
        ranker: Ranker::Bigram,
        ..Config::default()
    };
    let graph = Lattice::build(&lex, "ma", &cfg).unwrap();
    assert!(search_graph(&lex, &graph, &cfg, Some(model.model()), None).is_err());
}

#[test]
fn unknown_characters_and_resource_failure_keep_explicit_semantics() {
    let lex = lexicon(false);
    let model = LanguageModelSnapshot::train(&metadata(), &lex, SHA, &sequences()).unwrap();
    let cfg = Config {
        ranker: Ranker::CharacterBigram,
        output_k: 10,
        ..Config::default()
    };
    let graph = Lattice::build(&lex, "mana", &cfg).unwrap();
    let result = search_graph(&lex, &graph, &cfg, Some(model.model()), None).unwrap();
    let unknown = result
        .candidates
        .iter()
        .find(|c| c.text.contains('𠮷'))
        .unwrap();
    assert_eq!(unknown.features.character_unknown, 1);
    assert!(unknown.score < 0);
    let exhausted = search_graph(
        &lex,
        &graph,
        &Config {
            max_expansions: 1,
            ..cfg
        },
        Some(model.model()),
        None,
    )
    .unwrap();
    assert!(!exhausted.guaranteed);
    assert!(exhausted.statistics.budget.is_some());
    assert_eq!(exhausted.raw_fallback, "mana");
}

#[test]
fn precompiled_entry_cache_rejects_a_different_lexicon_instance() {
    let lex = lexicon(false);
    let model = LanguageModelSnapshot::train(&metadata(), &lex, SHA, &sequences()).unwrap();
    let cfg = Config {
        ranker: Ranker::CharacterBigram,
        ..Config::default()
    };
    let cloned = lex.clone();
    let graph = Lattice::build(&cloned, "ma", &cfg).unwrap();
    assert!(search_graph(&cloned, &graph, &cfg, Some(model.model()), None).is_ok());
    for other in [lexicon(false), lexicon(true)] {
        let graph = Lattice::build(&other, "mana", &cfg).unwrap();
        assert!(search_graph(&other, &graph, &cfg, Some(model.model()), None).is_err());
    }
}

#[test]
fn eos_can_reverse_the_more_likely_sentence_prefix() {
    let lex = lexicon(false);
    let mut training = vec![vec!["甲".into(), "丁".into()]; 20];
    training.extend(vec![vec!["乙".into()]; 10]);
    let model = LanguageModelSnapshot::train(&metadata(), &lex, SHA, &training).unwrap();
    let cfg = Config {
        ranker: Ranker::CharacterBigram,
        output_k: 1,
        ..Config::default()
    };
    let graph = Lattice::build(&lex, "ma", &cfg).unwrap();
    for method in [Method::Reference, Method::ExactDag, Method::Beam] {
        let result = search_graph(
            &lex,
            &graph,
            &Config {
                method,
                ..cfg.clone()
            },
            Some(model.model()),
            None,
        )
        .unwrap();
        assert_eq!(result.candidates[0].text, "乙");
        assert!(result.guaranteed);
    }
}

#[test]
fn exact_suffix_bounds_keep_top50_within_a_fixed_work_budget() {
    let lex = lexicon(false);
    let training = vec![vec!["甲".into(); 12]; 20];
    let model = LanguageModelSnapshot::train(&metadata(), &lex, SHA, &training).unwrap();
    let cfg = Config {
        ranker: Ranker::CharacterBigram,
        output_k: 50,
        context_pool: 50,
        max_expansions: 100_000,
        ..Config::default()
    };
    let graph = Lattice::build(&lex, &"ma".repeat(12), &cfg).unwrap();
    let reference = search_graph(
        &lex,
        &graph,
        &Config {
            method: Method::Reference,
            ..cfg.clone()
        },
        Some(model.model()),
        None,
    )
    .unwrap();
    assert!(reference.guaranteed);
    let bounded = search_graph(
        &lex,
        &graph,
        &Config {
            max_expansions: 600,
            ..cfg
        },
        Some(model.model()),
        None,
    )
    .unwrap();
    assert!(bounded.guaranteed, "{:?}", bounded.statistics);
    assert_eq!(bounded.candidates, reference.candidates);
}

#[test]
fn witten_bell_backoff_is_explicit_and_still_segmentation_invariant() {
    let mut scores = Vec::new();
    for with_phrase in [false, true] {
        let lex = lexicon(with_phrase);
        let header = metadata().replace(
            "kind\tcharacter_bigram\n",
            "kind\tcharacter_bigram_witten_bell\n",
        );
        let model = LanguageModelSnapshot::train(&header, &lex, SHA, &sequences()).unwrap();
        assert_eq!(model.metadata().kind, "character_bigram_witten_bell");
        let cfg = Config {
            ranker: Ranker::CharacterBigram,
            ..Config::default()
        };
        let graph = Lattice::build(&lex, "mana", &cfg).unwrap();
        let reference = search_graph(
            &lex,
            &graph,
            &Config {
                method: Method::Reference,
                ..cfg.clone()
            },
            Some(model.model()),
            None,
        )
        .unwrap();
        for method in [Method::ExactDag, Method::Beam] {
            let result = search_graph(
                &lex,
                &graph,
                &Config {
                    method,
                    ..cfg.clone()
                },
                Some(model.model()),
                None,
            )
            .unwrap();
            assert_eq!(result.candidates, reference.candidates);
        }
        assert_eq!(reference.candidates[0].text, "乙丙");
        scores.push(reference.candidates[0].score);
    }
    assert_eq!(scores[0], scores[1]);
}
