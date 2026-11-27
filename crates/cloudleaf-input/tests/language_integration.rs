use cloudleaf_input::{InputConfig, InputEngine, LanguageSnapshot, SearchSnapshot};
use cloudleaf_search::{Config, Method, Ranker};
use cloudleaf_types::{ErrorCode, InputEvent, Key};
use ring::digest::{SHA256, digest};
use std::sync::Arc;

fn hash(text: &str) -> String {
    digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

const WORDS: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\tma\t甲\t\tunknown\t\tfixture\ta\tMIT\nb\tma\t乙\t\tunknown\t\tfixture\tb\tMIT\nc\tna\t丙\t\tunknown\t\tfixture\tc\tMIT\n";

fn cfg() -> Config {
    Config {
        method: Method::PositionDag,
        ranker: Ranker::Lexical,
        ..Config::default()
    }
}

fn fixtures(prefer_b: bool) -> (Arc<SearchSnapshot>, Arc<LanguageSnapshot>) {
    let snapshot = Arc::new(SearchSnapshot::from_verified_tsv(WORDS, &hash(WORDS)).unwrap());
    let rows = if prefer_b {
        "<lexical>\ta\t1\n<lexical>\tb\t100\n"
    } else {
        "<lexical>\ta\t100\n<lexical>\tb\t1\n"
    };
    let text = format!(
        "# cloudleaf-language-v1\nlexicon_sha256\t{}\nsource_sha256\t{}\nsource_id\tmechanical\nlicense_id\tMIT\ntokenization\tstable-ids\nusage\tmechanical\nkind\tlexical\ntraining_split\ttrain\nprevious\tnext\tcount\n{rows}",
        hash(WORDS),
        "a".repeat(64)
    );
    let language =
        Arc::new(LanguageSnapshot::from_verified_tsv(&text, &hash(&text), &snapshot).unwrap());
    (snapshot, language)
}

#[test]
fn model_switch_invalidates_old_candidates_and_preserves_transaction_guards() {
    let (snapshot, model_a) = fixtures(false);
    let (_, model_b) = fixtures(true);
    let mut engine = InputEngine::with_language(
        snapshot.clone(),
        cfg(),
        model_a,
        None,
        InputConfig::default(),
    )
    .unwrap();
    engine.handle(InputEvent::Activate);
    for ch in "ma".chars() {
        engine.handle(InputEvent::Key(Key::Char(ch)));
    }
    let old = engine.frame();
    assert_eq!(old.candidates[0].text, "甲");
    engine
        .reconfigure_search(cfg(), Some(model_b.clone()), None)
        .unwrap();
    let new = engine.frame();
    assert_eq!(new.preedit, old.preedit);
    assert_eq!(new.candidates[0].text, "乙");
    assert!(new.revision > old.revision);
    assert_eq!(
        engine
            .select_candidate(old.revision, &old.candidates[0].id)
            .error
            .unwrap()
            .code,
        ErrorCode::StaleCandidate
    );
    assert_eq!(
        engine.search_diagnostic().unwrap().model_version.as_deref(),
        Some(model_b.version())
    );
    assert!(new.candidates.iter().all(|c| c.frequency.is_none()));
    let commit = engine.handle(InputEvent::Key(Key::Space)).commit.unwrap();
    assert_eq!(commit.text, "乙");
    assert!(
        engine
            .reconfigure_search(cfg(), Some(model_b.clone()), None)
            .is_err()
    );
    engine.handle(InputEvent::CommitFailed { id: commit.id });
    assert!(
        engine
            .reconfigure_search(cfg(), Some(model_b), None)
            .is_err()
    );
    assert!(engine.replace_search_snapshot(snapshot).is_err());
}

#[test]
fn no_model_and_wrong_dictionary_or_hash_are_rejected_before_queries() {
    let (snapshot, language) = fixtures(true);
    assert!(InputEngine::with_search(snapshot.clone(), cfg(), InputConfig::default()).is_err());
    assert!(LanguageSnapshot::from_verified_tsv("wrong", &"0".repeat(64), &snapshot).is_err());
    let changed = WORDS.replace("fixture", "different");
    let next = Arc::new(SearchSnapshot::from_verified_tsv(&changed, &hash(&changed)).unwrap());
    assert!(
        InputEngine::with_language(
            next.clone(),
            cfg(),
            language.clone(),
            None,
            InputConfig::default()
        )
        .is_err()
    );
    let mut engine =
        InputEngine::with_language(snapshot, cfg(), language, None, InputConfig::default())
            .unwrap();
    assert!(engine.replace_search_snapshot(next).is_err());
}

#[test]
fn explicit_left_context_changes_only_the_finite_bigram_pool() {
    let (snapshot, _) = fixtures(false);
    let text = format!(
        "# cloudleaf-language-v1\nlexicon_sha256\t{}\nsource_sha256\t{}\nsource_id\tmechanical\nlicense_id\tMIT\ntokenization\tstable-ids\nusage\tmechanical\nkind\tbigram\ntraining_split\ttrain\nprevious\tnext\tcount\n<bos>\ta\t1\n<bos>\tb\t20\n<bos>\tc\t11\na\t<eos>\t11\nb\t<eos>\t21\nc\ta\t10\nc\tb\t1\n",
        hash(WORDS),
        "a".repeat(64)
    );
    let language =
        Arc::new(LanguageSnapshot::from_verified_tsv(&text, &hash(&text), &snapshot).unwrap());
    let config = Config {
        method: Method::ExactDag,
        ranker: Ranker::ContextRerank,
        ..Config::default()
    };
    let mut engine = InputEngine::with_language(
        snapshot,
        config.clone(),
        language.clone(),
        None,
        InputConfig::default(),
    )
    .unwrap();
    engine.handle(InputEvent::Activate);
    for ch in "ma".chars() {
        engine.handle(InputEvent::Key(Key::Char(ch)));
    }
    assert_eq!(engine.frame().candidates[0].text, "乙");
    engine
        .reconfigure_search(config, Some(language), Some("c".into()))
        .unwrap();
    assert_eq!(engine.frame().candidates[0].text, "甲");
    let diagnostic = engine.search_diagnostic().unwrap();
    assert!(diagnostic.finite_pool_only);
    assert_ne!(diagnostic.paths[0].features.context_units, 0);
}

#[test]
fn character_model_switch_preserves_raw_revision_and_commit_guards() {
    let (snapshot, lexical) = fixtures(true);
    let text = format!(
        "# cloudleaf-language-v1\nlexicon_sha256\t{}\nsource_sha256\t{}\nsource_id\tmechanical\nlicense_id\tMIT\ntokenization\tunicode-scalar-v1\nusage\tmechanical\nkind\tcharacter_bigram\ntraining_split\ttrain\nprevious\tnext\tcount\n<bos>\t甲\t100\n<bos>\t乙\t1\n甲\t<eos>\t100\n乙\t<eos>\t1\n",
        snapshot.version(),
        "a".repeat(64)
    );
    let language =
        Arc::new(LanguageSnapshot::from_verified_tsv(&text, &hash(&text), &snapshot).unwrap());
    let character = Config {
        method: Method::ExactDag,
        ranker: Ranker::CharacterBigram,
        ..cfg()
    };
    assert!(
        InputEngine::with_search(snapshot.clone(), character.clone(), InputConfig::default())
            .is_err()
    );
    assert!(
        InputEngine::with_language(
            snapshot.clone(),
            character.clone(),
            lexical.clone(),
            None,
            InputConfig::default()
        )
        .is_err()
    );
    assert!(
        InputEngine::with_language(
            snapshot.clone(),
            cfg(),
            language.clone(),
            None,
            InputConfig::default()
        )
        .is_err()
    );
    let mut engine = InputEngine::with_language(
        snapshot.clone(),
        cfg(),
        lexical.clone(),
        None,
        InputConfig::default(),
    )
    .unwrap();
    engine.handle(InputEvent::Activate);
    for ch in "ma".chars() {
        engine.handle(InputEvent::Key(Key::Char(ch)));
    }
    let old = engine.frame();
    assert_eq!(old.candidates[0].text, "乙");
    engine
        .reconfigure_search(character.clone(), Some(language.clone()), None)
        .unwrap();
    let new = engine.frame();
    assert_eq!(new.preedit, old.preedit);
    assert_eq!(new.candidates[0].text, "甲");
    assert!(new.revision > old.revision);
    assert_eq!(
        engine
            .select_candidate(old.revision, &old.candidates[0].id)
            .error
            .unwrap()
            .code,
        ErrorCode::StaleCandidate
    );
    let diagnostic = engine.search_diagnostic().unwrap();
    assert_eq!(
        diagnostic.paths[0].score,
        diagnostic.paths[0].features.character_units
    );
    assert_eq!(
        diagnostic.model_version.as_deref(),
        Some(language.version())
    );
    let commit = engine.handle(InputEvent::Key(Key::Space)).commit.unwrap();
    assert_eq!(commit.text, "甲");
    assert!(
        engine
            .reconfigure_search(cfg(), Some(lexical.clone()), None)
            .is_err()
    );
    engine.handle(InputEvent::CommitFailed { id: commit.id });
    assert!(
        engine
            .reconfigure_search(cfg(), Some(lexical), None)
            .is_err()
    );
    assert!(engine.replace_search_snapshot(snapshot).is_err());
    assert!(
        InputEngine::with_language(
            fixtures(false).0,
            character.clone(),
            language.clone(),
            None,
            InputConfig::default()
        )
        .is_err()
    );
    let snapshot = engine.search_diagnostic().unwrap().snapshot_version.clone();
    assert_eq!(snapshot, language.metadata().lexicon_sha256);
    let snapshot = fixtures(false).0;
    let language =
        Arc::new(LanguageSnapshot::from_verified_tsv(&text, &hash(&text), &snapshot).unwrap());
    let mut engine =
        InputEngine::with_language(snapshot, character, language, None, InputConfig::default())
            .unwrap();
    engine.handle(InputEvent::Activate);
    for ch in "qz".chars() {
        engine.handle(InputEvent::Key(Key::Char(ch)));
    }
    assert!(engine.frame().candidates.is_empty());
    assert_eq!(
        engine
            .handle(InputEvent::Key(Key::Enter))
            .commit
            .unwrap()
            .text,
        "qz"
    );
}

#[test]
fn resource_switch_keeps_complete_provenance_and_rejects_old_transactions() {
    let (base, model) = fixtures(true);
    let rows = concat!(
        "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n",
        "a\tma\t甲\t\tunknown\t\tfixture\ta\tMIT\n",
        "other\tma\t甲\t\tunknown\t\talternate\tone\tMIT\n",
        "different\tmai\t甲\t\tunknown\t\talternate\ttwo\tMIT\n"
    );
    let supplement = SearchSnapshot::from_verified_tsv(rows, &hash(rows)).unwrap();
    let stack = Arc::new(base.with_supplement(&supplement).unwrap());
    assert_eq!(stack.entries().len(), 5);
    assert_eq!(
        stack
            .entries()
            .iter()
            .filter(|row| row.text == "甲")
            .count(),
        3
    );
    let corrupt = rows.replace("a\tma\t甲", "a\tma\t丁");
    let conflicting = SearchSnapshot::from_verified_tsv(&corrupt, &hash(&corrupt)).unwrap();
    assert!(base.with_supplement(&conflicting).is_err());

    let mut engine = InputEngine::with_language(
        base.clone(),
        cfg(),
        model.clone(),
        None,
        InputConfig::default(),
    )
    .unwrap();
    engine.handle(InputEvent::Activate);
    assert!(
        engine
            .prepare_idle_resources(stack.clone(), Some(model.clone()))
            .is_err()
    );
    for ch in "ma".chars() {
        engine.handle(InputEvent::Key(Key::Char(ch)));
    }
    engine.handle(InputEvent::Key(Key::Left));
    let previous = engine.frame();
    assert!(engine.prepare_idle_resources(stack.clone(), None).is_err());
    assert_eq!(previous, engine.frame());
    let request = engine.handle(InputEvent::Key(Key::Space)).commit.unwrap();
    assert!(engine.prepare_idle_resources(stack.clone(), None).is_err());
    engine.handle(InputEvent::CommitFailed { id: request.id });
    assert!(engine.prepare_idle_resources(stack.clone(), None).is_err());
    engine.handle(InputEvent::Key(Key::Escape));

    let revision = engine.frame().revision;
    engine
        .prepare_idle_resources(stack.clone(), None)
        .unwrap()
        .apply();
    assert!(engine.frame().revision > revision);
    assert_eq!(engine.search_snapshot_version(), Some(stack.version()));
    assert!(
        engine
            .translation_source_identity(None)
            .input_model
            .is_none()
    );
    for ch in "ma".chars() {
        engine.handle(InputEvent::Key(Key::Char(ch)));
    }
    assert_eq!(
        engine
            .frame()
            .candidates
            .iter()
            .filter(|candidate| candidate.text == "甲")
            .count(),
        1
    );
    assert_eq!(
        engine
            .select_candidate(previous.revision, &previous.candidates[0].id)
            .error
            .unwrap()
            .code,
        ErrorCode::StaleCandidate,
    );
    engine.handle(InputEvent::Key(Key::Escape));
    engine
        .prepare_idle_resources(base.clone(), Some(model.clone()))
        .unwrap()
        .apply();
    assert_eq!(
        engine
            .translation_source_identity(None)
            .input_model
            .as_deref(),
        Some(model.version())
    );
    for ch in "ma".chars() {
        engine.handle(InputEvent::Key(Key::Char(ch)));
    }
    assert_eq!(engine.frame().candidates[0].text, "乙");
}
