use cloudleaf_input::{
    DoublePinyinLayout, InputConfig, InputEngine, PinyinScheme, SchemeConfig, SearchSnapshot,
};
use cloudleaf_search::{Config, Ranker};
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
