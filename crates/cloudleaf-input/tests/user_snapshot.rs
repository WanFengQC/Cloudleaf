use cloudleaf_input::{InputConfig, InputEngine, SearchSnapshot};
use cloudleaf_search::Config;
use cloudleaf_types::{InputEvent, Key};
use ring::digest::{SHA256, digest};
use std::{collections::BTreeMap, sync::Arc};
const HEADER: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";
fn base() -> Arc<SearchSnapshot> {
    let t = format!("{HEADER}base\tyun\t云\t\tunknown\t\tfixture\tbase\tMIT\n");
    let sha: String = digest(&SHA256, t.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Arc::new(SearchSnapshot::from_verified_tsv(&t, &sha).unwrap())
}
#[test]
fn user_snapshot_switch_invalidates_old_candidate_and_pending_evidence_is_exact() {
    let base = base();
    let users = format!(
        "{HEADER}user:1\tyun jian\t云笺\t\tunknown\t\tuser\tuser:1\tLicenseRef-User-Private\n"
    );
    let user = Arc::new(base.with_user_layer(&users, &BTreeMap::new()).unwrap());
    let mut engine =
        InputEngine::with_search(base.clone(), Config::default(), InputConfig::default()).unwrap();
    engine.handle(InputEvent::Activate);
    for c in "yun".chars() {
        engine.handle(InputEvent::Key(Key::Char(c)));
    }
    let old = engine.frame();
    engine.replace_search_resources(user, None).unwrap();
    assert!(
        engine
            .select_candidate(old.revision, &old.candidates[0].id)
            .error
            .is_some()
    );
    for c in "jian".chars() {
        engine.handle(InputEvent::Key(Key::Char(c)));
    }
    let result = engine.handle(InputEvent::Key(Key::Space));
    let request = result.commit.unwrap();
    assert_eq!(request.text, "云笺");
    let evidence = engine.pending_selection().unwrap().clone();
    assert_eq!(evidence.commit_id, request.id);
    assert_eq!(evidence.entries.len(), 1);
    assert_eq!(evidence.entries[0].0, "user:1");
    assert!(engine.replace_search_resources(base, None).is_err());
    engine.handle(InputEvent::CommitSucceeded { id: request.id });
    assert!(engine.pending_selection().is_none());
}
#[test]
fn raw_fallback_and_punctuation_do_not_create_learning_evidence() {
    let mut e =
        InputEngine::with_search(base(), Config::default(), InputConfig::default()).unwrap();
    e.handle(InputEvent::Activate);
    for c in "qz".chars() {
        e.handle(InputEvent::Key(Key::Char(c)));
    }
    e.handle(InputEvent::Key(Key::Enter));
    assert!(e.pending_selection().is_none());
    e.handle(InputEvent::Key(Key::Escape));
    for c in "yun".chars() {
        e.handle(InputEvent::Key(Key::Char(c)));
    }
    e.handle(InputEvent::Key(Key::Char('.')));
    assert!(e.pending_selection().is_none());
}

#[test]
fn explicit_retry_keeps_selection_provenance_but_changes_attempt_id() {
    let mut e =
        InputEngine::with_search(base(), Config::default(), InputConfig::default()).unwrap();
    e.handle(InputEvent::Activate);
    for c in "yun".chars() {
        e.handle(InputEvent::Key(Key::Char(c)));
    }
    let first = e.handle(InputEvent::Key(Key::Space)).commit.unwrap();
    let original = e.pending_selection().unwrap().clone();
    e.handle(InputEvent::CommitFailed { id: first.id });
    let retry = e.handle(InputEvent::RetryCommit).commit.unwrap();
    let proof = e.pending_selection().unwrap();
    assert_ne!(proof.commit_id, original.commit_id);
    assert_eq!(proof.commit_id, retry.id);
    assert_eq!(proof.entries, original.entries);
    assert!(
        e.handle(InputEvent::CommitSucceeded { id: first.id })
            .error
            .is_some()
    );
    assert!(e.pending_selection().is_some());
    e.handle(InputEvent::CommitSucceeded { id: retry.id });
    assert!(e.pending_selection().is_none());
}
