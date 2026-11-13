use cloudleaf_search::{
    Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, search_graph,
};

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEADER: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";

fn lexicon() -> Lexicon {
    Lexicon::from_tsv(&format!("{HEADER}a\tma\t甲\t\tunknown\t\tfixture\ta\tMIT\nb\tma\t乙\t\tunknown\t\tfixture\tb\tMIT\nc\tna\t丙\t\tunknown\t\tfixture\tc\tMIT\n")).unwrap()
}

fn metadata(kind: &str) -> String {
    format!(
        "# cloudleaf-language-v1\nlexicon_sha256\t{SHA}\nsource_sha256\t{SHA}\nsource_id\tcloudleaf:mechanical\nlicense_id\tMIT\ntokenization\tstable-entry-ids\nusage\tmechanical\nkind\t{kind}\ntraining_split\ttrain\nprevious\tnext\tcount\n"
    )
}

#[test]
fn lexical_is_independent_tiebreak_and_unknown_dictionary_stays_unknown() {
    let lex = lexicon();
    let text = format!(
        "{}<lexical>\ta\t1\n<lexical>\tb\t1023\n",
        metadata("lexical")
    );
    let snapshot = LanguageModelSnapshot::from_tsv(&text, &lex, SHA).unwrap();
    assert_eq!(snapshot.to_tsv(), text);
    for method in [
        Method::Reference,
        Method::ExactDag,
        Method::PositionDag,
        Method::Beam,
    ] {
        let config = Config {
            method,
            ranker: Ranker::Lexical,
            ..Config::default()
        };
        let graph = Lattice::build(&lex, "ma", &config).unwrap();
        let result = search_graph(&lex, &graph, &config, Some(snapshot.model()), None).unwrap();
        assert_eq!(result.candidates[0].text, "乙");
        assert_eq!(result.candidates[0].score, -100);
        assert_eq!(result.candidates[0].features.lexical_units, 10240);
        assert_eq!(result.candidates[0].features.bigram_units, 0);
        assert!(lex.entries().iter().all(|entry| entry.frequency.is_none()));
    }
}

#[test]
fn fixed_model_rejects_wrong_version_scope_counts_and_entry_reference() {
    let lex = lexicon();
    let text = format!("{}<lexical>\ta\t1\n", metadata("lexical"));
    for changed in [
        text.replace(SHA, &"b".repeat(64)),
        text.replace("mechanical\nkind", "production\nkind"),
        text.replace("\ta\t1", "\tmissing\t1"),
        text.replace("\ta\t1", "\ta\t0"),
        text.replace("\ta\t1", "\ta\t18446744073709551615"),
        text.replace("train\n", "heldout\n"),
        format!("{text}<lexical>\ta\t1\n"),
    ] {
        assert!(LanguageModelSnapshot::from_tsv(&changed, &lex, SHA).is_err());
    }
}

#[test]
fn offline_training_roundtrips_real_bigram_counts_and_boundaries() {
    let lex = lexicon();
    let metadata_text = metadata("bigram");
    let sequences = vec![
        vec!["a".to_owned(), "c".to_owned()],
        vec!["b".to_owned(), "c".to_owned()],
    ];
    let model = LanguageModelSnapshot::train(&metadata_text, &lex, SHA, &sequences).unwrap();
    let restored = LanguageModelSnapshot::from_tsv(&model.to_tsv(), &lex, SHA).unwrap();
    assert_eq!(restored.to_tsv(), model.to_tsv());
    assert!(
        restored.model().transition(Some("a"), Some("c"))
            > restored.model().transition(Some("a"), Some("b"))
    );
    assert!(
        restored.model().transition(Some("c"), None)
            > restored.model().transition(Some("c"), Some("a"))
    );
    let cfg = Config {
        ranker: Ranker::Bigram,
        output_k: 5,
        ..Config::default()
    };
    let graph = Lattice::build(&lex, "mana", &cfg).unwrap();
    let exact = search_graph(&lex, &graph, &cfg, Some(restored.model()), None).unwrap();
    let reference = search_graph(
        &lex,
        &graph,
        &Config {
            method: Method::Reference,
            ..cfg
        },
        Some(restored.model()),
        None,
    )
    .unwrap();
    assert_eq!(exact.candidates, reference.candidates);
}

#[test]
fn training_header_cannot_inject_unobserved_sequences() {
    let lex = lexicon();
    let header = format!("{}<bos>\tb\t100\nb\t<eos>\t100\n", metadata("bigram"));
    assert!(LanguageModelSnapshot::train(&header, &lex, SHA, &[vec!["a".into()]]).is_err());
}

#[test]
fn reader_rejects_balanced_counts_disconnected_from_sentence_boundaries() {
    let lex = lexicon();
    let artifact = format!(
        "{}<bos>\ta\t1\na\t<eos>\t1\nb\tb\t100\n",
        metadata("bigram")
    );
    assert!(LanguageModelSnapshot::from_tsv(&artifact, &lex, SHA).is_err());
}

#[test]
fn lexical_multiedge_paths_match_full_reference_and_keep_structure_priority() {
    let lex = lexicon();
    let text = format!(
        "{}<lexical>\ta\t1\n<lexical>\tb\t1023\n<lexical>\tc\t10\n",
        metadata("lexical")
    );
    let snapshot = LanguageModelSnapshot::from_tsv(&text, &lex, SHA).unwrap();
    for raw in ["mana", "mama", "manama", "mamanama", "ma'na", "ma'"] {
        for k in [1, 2, 5] {
            let config = Config {
                method: Method::Reference,
                ranker: Ranker::Lexical,
                output_k: k,
                ..Config::default()
            };
            let graph = Lattice::build(&lex, raw, &config).unwrap();
            let reference =
                search_graph(&lex, &graph, &config, Some(snapshot.model()), None).unwrap();
            for method in [Method::ExactDag, Method::PositionDag, Method::Beam] {
                let result = search_graph(
                    &lex,
                    &graph,
                    &Config {
                        method,
                        ..config.clone()
                    },
                    Some(snapshot.model()),
                    None,
                )
                .unwrap();
                assert_eq!(result.candidates, reference.candidates);
                assert_eq!(result.guaranteed, reference.guaranteed);
            }
        }
    }
}
