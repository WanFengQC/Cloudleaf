use cloudleaf_search::{
    BigramModel, Budget, Config, ErrorKind, FrequencyKind, Lattice, Lexicon, LoadLimits, MatchKind,
    Method, Ranker, search_graph,
};

const HEADER: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";

fn lexicon(rows: &[(&str, &str, &str, Option<u32>)]) -> Lexicon {
    let mut tsv = HEADER.to_owned();
    for (id, pinyin, text, frequency) in rows {
        tsv.push_str(&format!(
            "{id}\t{pinyin}\t{text}\t{}\t{}\t\tcloudleaf:mechanical\t{id}\tMIT\n",
            frequency.map(|n| n.to_string()).unwrap_or_default(),
            if frequency.is_some() {
                "manual"
            } else {
                "unknown"
            }
        ));
    }
    Lexicon::from_tsv(&tsv).unwrap()
}

fn fixture() -> Lexicon {
    lexicon(&[
        ("a", "a", "甲", Some(100)),
        ("a-duplicate", "a", "甲", Some(10)),
        ("an", "an", "乙", None),
        ("na", "na", "丙", Some(10)),
        ("nan", "nan", "丁", Some(8)),
        ("ana", "a na", "甲丙", Some(5)),
        ("xi", "xi", "西", Some(10)),
        ("an-alt", "an", "安", Some(9)),
        ("xian", "xian", "先", None),
        ("xi-an", "xi an", "西安", Some(15)),
        ("lv", "lv", "绿", None),
        ("nv", "nv", "女", None),
        ("full", "xi an na", "符", None),
    ])
}

#[test]
fn nullable_frequency_and_legacy_manual_are_preserved() {
    let lex = fixture();
    let entry = lex.entries().iter().find(|e| e.id == "an").unwrap();
    assert_eq!(entry.frequency, None);
    assert_eq!(entry.frequency_kind, FrequencyKind::Unknown);
    let legacy = Lexicon::from_tsv("# cloudleaf-dictionary-v1\nid\tpinyin\ttext\tfrequency\tannotation\tsource\nid\ta\t甲\t0\t\tcloudleaf:legacy\n").unwrap();
    assert_eq!(legacy.entries()[0].frequency, Some(0));
    assert_eq!(legacy.entries()[0].frequency_kind, FrequencyKind::Manual);
    let cfg = Config::default();
    let graph = Lattice::build(&lex, "lv", &cfg).unwrap();
    let result = search_graph(&lex, &graph, &cfg, None, None).unwrap();
    assert_eq!(result.candidates[0].features.frequency_units, 0);
    assert_eq!(result.candidates[0].features.missing_frequencies, 1);
}

#[test]
fn rejects_malformed_records_without_echoing_them() {
    for row in [
        "id\ta\t甲\t0\tunknown\t\tsource\tid\tMIT",
        "id\ta\t甲\t\tmanual\t\tsource\tid\tMIT",
        "id\ta  na\t甲\t\tunknown\t\tsource\tid\tMIT",
        "id\tbad\t甲\t\tunknown\t\tsource\tid\tMIT",
        "id\ta\t 甲\t\tunknown\t\tsource\tid\tMIT",
        "id\ta\t甲\t\tunknown\t\tsource\t\tMIT",
    ] {
        let error = Lexicon::from_tsv(&format!("{HEADER}{row}\n")).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidLexicon);
        assert_eq!(error.line, Some(3));
        assert!(!error.to_string().contains('甲'));
    }
    assert!(Lexicon::from_tsv(&format!("\u{feff}{HEADER}")).is_err());
    assert!(Lexicon::from_tsv(&HEADER.replace('\n', "\r\n")).is_err());
    let repeated = "id\ta\t甲\t\tunknown\t\tsource\tid\tMIT\n";
    assert_eq!(
        Lexicon::from_tsv(&format!("{HEADER}{repeated}{repeated}"))
            .unwrap_err()
            .line,
        Some(4)
    );
}

