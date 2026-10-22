//! 使用真实输入核心验证宿主协议；模型不实现 TSF、COM 或原生文档编辑。

use std::collections::VecDeque;

use cloudleaf_dictionary::{Dictionary, PROTOTYPE_TSV};
use cloudleaf_input::InputEngine;
use cloudleaf_types::{
    CommitRequest, ErrorCode, InputEvent, InputFrame, InputMode, InputState, Key,
};

const QUEUE_LIMIT: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Session {
    instance: u64,
    epoch: u64,
}

#[derive(Debug, Clone)]
struct CommitEnvelope {
    session: Session,
    revision: u64,
    request: CommitRequest,
}

#[derive(Debug, Clone, Copy)]
enum Outcome {
    Applied,
    NotApplied,
    Unknown,
}

#[derive(Debug, PartialEq, Eq)]
enum Dispatch {
    Passed,
    Handled,
    Queued,
    Duplicate,
    Suspended,
    Full,
}

#[derive(Debug, PartialEq, Eq)]
enum Completion {
    Applied,
    Failed,
    Quarantined,
    Ignored,
}

/// 只约束身份、去重、排队与确认；不能直接作为 Windows 生产适配器。
struct HostModel {
    engine: InputEngine,
    session: Session,
    last_native_sequence: u64,
    queue: VecDeque<Key>,
    pending: Option<CommitEnvelope>,
    quarantined: bool,
    applied: Vec<String>,
}

impl HostModel {
    fn new(instance: u64) -> Self {
        let dictionary = Dictionary::from_tsv(PROTOTYPE_TSV).unwrap();
        let mut engine = InputEngine::new(dictionary);
        assert!(engine.handle(InputEvent::Activate).error.is_none());
        Self {
            engine,
            session: Session { instance, epoch: 1 },
            last_native_sequence: 0,
            queue: VecDeque::new(),
            pending: None,
            quarantined: false,
            applied: Vec::new(),
        }
    }

    /// 只读接管判断仅覆盖本组测试的按键，不声称等同完整原生 admission。
    fn probe(&self, shortcut: bool, key: Key) -> bool {
        let frame = self.engine.frame();
        if shortcut
            || self.quarantined
            || frame.state == InputState::Inactive
            || frame.mode == InputMode::English
        {
            return false;
        }
        match key {
            Key::Char(character) if character.is_ascii_alphabetic() => true,
            Key::Space | Key::Enter => !frame.preedit.is_empty(),
            _ => false,
        }
    }

    fn native_key(&mut self, sequence: u64, shortcut: bool, key: Key) -> Dispatch {
        if sequence <= self.last_native_sequence {
            return Dispatch::Duplicate;
        }
        if shortcut {
            self.last_native_sequence = sequence;
            return Dispatch::Passed;
        }
        if self.quarantined || self.engine.failed_commit().is_some() {
            return Dispatch::Suspended;
        }
        if self.pending.is_some() {
            if self.queue.len() == QUEUE_LIMIT {
                // 拒绝取得所有权，调用方仍保留本事件；Windows 恢复策略需另外验收。
                return Dispatch::Full;
            }
            self.last_native_sequence = sequence;
            self.queue.push_back(key);
            return Dispatch::Queued;
        }
        self.last_native_sequence = sequence;
        self.deliver(key)
    }

    fn deliver(&mut self, key: Key) -> Dispatch {
        let result = self.engine.handle(InputEvent::Key(key));
        assert!(result.error.is_none());
        if let Some(request) = result.commit {
            self.pending = Some(CommitEnvelope {
                session: self.session,
                revision: result.frame.revision,
                request,
            });
        }
        if result.handled {
            Dispatch::Handled
        } else {
            Dispatch::Passed
        }
    }

    fn text(&mut self, text: &str) {
        for character in text.chars() {
            let sequence = self.last_native_sequence + 1;
            assert_eq!(
                self.native_key(sequence, false, Key::Char(character)),
                Dispatch::Handled
            );
        }
    }

    fn key(&mut self, key: Key) -> Dispatch {
        self.native_key(self.last_native_sequence + 1, false, key)
    }

    fn complete(&mut self, envelope: &CommitEnvelope, outcome: Outcome) -> Completion {
        let current = self.pending.as_ref().map(|pending| pending.request.id);
        if envelope.session != self.session || current != Some(envelope.request.id) {
            return Completion::Ignored;
        }
        assert_eq!(envelope.revision, self.engine.frame().revision);
        if matches!(outcome, Outcome::Unknown) {
            self.quarantined = true;
            return Completion::Quarantined;
        }
        self.quarantined = false;
        self.pending = None;
        match outcome {
            Outcome::Applied => {
                let result = self.engine.handle(InputEvent::CommitSucceeded {
                    id: envelope.request.id,
                });
                assert!(result.error.is_none());
                self.applied.push(envelope.request.text.clone());
                while self.pending.is_none() {
                    let Some(key) = self.queue.pop_front() else {
                        break;
                    };
                    self.deliver(key);
                }
                Completion::Applied
            }
            Outcome::NotApplied => {
                let result = self.engine.handle(InputEvent::CommitFailed {
                    id: envelope.request.id,
                });
                assert_eq!(result.error.unwrap().code, ErrorCode::CommitFailed);
                Completion::Failed
            }
            Outcome::Unknown => unreachable!(),
        }
    }

