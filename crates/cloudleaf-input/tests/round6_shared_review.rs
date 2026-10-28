//! 独立审阅只通过公开入口驱动真实核心，不复制候选或排序算法。
use cloudleaf_dictionary::Dictionary;
use cloudleaf_input::{InputConfig, InputEngine};
use cloudleaf_types::{
    ErrorCode, InputEvent, InputMode, Key, KeyDisposition, utf8_to_utf16_offset,
};

fn prepared(config: InputConfig, events: &[InputEvent]) -> InputEngine {
    let dictionary =
        Dictionary::from_tsv(include_str!("../../../data/dictionaries/prototype.tsv")).unwrap();
    let mut engine = InputEngine::with_config(dictionary, config).unwrap();
    for event in events {
        engine.handle(*event);
    }
    engine
}

#[test]
fn prediction_and_dispatch_agree_across_pages_limits_and_commit_states() {
    let mut composing = vec![InputEvent::Activate];
    composing.extend("ni".chars().map(|c| InputEvent::Key(Key::Char(c))));
    let mut scenarios = vec![vec![], vec![InputEvent::Activate], composing.clone()];
    for suffix in [
        vec![InputEvent::Key(Key::PageDown)],
        vec![InputEvent::Key(Key::Enter)],
        vec![
            InputEvent::Key(Key::Enter),
            InputEvent::CommitFailed { id: 1 },
        ],
        vec![
            InputEvent::Key(Key::Enter),
            InputEvent::CommitFailed { id: 1 },
            InputEvent::RetryCommit,
        ],
        vec![InputEvent::SetMode(InputMode::English)],
        vec![InputEvent::FocusLost],
        vec![InputEvent::Deactivate],
    ] {
        let mut events = composing.clone();
        events.extend(suffix);
        scenarios.push(events);
    }
    let mut full = vec![InputEvent::Activate];
    full.extend((0..64).map(|_| InputEvent::Key(Key::Char('x'))));
    scenarios.push(full);
    let mut keys = vec![
        Key::Backspace,
        Key::Space,
        Key::Enter,
        Key::Escape,
        Key::Left,
        Key::Right,
        Key::Up,
        Key::Down,
        Key::PageUp,
        Key::PageDown,
    ];
    keys.extend(
        "aZ'0123456789,:;.!? 你好😀\n\r\t\0\u{85}\u{301}\u{200d}"
            .chars()
            .map(Key::Char),
    );
    for (page_size, max_candidates) in [(1, 1), (2, 3), (5, 50), (9, 128)] {
        let config = InputConfig {
            page_size,
            max_candidates,
        };
        for events in &scenarios {
            for key in &keys {
                let mut engine = prepared(config, events);
                let frame = engine.frame();
                let failed = engine.failed_commit().cloned();
                let predicted = engine.key_disposition(*key);
                for _ in 0..3 {
                    assert_eq!(engine.key_disposition(*key), predicted);
                    assert_eq!(engine.frame(), frame);
                    assert_eq!(engine.failed_commit().cloned(), failed);
                }
                assert!(frame.candidates.len() <= max_candidates);
                let result = engine.handle(InputEvent::Key(*key));
                match predicted {
                    KeyDisposition::Handle => assert!(result.handled && result.error.is_none()),
                    KeyDisposition::PassThrough => {
                        assert!(!result.handled && result.error.is_none())
                    }
                    KeyDisposition::Blocked(code) => {
                        assert!(!result.handled && result.commit.is_none());
                        assert_eq!(result.error.unwrap().code, code);
                        assert_eq!(result.frame, frame);
                    }
                }
            }
        }
    }
}

