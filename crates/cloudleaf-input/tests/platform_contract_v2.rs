//! 平台v2的只读预判、候选身份和显示配置回归。

use cloudleaf_dictionary::Dictionary;
use cloudleaf_input::{InputConfig, InputEngine};
use cloudleaf_types::{
    ErrorCode, InputEvent, InputMode, Key, KeyDisposition, utf8_to_utf16_offset,
};

fn engine(events: &[InputEvent]) -> InputEngine {
    let dictionary =
        Dictionary::from_tsv(include_str!("../../../data/dictionaries/prototype.tsv")).unwrap();
    let mut engine = InputEngine::new(dictionary);
    for event in events {
        engine.handle(*event);
    }
    engine
}

#[test]
fn readonly_prediction_matches_dispatch_in_all_input_states() {
    let mut scenarios = vec![
        vec![],
        vec![InputEvent::Activate],
        vec![
            InputEvent::Activate,
            InputEvent::SetMode(InputMode::English),
        ],
    ];
    let mut composing = vec![InputEvent::Activate];
    composing.extend("ni".chars().map(|c| InputEvent::Key(Key::Char(c))));
    scenarios.push(composing.clone());
    let mut pending = composing.clone();
    pending.push(InputEvent::Key(Key::Enter));
    scenarios.push(pending.clone());
    pending.push(InputEvent::CommitFailed { id: 1 });
    scenarios.push(pending);
    let mut full = vec![InputEvent::Activate];
    full.extend((0..64).map(|_| InputEvent::Key(Key::Char('x'))));
    scenarios.push(full);
    let mut keys = vec![
        Key::Backspace,
        Key::Space,
        Key::Enter,
        Key::Escape,
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::PageUp,
        Key::PageDown,
    ];
    keys.extend("azAZ'0123456789 ,.;!?你好\n\r\t".chars().map(Key::Char));
    for events in scenarios {
        for key in &keys {
            let mut instance = engine(&events);
            let before = instance.frame();
            let prediction = instance.key_disposition(*key);
            assert_eq!(instance.key_disposition(*key), prediction);
            assert_eq!(instance.frame(), before, "只读预判不得改变修订、候选或组合");
            let result = instance.handle(InputEvent::Key(*key));
            match prediction {
                KeyDisposition::PassThrough => assert!(!result.handled && result.error.is_none()),
                KeyDisposition::Handle => assert!(result.handled && result.error.is_none()),
                KeyDisposition::Blocked(code) => assert_eq!(result.error.unwrap().code, code),
            }
        }
    }
}

#[test]
fn candidate_selection_requires_current_revision_and_id() {
    let mut instance = engine(&[
        InputEvent::Activate,
        InputEvent::Key(Key::Char('n')),
        InputEvent::Key(Key::Char('i')),
    ]);
    let shown = instance.frame();
    instance.handle(InputEvent::Key(Key::Down));
    let current = instance.frame();
    let stale = instance.select_candidate(shown.revision, &shown.candidates[0].id);
    assert_eq!(stale.error.unwrap().code, ErrorCode::StaleCandidate);
    assert_eq!(stale.frame, current);
    let unknown = instance.select_candidate(current.revision, "absent-id");
    assert_eq!(unknown.error.unwrap().code, ErrorCode::UnknownCandidate);
    assert_eq!(unknown.frame, current);
    let selected = &current.candidates[1];
    let commit = instance
        .select_candidate(current.revision, &selected.id)
        .commit
        .unwrap();
    assert_eq!(commit.text, selected.text);
    assert_eq!(
        instance
            .select_candidate(current.revision, &selected.id)
            .error
            .unwrap()
            .code,
        ErrorCode::CommitPending
    );
    instance.handle(InputEvent::CommitSucceeded { id: commit.id });
    assert_eq!(
        instance
            .select_candidate(current.revision, &selected.id)
            .error
            .unwrap()
            .code,
        ErrorCode::StaleCandidate
    );
}

#[test]
fn configurable_pages_obey_candidate_budget_and_current_page_digits() {
    for (page_size, max_candidates) in [(0, 5), (10, 5), (5, 0), (5, 129)] {
        assert!(
            InputEngine::with_config(
                Dictionary::empty(),
                InputConfig {
                    page_size,
                    max_candidates
                }
            )
            .is_err()
        );
    }
    let dictionary =
        Dictionary::from_tsv(include_str!("../../../data/dictionaries/prototype.tsv")).unwrap();
    let mut instance = InputEngine::with_config(
        dictionary,
        InputConfig {
            page_size: 2,
            max_candidates: 3,
        },
    )
    .unwrap();
    for event in [
        InputEvent::Activate,
        InputEvent::Key(Key::Char('n')),
        InputEvent::Key(Key::Char('i')),
        InputEvent::Key(Key::PageDown),
    ] {
        instance.handle(event);
    }
    let page = instance.frame();
    assert_eq!(
        (page.page_size, page.page_start, page.candidates.len()),
        (2, 2, 3)
    );
    let expected = page.candidates[2].text.clone();
    assert_eq!(
        instance
            .handle(InputEvent::Key(Key::Char('1')))
            .commit
            .unwrap()
            .text,
        expected
    );
    let mut one = InputEngine::with_config(
        Dictionary::empty(),
        InputConfig {
            page_size: 9,
            max_candidates: 1,
        },
    )
    .unwrap();
    one.handle(InputEvent::Activate);
    assert_eq!(one.frame().page_size, 9);
}

#[test]
fn utf16_offsets_reject_split_characters_and_count_surrogate_pairs() {
    let text = "a你𠀀b";
    for (byte, utf16) in [(0, 0), (1, 1), (4, 2), (8, 4), (9, 5)] {
        assert_eq!(utf8_to_utf16_offset(text, byte).unwrap(), utf16);
    }
    for byte in [2, 3, 5, 6, 7, 10, usize::MAX] {
        assert_eq!(
            utf8_to_utf16_offset(text, byte).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
}