    fn retry(&mut self) {
        assert!(!self.quarantined && self.pending.is_none());
        let result = self.engine.handle(InputEvent::RetryCommit);
        assert!(result.error.is_none());
        self.pending = Some(CommitEnvelope {
            session: self.session,
            revision: result.frame.revision,
            request: result.commit.unwrap(),
        });
    }

    fn lose_focus(&mut self) {
        // 先使原生回调身份失效，再清核心；旧队列不迁移到另一个应用。
        self.session.epoch += 1;
        self.pending = None;
        self.quarantined = false;
        self.queue.clear();
        let result = self.engine.handle(InputEvent::FocusLost);
        assert!(result.error.is_none());
    }

    fn accepts_frame(&self, session: Session, frame: &InputFrame) -> bool {
        session == self.session && frame.revision == self.engine.frame().revision
    }
}

#[test]
fn native_probe_is_read_only_and_shortcuts_never_become_characters() {
    let mut host = HostModel::new(1);
    let before = host.engine.frame();
    assert!(host.probe(false, Key::Char('n')));
    assert!(!host.probe(true, Key::Char('c')));
    assert!(!host.probe(false, Key::Space));
    assert_eq!(host.engine.frame(), before);
    host.text("ni");
    let composing = host.engine.frame();
    assert!(host.probe(false, Key::Space));
    assert_eq!(
        host.native_key(host.last_native_sequence + 1, true, Key::Char('c')),
        Dispatch::Passed
    );
    assert_eq!(host.engine.frame(), composing);
}

#[test]
fn duplicate_native_callback_does_not_repeat_input_or_commit() {
    let mut host = HostModel::new(1);
    assert_eq!(host.native_key(1, false, Key::Char('n')), Dispatch::Handled);
    assert_eq!(
        host.native_key(1, false, Key::Char('n')),
        Dispatch::Duplicate
    );
    assert_eq!(host.engine.frame().preedit, "n");
    host.text("ihao");
    let sequence = host.last_native_sequence + 1;
    assert_eq!(
        host.native_key(sequence, false, Key::Space),
        Dispatch::Handled
    );
    let request = host.pending.clone().unwrap();
    assert_eq!(
        host.native_key(sequence, false, Key::Space),
        Dispatch::Duplicate
    );
    assert_eq!(
        host.complete(&request, Outcome::Applied),
        Completion::Applied
    );
    assert_eq!(
        host.complete(&request, Outcome::Applied),
        Completion::Ignored
    );
    assert_eq!(host.applied, ["你好"]);
}

#[test]
fn delayed_commit_serializes_owned_input_and_resumes_in_original_order() {
    let mut host = HostModel::new(1);
    host.text("nihao");
    assert_eq!(host.key(Key::Enter), Dispatch::Handled);
    let first = host.pending.clone().unwrap();
    assert_eq!(host.key(Key::Char('l')), Dispatch::Queued);
    assert_eq!(host.key(Key::Char('v')), Dispatch::Queued);
    assert_eq!(host.key(Key::Space), Dispatch::Queued);
    assert_eq!(host.engine.frame().preedit, "nihao");
    assert_eq!(host.complete(&first, Outcome::Applied), Completion::Applied);
    let second = host.pending.clone().unwrap();
    assert_eq!(second.request.text, "绿");
    assert_ne!(first.request.id, second.request.id);
    assert!(host.queue.is_empty());
    assert_eq!(
        host.complete(&second, Outcome::Applied),
        Completion::Applied
    );
    assert_eq!(host.applied, ["你好", "绿"]);
    assert_eq!(host.engine.frame().state, InputState::Idle);
}

#[test]
fn known_failure_retains_full_text_and_requires_explicit_new_request() {
    let mut host = HostModel::new(1);
    host.text("nihao");
    host.key(Key::Char(','));
    let first = host.pending.clone().unwrap();
    assert_eq!(first.request.text, "你好，");
    assert_eq!(
        host.complete(&first, Outcome::NotApplied),
        Completion::Failed
    );
    assert_eq!(host.engine.failed_commit().unwrap().text, "你好，");
    assert_eq!(host.key(Key::Char('n')), Dispatch::Suspended);
    assert!(host.pending.is_none());
    assert!(host.applied.is_empty());
    host.retry();
    let retried = host.pending.clone().unwrap();
    assert_ne!(first.request.id, retried.request.id);
    assert_eq!(first.request.text, retried.request.text);
    assert_eq!(
        host.complete(&retried, Outcome::Applied),
        Completion::Applied
    );
    assert_eq!(host.applied, ["你好，"]);
}

