use cloudleaf_dictionary::{Dictionary, PROTOTYPE_TSV};
use cloudleaf_input::{InputEngine, MAX_TRANSLATION_COMMIT_BYTES};
use cloudleaf_types::{ErrorCode, InputEvent, InputMode, InputState, Key};

#[test]
fn translated_text_uses_existing_commit_receipt_and_never_creates_learning_evidence() {
    let mut engine = InputEngine::new(Dictionary::from_tsv(PROTOTYPE_TSV).unwrap());
    engine.handle(InputEvent::Activate);
    for key in "nihao".chars() {
        engine.handle(InputEvent::Key(Key::Char(key)));
    }
    let original = engine.frame();
    assert!(!original.candidates.is_empty());

    let first = engine
        .confirm_translation(original.revision, "Hello")
        .commit
        .unwrap();
    assert_eq!(first.text, "Hello");
    assert_eq!(engine.frame().preedit, original.preedit);
    assert!(engine.pending_selection().is_none());
    assert_eq!(
        engine
            .confirm_translation(engine.revision(), "Hello")
            .error
            .unwrap()
            .code,
        ErrorCode::CommitPending
    );

    engine.handle(InputEvent::CommitFailed { id: first.id });
    assert_eq!(engine.frame().preedit, original.preedit);
    let retry = engine.handle(InputEvent::RetryCommit).commit.unwrap();
    assert_ne!(first.id, retry.id);
    assert_eq!(first.text, retry.text);
    assert!(engine.pending_selection().is_none());
    assert_eq!(
        engine
            .handle(InputEvent::CommitSucceeded { id: first.id })
            .error
            .unwrap()
            .code,
        ErrorCode::StaleCommit
    );
    engine.handle(InputEvent::CommitSucceeded { id: retry.id });
    assert_eq!(engine.frame().state, InputState::Idle);
    assert!(engine.frame().preedit.is_empty());
}

#[test]
fn translation_confirmation_rejects_inactive_stale_and_invalid_text() {
    let mut engine = InputEngine::new(Dictionary::from_tsv(PROTOTYPE_TSV).unwrap());
    assert!(
        engine
            .confirm_translation(engine.revision(), "你好")
            .commit
            .is_none()
    );
    engine.handle(InputEvent::Activate);
    let old_revision = engine.revision();
    engine.handle(InputEvent::SetMode(InputMode::English));
    let current = engine.frame();
    assert_eq!(
        engine
            .confirm_translation(old_revision, "你好")
            .error
            .unwrap()
            .code,
        ErrorCode::StaleCandidate
    );
    for text in [
        "",
        "  ",
        "译文\0其余",
        &"a".repeat(MAX_TRANSLATION_COMMIT_BYTES + 1),
    ] {
        assert!(
            engine
                .confirm_translation(current.revision, text)
                .commit
                .is_none()
        );
        assert_eq!(engine.frame(), current);
    }
    let request = engine
        .confirm_translation(current.revision, "你好。")
        .commit
        .unwrap();
    assert_eq!(request.text, "你好。");
    assert!(engine.pending_selection().is_none());
}

#[test]
fn discarded_not_applied_translation_preserves_input_and_cannot_be_replayed() {
    let mut engine = InputEngine::new(Dictionary::from_tsv(PROTOTYPE_TSV).unwrap());
    engine.handle(InputEvent::Activate);
    for key in "nihao".chars() {
        engine.handle(InputEvent::Key(Key::Char(key)));
    }
    let source = engine.frame();
    assert_eq!(
        engine.validate_translation(source.revision, "Hello."),
        Ok(())
    );
    assert_eq!(engine.frame(), source);

    let first = engine
        .confirm_translation(source.revision, "Hello.")
        .commit
        .unwrap();
    assert!(engine.discard_failed_translation(first.id).is_err());
    engine.handle(InputEvent::CommitFailed { id: first.id });
    assert_eq!(engine.validate_translation_retry(first.id), Ok(()));
    let retry = engine.handle(InputEvent::RetryCommit).commit.unwrap();
    assert_ne!(retry.id, first.id);
    engine.handle(InputEvent::CommitFailed { id: retry.id });
    assert!(engine.discard_failed_translation(first.id).is_err());
    let mut before_discard = engine.frame();
    engine.discard_failed_translation(retry.id).unwrap();
    before_discard.revision += 1;
    assert_eq!(engine.frame(), before_discard);
    assert!(engine.handle(InputEvent::RetryCommit).commit.is_none());
    assert!(engine.handle(InputEvent::Key(Key::Char('n'))).handled);
}

#[test]
fn unlearned_punctuation_failure_is_not_a_failed_translation() {
    let mut engine = InputEngine::new(Dictionary::from_tsv(PROTOTYPE_TSV).unwrap());
    engine.handle(InputEvent::Activate);
    let ordinary = engine
        .handle(InputEvent::Key(Key::Char('.')))
        .commit
        .unwrap();
    engine.handle(InputEvent::CommitFailed { id: ordinary.id });
    assert!(engine.pending_selection().is_none());
    assert!(engine.discard_failed_translation(ordinary.id).is_err());
    assert_eq!(engine.failed_commit(), Some(&ordinary));
}
