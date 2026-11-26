use cloudleaf_input::{InputConfig, InputEngine, ResearchLanguageSnapshot, SearchSnapshot};
use cloudleaf_search::{Config, Method, Ranker, ResearchRanker};
use cloudleaf_types::{ErrorCode, InputEvent, Key};
use ring::digest::{SHA256, digest};
use std::sync::Arc;

fn sha(text: &str) -> String {
    digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn fixture() -> (Arc<SearchSnapshot>, String) {
    let dictionary = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\tma\t甲\t\tunknown\t\tfixture\ta\tMIT\nb\tma\t乙\t\tunknown\t\tfixture\tb\tMIT\nc\tla\t丙\t\tunknown\t\tfixture\tc\tMIT\nd\tna\t丁\t\tunknown\t\tfixture\td\tMIT\n";
    let sha = sha(dictionary);
    let result = SearchSnapshot::from_verified_tsv(dictionary, &sha).unwrap();
    let model = format!(
        "# cloudleaf-word-context-v1\nlexicon_sha256\t{sha}\ncorpus_sha256\t{sha}\nW\t</s>\t16\nW\t<s>\t16\nW\t丁\t8\nW\t丙\t8\nW\t乙\t8\nW\t甲\t8\nB\t<s>\t乙\t8\nB\t<s>\t甲\t8\nB\t丁\t</s>\t8\nB\t丙\t</s>\t8\nB\t乙\t丁\t8\nB\t甲\t丙\t8\n"
    );
    (Arc::new(result), model)
}

#[test]
fn research_model_is_opt_in_and_switch_has_atomic_candidate_identity() {
    let (dictionary, artifact) = fixture();
    let make = |mode, dictionary: &SearchSnapshot| {
        Arc::new(
            ResearchLanguageSnapshot::from_verified_tsv(
                &artifact,
                &sha(&artifact),
                dictionary,
                None,
                mode,
                10,
            )
            .unwrap(),
        )
    };
    let config = Config {
        method: Method::PositionDag,
        ranker: Ranker::Structural,
        ..Config::default()
    };
    let mut engine =
        InputEngine::with_search(dictionary.clone(), config.clone(), InputConfig::default())
            .unwrap();
    engine.handle(InputEvent::Activate);
    for key in "mana".chars() {
        engine.handle(InputEvent::Key(Key::Char(key)));
    }
    let original = engine.frame();
    assert_eq!(original.candidates[0].text, "甲丁");
    let (different_instance, _) = fixture();
    let incompatible = make(ResearchRanker::Word, &different_instance);
    assert!(
        engine
            .replace_research_context(dictionary.clone(), config.clone(), incompatible)
            .is_err()
    );
    assert_eq!(engine.frame(), original);
    let research = make(ResearchRanker::Word, &dictionary);
    engine
        .replace_research_context(dictionary.clone(), config.clone(), research.clone())
        .unwrap();
    let changed = engine.frame();
    assert_eq!(changed.candidates[0].text, "乙丁");
    assert!(engine.search_diagnostic().unwrap().finite_pool_only);
    assert_eq!(
        engine
            .translation_source_identity(None)
            .input_model
            .as_deref(),
        Some(research.version())
    );
    assert_eq!(
        engine
            .select_candidate(original.revision, &original.candidates[0].id)
            .error
            .unwrap()
            .code,
        ErrorCode::StaleCandidate
    );
    assert_eq!(engine.frame(), changed);
    engine.confirm_translation(changed.revision, "test");
    assert!(
        engine
            .replace_research_context(dictionary, config, research)
            .is_err()
    );
}