#[test]
fn explicit_boundaries_and_utf8_spans_cover_every_raw_byte() {
    let lex = fixture();
    let cfg = Config {
        output_k: 20,
        ..Config::default()
    };
    let graph = Lattice::build(&lex, "Xİ", &cfg);
    assert!(graph.is_err());
    let graph = Lattice::build(&lex, "XI'AN", &cfg).unwrap();
    assert_eq!(graph.letters(), "xian");
    assert_eq!(graph.raw_offsets(), &[0, 1, 2, 4, 5]);
    assert!(
        !graph
            .edges()
            .iter()
            .any(|edge| lex.entries()[edge.entry].id == "xian")
    );
    let result = search_graph(&lex, &graph, &cfg, None, None).unwrap();
    assert!(result.candidates.iter().any(|c| c.text == "西安"));
    for candidate in result.candidates {
        let mut previous = 0;
        for index in candidate.edge_indices {
            let edge = &graph.edges()[index];
            assert_eq!(edge.raw_start, previous);
            previous = edge.raw_end;
        }
        assert_eq!(previous, graph.raw().len());
    }
    let graph = Lattice::build(&lex, "LÜ'NV", &cfg).unwrap();
    assert_eq!(graph.raw_offsets(), &[0, 1, 3, 5, 6]);
    assert_eq!(graph.letters(), "lvnv");
    let candidate = &search_graph(&lex, &graph, &cfg, None, None)
        .unwrap()
        .candidates[0];
    assert_eq!(candidate.text, "绿女");
    let spans: Vec<_> = candidate
        .edge_indices
        .iter()
        .map(|i| (graph.edges()[*i].raw_start, graph.edges()[*i].raw_end))
        .collect();
    assert_eq!(spans, vec![(0, 3), (3, 6)]);
}

#[test]
fn prefix_cannot_invent_future_syllables_and_oov_never_disappears() {
    let lex = fixture();
    let cfg = Config::default();
    let graph = Lattice::build(&lex, "x", &cfg).unwrap();
    assert!(
        !graph
            .edges()
            .iter()
            .any(|e| lex.entries()[e.entry].id == "full")
    );
    assert!(
        graph
            .edges()
            .iter()
            .any(|e| lex.entries()[e.entry].id == "xian" && e.kind == MatchKind::Prefix)
    );
    for raw in ["xianq", "q", "", "xi'"] {
        let graph = Lattice::build(&lex, raw, &cfg).unwrap();
        let result = search_graph(&lex, &graph, &cfg, None, None).unwrap();
        assert!(result.candidates.is_empty(), "{raw}");
        assert_eq!(result.raw_fallback, raw);
        assert_eq!(result.waiting, raw.ends_with('\''));
    }
    for raw in ["'a", "a''na", "a b", "a\0", "中文"] {
        assert!(Lattice::build(&lex, raw, &cfg).is_err());
    }
}

#[test]
fn lattice_is_bound_to_original_lexicon() {
    let lex = fixture();
    let other = fixture();
    let cfg = Config::default();
    let graph = Lattice::build(&lex, "a", &cfg).unwrap();
    assert_eq!(
        search_graph(&other, &graph, &cfg, None, None)
            .unwrap_err()
            .kind,
        ErrorKind::InvalidInput
    );
}

#[test]
fn no_inherited_thirty_two_parse_limit() {
    let lex = fixture();
    let cfg = Config {
        output_k: 50,
        context_pool: 50,
        ..Config::default()
    };
    let raw = "xian".repeat(6);
    let graph = Lattice::build(&lex, &raw, &cfg).unwrap();
    let result = search_graph(&lex, &graph, &cfg, None, None).unwrap();
    assert!(result.guaranteed);
    assert_eq!(result.candidates.len(), 50);
    for candidate in result.candidates {
        assert_eq!(
            graph.edges()[*candidate.edge_indices.last().unwrap()].raw_end,
            raw.len()
        );
    }
}

