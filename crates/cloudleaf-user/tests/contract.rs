use cloudleaf_user::{SelectionEvent, SelectionOrigin, UserDictionary};

#[test]
fn manual_entries_roundtrip_and_learning_requires_trusted_current_event() {
    let mut dictionary = UserDictionary::new();
    assert!(!dictionary.learning_enabled());
    let id = dictionary.add("云笺", "yun jian").unwrap();
    dictionary.add("云笺", "yun qian").unwrap();
    let mut event = SelectionEvent {
        token: "session-1:commit-1".into(),
        origin: SelectionOrigin::IsolatedOwnedWindow,
        learning_epoch: dictionary.learning_epoch(),
        entries: vec![(id.clone(), cloudleaf_user::fingerprint("云笺", "yun jian"))],
    };
    assert!(!dictionary.record_selection(&event).unwrap());
    dictionary.set_learning_enabled(true).unwrap();
    assert!(!dictionary.record_selection(&event).unwrap());
    event.learning_epoch = dictionary.learning_epoch();
    assert!(dictionary.record_selection(&event).unwrap());
    assert!(!dictionary.record_selection(&event).unwrap());
    assert_eq!(
        UserDictionary::from_bytes(&dictionary.to_bytes()).unwrap(),
        dictionary
    );
    dictionary.remove(&id).unwrap();
    assert!(dictionary.get(&id).is_none());
}

#[test]
fn queued_old_user_selection_cannot_revive_deleted_or_changed_words() {
    for action in 0..5 {
        let mut dictionary = UserDictionary::new();
        let id = dictionary.add("云笺", "yun jian").unwrap();
        dictionary.set_learning_enabled(true).unwrap();
        let event = SelectionEvent {
            token: "session:old".into(),
            origin: SelectionOrigin::IsolatedOwnedWindow,
            learning_epoch: dictionary.learning_epoch(),
            entries: vec![(id.clone(), cloudleaf_user::fingerprint("云笺", "yun jian"))],
        };
        match action {
            0 => dictionary.remove(&id).unwrap(),
            1 => dictionary.update(&id, "测试", "ce shi").unwrap(),
            2 => dictionary.clear_entries().unwrap(),
            3 => dictionary.set_enabled(&id, false).unwrap(),
            _ => {
                dictionary.add("测试", "ce shi").unwrap();
            }
        }
        assert!(!dictionary.record_selection(&event).unwrap());
        assert!(dictionary.preferences().is_empty());
        let mut forged = event.clone();
        forged.token = "session:new".into();
        forged.learning_epoch = dictionary.learning_epoch();
        if action != 4 {
            assert!(!dictionary.record_selection(&forged).unwrap());
        }
    }
}