#[test]
fn candidate_identity_rejections_leave_live_and_failed_contexts_unchanged() {
    let events = [
        InputEvent::Activate,
        InputEvent::Key(Key::Char('n')),
        InputEvent::Key(Key::Char('i')),
    ];
    for change in [
        InputEvent::Key(Key::Left),
        InputEvent::FocusLost,
        InputEvent::SetMode(InputMode::English),
        InputEvent::Deactivate,
    ] {
        let mut engine = prepared(InputConfig::default(), &events);
        let old = engine.frame();
        let id = old.candidates[0].id.clone();
        engine.handle(change);
        let current = engine.frame();
        let rejected = engine.select_candidate(old.revision, &id);
        assert_eq!(
            rejected.error.unwrap().code,
            if change == InputEvent::Deactivate {
                ErrorCode::InvalidInput
            } else {
                ErrorCode::StaleCandidate
            }
        );
        assert!(!rejected.handled && rejected.commit.is_none());
        assert_eq!(rejected.frame, current);
    }
    let mut engine = prepared(InputConfig::default(), &events);
    let shown = engine.frame();
    let request = engine
        .select_candidate(shown.revision, &shown.candidates[0].id)
        .commit
        .unwrap();
    let pending = engine.frame();
    let blocked = engine.select_candidate(shown.revision, "missing-id");
    assert_eq!(blocked.error.unwrap().code, ErrorCode::CommitPending);
    assert_eq!(blocked.frame, pending);
    engine.handle(InputEvent::CommitFailed { id: request.id });
    let failed_frame = engine.frame();
    let failed = engine.failed_commit().cloned();
    let blocked = engine.select_candidate(shown.revision, "missing-id");
    assert_eq!(blocked.error.unwrap().code, ErrorCode::CommitFailed);
    assert_eq!(blocked.frame, failed_frame);
    assert_eq!(engine.failed_commit().cloned(), failed);
}

#[test]
fn unconfirmed_transaction_never_creates_a_retry_or_reuses_cancelled_identity() {
    let mut engine = prepared(
        InputConfig::default(),
        &[InputEvent::Activate, InputEvent::Key(Key::Char('n'))],
    );
    let old = engine.handle(InputEvent::Key(Key::Enter)).commit.unwrap();
    let pending = engine.frame();
    // 平台结果Unknown时不发送任何成功/失败确认，核心仍保持pending。
    for event in [
        InputEvent::RetryCommit,
        InputEvent::Key(Key::Char('a')),
        InputEvent::RetryCommit,
    ] {
        let result = engine.handle(event);
        assert_eq!(result.error.unwrap().code, ErrorCode::CommitPending);
        assert!(result.commit.is_none());
        assert_eq!(result.frame, pending);
    }
    engine.handle(InputEvent::FocusLost);
    engine.handle(InputEvent::Key(Key::Char('i')));
    let fresh = engine.handle(InputEvent::Key(Key::Enter)).commit.unwrap();
    assert_ne!(old.id, fresh.id);
    let current = engine.frame();
    for event in [
        InputEvent::CommitSucceeded { id: old.id },
        InputEvent::CommitFailed { id: old.id },
    ] {
        let result = engine.handle(event);
        assert_eq!(result.error.unwrap().code, ErrorCode::StaleCommit);
        assert!(result.commit.is_none());
        assert_eq!(result.frame, current);
    }
    assert!(
        engine
            .handle(InputEvent::CommitSucceeded { id: fresh.id })
            .error
            .is_none()
    );
}

#[test]
fn utf16_conversion_uses_scalar_boundaries_without_claiming_grapheme_editing() {
    for (text, offsets) in [
        ("", vec![(0, 0)]),
        ("a你𠀀b", vec![(0, 0), (1, 1), (4, 2), (8, 4), (9, 5)]),
        ("e\u{301}", vec![(0, 0), (1, 1), (3, 2)]),
        ("👩\u{200d}💻", vec![(0, 0), (4, 2), (7, 3), (11, 5)]),
        ("\r\n\0", vec![(0, 0), (1, 1), (2, 2), (3, 3)]),
    ] {
        for byte in 0..=text.len() + 1 {
            if let Some((_, expected)) = offsets.iter().find(|(boundary, _)| *boundary == byte) {
                assert_eq!(utf8_to_utf16_offset(text, byte).unwrap(), *expected);
            } else {
                assert_eq!(
                    utf8_to_utf16_offset(text, byte).unwrap_err().code,
                    ErrorCode::InvalidInput
                );
            }
        }
        assert!(utf8_to_utf16_offset(text, usize::MAX).is_err());
    }
}