#[test]
fn all_reference_paths_match_exact_kbest_with_text_duplicates_and_fixed_seed_inputs() {
    let lex = fixture();
    let model = BigramModel::train(&[
        vec!["a".into(), "na".into()],
        vec!["xi".into(), "an".into()],
        vec!["an-alt".into(), "a".into()],
    ])
    .unwrap();
    let mut inputs = vec![
        "anana".to_string(),
        "xian".into(),
        "xi'an".into(),
        "ananan".into(),
        "xiana".into(),
    ];
    // 固定种子生成机械序列，不用真实语言测试集构造权重。
    let mut seed = 0x51a7_u64;
    for _ in 0..100 {
        let mut raw = String::new();
        for _ in 0..3 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            raw.push_str(["a", "an", "na", "nan", "xi", "xian"][(seed >> 32) as usize % 6]);
        }
        inputs.push(raw);
    }
    for raw in inputs {
        for ranker in [
            Ranker::Structural,
            Ranker::Weighted,
            Ranker::Bigram,
            Ranker::ContextRerank,
        ] {
            for k in [1, 3, 10] {
                let mut cfg = Config {
                    output_k: k,
                    context_pool: k.max(15),
                    ranker,
                    ..Config::default()
                };
                let graph = Lattice::build(&lex, &raw, &cfg).unwrap();
                cfg.method = Method::Reference;
                let reference = search_graph(&lex, &graph, &cfg, Some(&model), Some("a")).unwrap();
                cfg.method = Method::ExactDag;
                let exact = search_graph(&lex, &graph, &cfg, Some(&model), Some("a")).unwrap();
                assert!(reference.guaranteed && exact.guaranteed, "{raw}");
                assert_eq!(
                    reference.candidates, exact.candidates,
                    "{raw}, {ranker:?}, {k}"
                );
                cfg.method = Method::Beam;
                cfg.beam_width = 4096;
                let beam = search_graph(&lex, &graph, &cfg, Some(&model), Some("a")).unwrap();
                assert!(beam.guaranteed);
                assert_eq!(beam.candidates, exact.candidates);
            }
        }
    }
}

#[test]
fn budgets_are_explicit_and_resident_high_water_includes_active_and_clone() {
    let lex = fixture();
    for method in [Method::Reference, Method::ExactDag, Method::Beam] {
        for budget in [
            Budget::GraphEdges,
            Budget::Expansions,
            Budget::ResidentPaths,
        ] {
            let mut cfg = Config {
                method,
                ..Config::default()
            };
            match budget {
                Budget::GraphEdges => cfg.max_edges = 1,
                Budget::Expansions => cfg.max_expansions = 1,
                Budget::ResidentPaths => cfg.max_resident_paths = 1,
            }
            let graph = Lattice::build(&lex, "anana", &cfg).unwrap();
            let result = search_graph(&lex, &graph, &cfg, None, None).unwrap();
            assert!(!result.guaranteed);
            assert_eq!(result.statistics.budget, Some(budget));
            assert!(result.statistics.resident_high_water <= cfg.max_resident_paths);
            assert!(result.statistics.expansions <= cfg.max_expansions);
            assert_eq!(result.raw_fallback, "anana");
        }
    }
    let one = lexicon(&[("a", "a", "甲", None)]);
    for method in [Method::Reference, Method::ExactDag] {
        let mut cfg = Config {
            method,
            max_resident_paths: 1,
            ..Config::default()
        };
        let graph = Lattice::build(&one, "a", &cfg).unwrap();
        assert_eq!(
            search_graph(&one, &graph, &cfg, None, None)
                .unwrap()
                .statistics
                .budget,
            Some(Budget::ResidentPaths)
        );
        cfg.max_resident_paths = 2;
        let result = search_graph(&one, &graph, &cfg, None, None).unwrap();
        assert!(result.guaranteed);
        assert_eq!(result.statistics.resident_high_water, 2);
    }
}

