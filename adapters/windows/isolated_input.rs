use super::*;
use std::cell::Cell;

#[cfg(test)]
mod tests {
    use super::*;
    fn type_text(local: &LocalHost, value: &str) {
        for c in value.chars() {
            local
                .process(InputEvent::Key(Key::Char(c)), None, &|_| {
                    CommitOutcome::Applied
                })
                .unwrap();
        }
    }
    #[test]
    fn actual_host_controller_commit_once_stale_selection_and_original_fallback() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        assert!(local.host.context.borrow().is_none() && local.host.manager.borrow().is_none());
        type_text(&local, "nihao");
        let old = local.host.engine.borrow().frame();
        let writes = Cell::new(0);
        let sink = |_: &str| {
            writes.set(writes.get() + 1);
            CommitOutcome::Applied
        };
        local
            .process(
                InputEvent::Key(Key::Space),
                Some((old.revision, old.candidates[0].id.clone())),
                &sink,
            )
            .unwrap();
        assert_eq!(local.document.borrow().as_str(), "你好");
        assert_eq!(writes.get(), 1);
        assert!(
            local
                .process(
                    InputEvent::Key(Key::Space),
                    Some((old.revision, old.candidates[0].id.clone())),
                    &sink
                )
                .is_err()
        );
        assert_eq!(writes.get(), 1);
        type_text(&local, "qz");
        local
            .process(InputEvent::Key(Key::Enter), None, &sink)
            .unwrap();
        assert_eq!(local.document.borrow().as_str(), "你好qz");
    }
    #[test]
    fn unknown_blocks_replay_and_explicit_new_epoch_rejects_old_candidate() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        type_text(&local, "nihao");
        let old = local.host.engine.borrow().frame();
        let epoch = local.host.epoch.get();
        let writes = Cell::new(0);
        let sink = |_: &str| {
            writes.set(writes.get() + 1);
            CommitOutcome::Unknown
        };
        local
            .process(InputEvent::Key(Key::Space), None, &sink)
            .unwrap();
        assert_eq!(writes.get(), 1);
        assert!(local.host.quarantined.get());
        assert_eq!(
            local.host.engine.borrow().frame().state,
            cloudleaf_types::InputState::Committing
        );
        assert!(
            local
                .process(InputEvent::Key(Key::Space), None, &sink)
                .is_err()
        );
        assert!(local.process(InputEvent::FocusLost, None, &sink).is_err());
        assert_eq!(writes.get(), 1);
        assert!(local.retained.borrow().contains("nihao"));
        local.reset().unwrap();
        assert!(local.host.epoch.get() > epoch);
        assert!(
            local
                .process(
                    InputEvent::Key(Key::Space),
                    Some((old.revision, old.candidates[0].id.clone())),
                    &sink
                )
                .is_err()
        );
        assert_eq!(writes.get(), 1);
        assert!(!local.host.quarantined.get());
    }

    #[test]
    fn not_applied_keeps_original_and_a_later_explicit_selection_can_commit_once() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        type_text(&local, "nihao");
        let writes = Cell::new(0);
        let (frame, _) = local
            .process(InputEvent::Key(Key::Space), None, &|_| {
                writes.set(writes.get() + 1);
                CommitOutcome::NotApplied
            })
            .unwrap();
        assert_eq!(frame.preedit, "nihao");
        assert!(local.document.borrow().is_empty());
        assert!(!local.host.quarantined.get());
        assert_eq!(writes.get(), 1);
        local
            .process(InputEvent::RetryCommit, None, &|_| {
                writes.set(writes.get() + 1);
                CommitOutcome::Applied
            })
            .unwrap();
        assert_eq!(local.document.borrow().as_str(), "你好");
        assert_eq!(writes.get(), 2);
    }
}