#[test]
fn unknown_outcome_does_not_become_retryable_failure_or_clear_new_context() {
    let mut host = HostModel::new(1);
    host.text("nihao");
    host.key(Key::Enter);
    let unknown = host.pending.clone().unwrap();
    assert_eq!(
        host.complete(&unknown, Outcome::Unknown),
        Completion::Quarantined
    );
    assert_eq!(host.engine.frame().state, InputState::Committing);
    assert!(host.engine.failed_commit().is_none());
    assert_eq!(host.key(Key::Space), Dispatch::Suspended);
    assert!(host.applied.is_empty());
    assert_eq!(
        host.pending.as_ref().unwrap().request.id,
        unknown.request.id
    );
    host.lose_focus();
    host.text("lv");
    let new_frame = host.engine.frame();
    assert_eq!(
        host.complete(&unknown, Outcome::Applied),
        Completion::Ignored
    );
    assert_eq!(host.engine.frame(), new_frame);
}

#[test]
fn old_instance_ack_is_rejected_even_when_request_ids_collide() {
    let mut old = HostModel::new(10);
    old.text("nihao");
    old.key(Key::Space);
    let old_request = old.pending.clone().unwrap();
    let mut current = HostModel::new(11);
    current.text("lv");
    current.key(Key::Space);
    let current_request = current.pending.clone().unwrap();
    assert_eq!(old_request.request.id, current_request.request.id);
    assert_eq!(
        current.complete(&old_request, Outcome::Applied),
        Completion::Ignored
    );
    assert_eq!(current.engine.frame().state, InputState::Committing);
    assert_eq!(
        current.complete(&current_request, Outcome::Applied),
        Completion::Applied
    );
    assert_eq!(current.applied, ["绿"]);
}

#[test]
fn queue_bound_preserves_owned_events_and_focus_cancellation_invalidates_frames() {
    let mut host = HostModel::new(1);
    host.text("ni");
    let old_session = host.session;
    let old_frame = host.engine.frame();
    assert!(host.accepts_frame(old_session, &old_frame));
    host.key(Key::Enter);
    let old_request = host.pending.clone().unwrap();
    for _ in 0..QUEUE_LIMIT {
        assert_eq!(host.key(Key::Char('a')), Dispatch::Queued);
    }
    assert_eq!(host.key(Key::Char('b')), Dispatch::Full);
    assert_eq!(host.queue.len(), QUEUE_LIMIT);
    assert!(host.queue.iter().all(|key| *key == Key::Char('a')));
    assert!(!host.accepts_frame(old_session, &old_frame));
    host.lose_focus();
    assert!(host.queue.is_empty());
    assert_eq!(host.engine.frame().state, InputState::Idle);
    assert!(!host.accepts_frame(old_session, &old_frame));
    assert_eq!(
        host.complete(&old_request, Outcome::Applied),
        Completion::Ignored
    );
}

#[test]
fn rejected_queue_event_keeps_its_sequence_until_caller_retries() {
    let mut host = HostModel::new(1);
    host.text("ni");
    host.key(Key::Enter);
    let request = host.pending.clone().unwrap();
    for _ in 0..QUEUE_LIMIT {
        assert_eq!(host.key(Key::Char('a')), Dispatch::Queued);
    }
    let refused_sequence = host.last_native_sequence + 1;
    assert_eq!(
        host.native_key(refused_sequence, false, Key::Char('b')),
        Dispatch::Full
    );
    assert_eq!(host.last_native_sequence + 1, refused_sequence);
    assert_eq!(
        host.complete(&request, Outcome::Applied),
        Completion::Applied
    );
    assert_eq!(host.engine.frame().preedit, "a".repeat(QUEUE_LIMIT));
    assert_eq!(
        host.native_key(refused_sequence, false, Key::Char('b')),
        Dispatch::Handled
    );
    assert_eq!(
        host.engine.frame().preedit,
        format!("{}b", "a".repeat(QUEUE_LIMIT))
    );
}

#[test]
fn utf16_offsets_and_submission_text_do_not_include_annotations() {
    let mut host = HostModel::new(1);
    host.text("cankao");
    let frame = host.engine.frame();
    assert!(frame.preedit.is_char_boundary(frame.cursor));
    assert_eq!(
        frame.preedit[..frame.cursor].encode_utf16().count(),
        frame.cursor
    );
    assert!(frame.candidates[0].annotation.is_some());
    host.key(Key::Space);
    assert_eq!(host.pending.as_ref().unwrap().request.text, "参考");
    assert_eq!("云🙂".chars().count(), 2);
    assert_eq!("云🙂".encode_utf16().count(), 3);
}