#[test]
fn bigram_future_can_defeat_narrow_beam_and_context_is_only_finite_pool() {
    let lex = lexicon(&[
        ("high", "a", "甲", None),
        ("low", "a", "乙", None),
        ("next", "na", "丙", None),
        ("other", "guo", "丁", None),
        ("context", "zhong", "戊", None),
    ]);
    let mut train = vec![vec!["high".into(), "other".into()]; 60];
    train.extend(vec![
        vec!["context".into(), "high".into(), "other".into()];
        1000
    ]);
    train.extend(vec![
        vec!["context".into(), "low".into(), "next".into()];
        80
    ]);
    let model = BigramModel::train(&train).unwrap();
    let mut cfg = Config {
        ranker: Ranker::Bigram,
        output_k: 1,
        beam_width: 1,
        ..Config::default()
    };
    let graph = Lattice::build(&lex, "ana", &cfg).unwrap();
    let exact = search_graph(&lex, &graph, &cfg, Some(&model), None).unwrap();
    assert_eq!(exact.candidates[0].text, "乙丙");
    cfg.method = Method::Beam;
    let beam = search_graph(&lex, &graph, &cfg, Some(&model), None).unwrap();
    assert_eq!(beam.candidates[0].text, "甲丙");
    assert!(beam.statistics.beam_pruned > 0);
    assert!(!beam.guaranteed);
    cfg.method = Method::ExactDag;
    cfg.output_k = 2;
    let mut context_train = vec![vec!["high".into()]; 20];
    context_train.extend(vec![vec!["context".into(), "low".into()]; 100]);
    let context_model = BigramModel::train(&context_train).unwrap();
    let graph = Lattice::build(&lex, "a", &cfg).unwrap();
    let baseline = search_graph(&lex, &graph, &cfg, Some(&context_model), None).unwrap();
    assert_eq!(baseline.candidates[0].text, "甲");
    cfg.ranker = Ranker::ContextRerank;
    let disabled = search_graph(&lex, &graph, &cfg, Some(&context_model), None).unwrap();
    assert_eq!(baseline.candidates, disabled.candidates);
    let contextual =
        search_graph(&lex, &graph, &cfg, Some(&context_model), Some("context")).unwrap();
    assert_eq!(contextual.candidates[0].text, "乙");
    assert!(contextual.finite_pool_only);
    assert!(
        contextual
            .candidates
            .iter()
            .any(|c| c.features.context_units != 0)
    );
    assert!(search_graph(&lex, &graph, &cfg, None, None).is_err());
}

#[test]
fn load_limits_are_explicit_and_v2_can_exceed_legacy_entry_limit() {
    let mut text = HEADER.to_owned();
    // 重复机械读音只用于验证装载容量，绝不登记为正式词库。
    for index in 0..100_001 {
        text.push_str(&format!(
            "id{index}\ta\t甲\t\tunknown\t\tcloudleaf:capacity\tid{index}\tMIT\n"
        ));
    }
    let lexicon = Lexicon::from_tsv(&text).unwrap();
    assert_eq!(lexicon.entries().len(), 100_001);
    assert_eq!(lexicon.trie_nodes(), 2);
    for limits in [
        LoadLimits {
            max_entries: 100_000,
            ..LoadLimits::default()
        },
        LoadLimits {
            max_bytes: 8,
            ..LoadLimits::default()
        },
        LoadLimits {
            max_trie_nodes: 1,
            ..LoadLimits::default()
        },
    ] {
        assert!(Lexicon::from_tsv_with_limits(&text, &limits).is_err());
    }
    assert_eq!(
        Lexicon::from_tsv_with_limits(
            HEADER,
            &LoadLimits {
                max_entries: 0,
                ..LoadLimits::default()
            }
        )
        .unwrap_err()
        .kind,
        ErrorKind::InvalidConfig
    );
}

#[test]
fn invalid_limits_and_models_fail_before_search() {
    assert!(
        Config {
            output_k: 0,
            ..Config::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        Config {
            context_pool: 101,
            ..Config::default()
        }
        .validate()
        .is_err()
    );
    assert!(BigramModel::train(&[vec!["<bos>".into()]]).is_err());
    assert!(BigramModel::train(&[vec!["has space".into()]]).is_err());
    let lex = fixture();
    let cfg = Config {
        max_letters: 3,
        ..Config::default()
    };
    assert!(Lattice::build(&lex, "xian", &cfg).is_err());
}
