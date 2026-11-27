use cloudleaf_input::{
    DoublePinyinLayout, InputConfig, InputEngine, PinyinScheme, SchemeConfig, SearchSnapshot,
};
use cloudleaf_search::{Budget, Config, Method, Ranker};
use cloudleaf_types::{ErrorCode, InputEvent, Key};
use ring::digest::{SHA256, digest};
use std::sync::Arc;

fn engine() -> InputEngine {
    let text = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\nnihao\tni hao\t你好\t\tunknown\t\tmechanical\tnihao\tMIT\nnonghu\tnong hu\t农户\t\tunknown\t\tmechanical\tnonghu\tMIT\n";
    let sha: String = digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    InputEngine::with_search(
        Arc::new(SearchSnapshot::from_verified_tsv(text, &sha).unwrap()),
        Config {
            ranker: Ranker::Structural,
            output_k: 10,
            ..Default::default()
        },
        InputConfig::default(),
    )
    .unwrap()
}
fn type_keys(engine: &mut InputEngine, keys: &str) {
    for key in keys.chars() {
        assert!(
            engine
                .handle(InputEvent::Key(Key::Char(key)))
                .error
                .is_none()
        );
    }
}

#[test]
fn scheme_switch_keeps_original_and_cursor_but_invalidates_revision_and_id() {
    let mut engine = engine();
    engine.handle(InputEvent::Activate);
    type_keys(&mut engine, "nh");
    engine
        .reconfigure_scheme(SchemeConfig {
            scheme: PinyinScheme::AbbreviatedPinyin,
            ..Default::default()
        })
        .unwrap();
    let old = engine.frame();
    assert_eq!(old.preedit, "nh");
    assert_eq!(old.cursor, 2);
    assert!(old.candidates.iter().any(|c| c.text == "你好"));
    engine
        .reconfigure_scheme(SchemeConfig {
            scheme: PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
            ..Default::default()
        })
        .unwrap();
    let new = engine.frame();
    assert_eq!(new.preedit, old.preedit);
    assert!(new.revision > old.revision);
    assert_eq!(
        engine
            .select_candidate(old.revision, &old.candidates[0].id)
            .error
            .unwrap()
            .code,
        ErrorCode::StaleCandidate
    );
    engine
        .reconfigure_scheme(SchemeConfig {
            scheme: PinyinScheme::AbbreviatedPinyin,
            ..Default::default()
        })
        .unwrap();
    assert!(
        engine
            .select_candidate(old.revision, &old.candidates[0].id)
            .error
            .is_some()
    );
}

#[test]
fn unknown_pending_and_not_applied_failed_transactions_cannot_switch_scheme() {
    let mut engine = engine();
    engine.handle(InputEvent::Activate);
    type_keys(&mut engine, "nihao");
    let request = engine.handle(InputEvent::Key(Key::Space)).commit.unwrap();
    let pending = engine.frame();
    let scheme = SchemeConfig {
        scheme: PinyinScheme::AbbreviatedPinyin,
        ..Default::default()
    };
    assert_eq!(
        engine.reconfigure_scheme(scheme.clone()).unwrap_err().code,
        ErrorCode::CommitPending
    );
    assert_eq!(engine.frame(), pending);
    engine.handle(InputEvent::CommitFailed { id: request.id });
    let failed = engine.frame();
    assert_eq!(
        engine.reconfigure_scheme(scheme).unwrap_err().code,
        ErrorCode::CommitPending
    );
    assert_eq!(engine.frame(), failed);
}

#[test]
fn invalid_scheme_configuration_is_atomic_and_does_not_change_raw() {
    let mut engine = engine();
    engine.handle(InputEvent::Activate);
    type_keys(&mut engine, "nihao");
    let before = engine.frame();
    let mut invalid = SchemeConfig::default();
    invalid.limits.max_work = 0;
    assert_eq!(
        engine.reconfigure_scheme(invalid).unwrap_err().code,
        ErrorCode::InvalidConfig
    );
    assert_eq!(engine.frame(), before);
}

fn capped_engine(method: Method, max_expansions: usize) -> InputEngine {
    let mut text = String::from(
        "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n",
    );
    for index in 0..24 {
        let word = char::from_u32(0x4e00 + index).unwrap();
        text.push_str(&format!(
            "one-{index}\tni\t{word}\t\tunknown\t\tfixture\t{index}\tMIT\n"
        ));
    }
    text.push_str("two\tkan\t看\t\tunknown\t\tfixture\ttwo\tMIT\n");
    let sha: String = digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let mut engine = InputEngine::with_search(
        Arc::new(SearchSnapshot::from_verified_tsv(&text, &sha).unwrap()),
        Config {
            method,
            ranker: Ranker::Structural,
            output_k: 8,
            max_edges: 8,
            max_expansions,
            ..Config::default()
        },
        InputConfig::default(),
    )
    .unwrap();
    engine.handle(InputEvent::Activate);
    engine
        .reconfigure_scheme(SchemeConfig {
            scheme: PinyinScheme::AbbreviatedPinyin,
            ..SchemeConfig::default()
        })
        .unwrap();
    engine
}

#[test]
fn capped_complete_abbreviation_is_selectable_and_literal_confirmation_remains_available() {
    let mut engine = capped_engine(Method::PositionDag, Config::default().max_expansions);
    type_keys(&mut engine, "nk");
    let frame = engine.frame();
    let diagnostic = engine.search_diagnostic().unwrap();
    assert_eq!(diagnostic.statistics.budget, Some(Budget::GraphEdges));
    assert!(!diagnostic.guaranteed);
    let candidate = &frame.candidates[0];
    assert!(candidate.text.ends_with('看'));
    assert_eq!(candidate.id, diagnostic.paths[0].candidate_id);
    let commit = engine
        .select_candidate(frame.revision, &candidate.id)
        .commit
        .unwrap();
    assert_eq!(commit.text, candidate.text);
    engine.handle(InputEvent::CommitSucceeded { id: commit.id });
    assert!(engine.frame().preedit.is_empty());

    type_keys(&mut engine, "nk");
    let literal = engine
        .confirm_literal(engine.frame().revision, "")
        .commit
        .unwrap();
    assert_eq!(literal.text, "nk");
}

#[test]
fn capped_abbreviation_keeps_literal_fallback_outside_the_complete_position_search() {
    for (method, limit, input, expected_budget) in [
        (
            Method::PositionDag,
            Config::default().max_expansions,
            "nq",
            Budget::GraphEdges,
        ),
        (
            Method::Reference,
            Config::default().max_expansions,
            "nk",
            Budget::GraphEdges,
        ),
        (Method::PositionDag, 1, "nk", Budget::Intersection),
    ] {
        let mut engine = capped_engine(method, limit);
        type_keys(&mut engine, input);
        assert_eq!(
            engine.search_diagnostic().unwrap().statistics.budget,
            Some(expected_budget)
        );
        assert!(engine.frame().candidates.is_empty());
        let commit = engine.handle(InputEvent::Key(Key::Space)).commit.unwrap();
        assert_eq!(commit.text, input);
    }
}
