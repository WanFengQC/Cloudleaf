use cloudleaf_search::{
    Budget, Cancellation, Config, ErrorKind, LanguageModelSnapshot, Lattice, Lexicon,
    WordContextModel, recall_context, recall_interleaved, reference_recall_context,
};

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn fixture() -> (Lexicon, String, String, String) {
    let lexicon = Lexicon::from_tsv(concat!(
        "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n",
        "a\tma\t甲\t\tunknown\t\tfixture\ta\tMIT\n",
        "b\tma\t乙\t\tunknown\t\tfixture\tb\tMIT\n",
        "c\tna\t丙\t\tunknown\t\tfixture\tc\tMIT\n",
        "d\tna\t丁\t\tunknown\t\tfixture\td\tMIT\n",
        "p\tma na\t甲丙\t\tunknown\t\tfixture\tp\tMIT\n",
        "q\tma na\t乙丙\t\tunknown\t\tfixture\tq\tMIT\n",
    )).unwrap();
    let words = format!(
        concat!(
            "# cloudleaf-word-context-v1\nlexicon_sha256\t{0}\ncorpus_sha256\t{0}\n",
            "W\t</s>\t16\nW\t<s>\t16\nW\t丁\t8\nW\t丙\t8\nW\t乙\t8\nW\t甲\t8\n",
            "B\t<s>\t乙\t8\nB\t<s>\t甲\t8\nB\t丁\t</s>\t8\nB\t丙\t</s>\t8\nB\t乙\t丁\t8\nB\t甲\t丙\t8\n"
        ),
        SHA
    );
    let metadata = format!(
        concat!(
            "# cloudleaf-language-v1\nlexicon_sha256\t{0}\nsource_sha256\t{0}\nsource_id\tmechanical\nlicense_id\tMIT\n",
            "tokenization\tunicode-scalar-v1\nusage\tmechanical\nkind\t",
        ),
        SHA
    );
    let columns = "\ntraining_split\ttrain\nprevious\tnext\tcount\n";
    let characters = format!("{metadata}character_bigram{columns}");
    let lexical = format!("{metadata}lexical{columns}<lexical>\ta\t1\n<lexical>\tb\t32\n");
    (lexicon, words, characters, lexical)
}

#[test]
fn full_paths_match_reference_and_choose_the_higher_scoring_segmentation() {
    let (lex, word_tsv, metadata, _) = fixture();
    let words = WordContextModel::from_tsv(&word_tsv, &lex, SHA).unwrap();
    let corpus = vec![vec!["甲".to_owned(), "丙".to_owned()]; 12];
    let characters = LanguageModelSnapshot::train(&metadata, &lex, SHA, &corpus).unwrap();
    let cancel = Cancellation::default();
    let mut compared = 0;
    let mut inputs = vec!["qz".to_owned(), "ma'".to_owned(), "ma'na".to_owned()];
    for count in 1..=5 {
        for mask in 0..(1 << count) {
            inputs.push(
                (0..count)
                    .map(|bit| if mask & (1 << bit) == 0 { "ma" } else { "na" })
                    .collect(),
            );
        }
    }
    for raw in &inputs {
        for output_k in [1, 3, 10, 50] {
            let config = Config {
                output_k,
                context_pool: output_k.max(20),
                beam_width: 4096,
                ..Config::default()
            };
            let graph = Lattice::build(&lex, raw, &config).unwrap();
            let old =
                reference_recall_context(&graph, &config, characters.model(), &words, &cancel)
                    .unwrap();
            let new = recall_context(&graph, &config, characters.model(), &words, &cancel).unwrap();
            assert!(old.exhaustive_for_score && new.exhaustive_for_score);
            assert_eq!(new.candidates, old.candidates, "{raw} K={output_k}");
            assert_eq!(&new.raw_fallback, raw);
            compared += new.candidates.len();
            if raw == "mana" {
                let path = &new.candidates[0];
                assert_eq!(path.path.text, "甲丙");
                assert_eq!(path.path.entry_ids, ["a", "c"]);
                assert_eq!(
                    path.path.score,
                    path.path.features.character_units + path.word_gain
                );
                assert!(path.word_gain > 0);
            }
        }
    }
    assert!(compared > 1_000);
    println!("完整召回候选及得分对照：{compared}");
}

#[test]
fn quotas_pruning_and_cancellation_are_reported_and_keep_raw_input() {
    let (lex, word_tsv, metadata, lexical_tsv) = fixture();
    let words = WordContextModel::from_tsv(&word_tsv, &lex, SHA).unwrap();
    let corpus = vec![vec!["甲".to_owned(), "丙".to_owned()]; 12];
    let characters = LanguageModelSnapshot::train(&metadata, &lex, SHA, &corpus).unwrap();
    let lexical = LanguageModelSnapshot::from_tsv(&lexical_tsv, &lex, SHA).unwrap();
    let cancel = Cancellation::default();
    let config = Config {
        beam_width: 1,
        output_k: 10,
        ..Config::default()
    };
    let graph = Lattice::build(&lex, "ma'namana", &config).unwrap();
    let reference =
        reference_recall_context(&graph, &config, characters.model(), &words, &cancel).unwrap();
    let beam = recall_context(&graph, &config, characters.model(), &words, &cancel).unwrap();
    assert!(reference.exhaustive_for_score);
    assert!(!beam.exhaustive_for_score);
    assert!(beam.work.beam_pruned > 0);
    assert!(beam.candidates.len() < reference.candidates.len());
    assert_eq!(beam.raw_fallback, "ma'namana");
    let normal = recall_interleaved(
        &graph,
        &config,
        characters.model(),
        lexical.model(),
        &cancel,
    )
    .unwrap();
    assert!(!normal.exhaustive_for_score);
    assert!(
        normal
            .candidates
            .iter()
            .any(|item| item.source_ranks.iter().flatten().count() > 1)
    );
    assert_eq!(
        normal.candidates.len(),
        normal
            .candidates
            .iter()
            .map(|item| &item.path.text)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    );

    for (max_expansions, max_resident_paths, budget) in [
        (5, 30, Budget::Expansions),
        (200000, 1, Budget::ResidentPaths),
    ] {
        let small = Config {
            max_expansions,
            max_resident_paths,
            ..config.clone()
        };
        let merged =
            recall_interleaved(&graph, &small, characters.model(), lexical.model(), &cancel)
                .unwrap();
        assert!(merged.work.expansions <= max_expansions);
        assert!(merged.work.resident_high_water <= max_resident_paths);
        assert_eq!(merged.work.budget, Some(budget));
        assert_eq!(
            merged
                .source_work
                .iter()
                .map(|source| source.expansions)
                .sum::<usize>(),
            merged.work.expansions
        );
        let scored = recall_context(&graph, &small, characters.model(), &words, &cancel).unwrap();
        assert_eq!(scored.work.budget, Some(budget));
        assert_eq!(scored.raw_fallback, "ma'namana");
        assert!(!scored.exhaustive_for_score);
    }
    cancel.cancel();
    assert_eq!(
        recall_context(&graph, &config, characters.model(), &words, &cancel)
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    assert_eq!(
        recall_interleaved(
            &graph,
            &config,
            characters.model(),
            lexical.model(),
            &cancel
        )
        .unwrap_err()
        .kind,
        ErrorKind::Cancelled
    );
}
