use cloudleaf_user::{
    ErrorKind, SaveFault, SelectionEvent, SelectionOrigin, Store, UserDictionary,
};
use ring::digest::{SHA256, digest};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "Cloudleaf-r19-storage-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn generation(&self, value: u64) -> PathBuf {
        self.0.join(format!("generation-{value:020}.clu"))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn example() -> UserDictionary {
    let mut d = UserDictionary::new();
    d.add("云笺", "yun jian").unwrap();
    d
}
fn rehash(body: &str) -> Vec<u8> {
    let hash: String = digest(&SHA256, body.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{body}sha256\t{hash}\n").into_bytes()
}
fn body(dictionary: &UserDictionary) -> String {
    String::from_utf8(dictionary.to_bytes())
        .unwrap()
        .rsplit_once("sha256\t")
        .unwrap()
        .0
        .into()
}

#[test]
fn real_crud_save_restart_import_export_and_private_backups() {
    let temp = Temp::new();
    let export = Temp::new();
    let mut store = Store::open(temp.0.clone()).unwrap();
    let mut d = store.load().unwrap();
    assert!(d.entries().is_empty());
    let id = d.add("云笺", "yun jian").unwrap();
    d = store.save(&d, 0).unwrap();
    assert_eq!(d.generation(), 1);
    d.update(&id, "云笺词", "yun jian ci").unwrap();
    d = store.save(&d, 1).unwrap();
    assert_eq!(d.get(&id).unwrap().revision, 2);
    assert!(!temp.generation(1).exists());
    d.set_enabled(&id, false).unwrap();
    d = store.save(&d, 2).unwrap();
    store.export(&d, &export.0.join("words.clu")).unwrap();
    assert!(store.export(&d, &temp.0.join("not-managed.clu")).is_err());
    drop(store);
    let mut store = Store::open(temp.0.clone()).unwrap();
    let mut d = store.load().unwrap();
    assert!(!d.get(&id).unwrap().enabled);
    assert_eq!(d.generation(), 3);
    let imported = store
        .import(&export.0.join("words.clu"), &UserDictionary::new())
        .unwrap();
    assert_eq!(imported.entries(), d.entries());
    assert!(!imported.learning_enabled());
    d.remove(&id).unwrap();
    let d = store.save(&d, 3).unwrap();
    assert_eq!(store.recovery_candidates().unwrap(), vec![4]);
    assert!(!temp.generation(3).exists());
    assert!(d.entries().is_empty());
    drop(store);
    let mut store = Store::open(temp.0.clone()).unwrap();
    let mut d = store.load().unwrap();
    assert!(d.entries().is_empty());
    assert_eq!(d.add("云笺", "yun jian").unwrap(), "user:2");
}

#[test]
fn save_faults_preserve_previous_and_unknown_requires_reload() {
    for fault in [
        SaveFault::BeforeWrite,
        SaveFault::PartialWrite,
        SaveFault::StorageFull,
        SaveFault::AfterSync,
        SaveFault::AfterPublish,
    ] {
        let temp = Temp::new();
        let mut store = Store::open(temp.0.clone()).unwrap();
        let original = store.save(&example(), 0).unwrap();
        let old_bytes = fs::read(temp.generation(1)).unwrap();
        let mut edited = original.clone();
        edited.add("测试", "ce shi").unwrap();
        let error = store.save_with_fault(&edited, 1, fault).unwrap_err();
        assert_eq!(
            error.kind,
            if fault == SaveFault::AfterPublish {
                ErrorKind::Unknown
            } else {
                ErrorKind::Io
            }
        );
        assert_eq!(fs::read(temp.generation(1)).unwrap(), old_bytes);
        assert_eq!(edited.entries().len(), 2);
        assert_eq!(original.entries().len(), 1);
        drop(store);
        let mut store = Store::open(temp.0.clone()).unwrap();
        let loaded = store.load().unwrap();
        if fault == SaveFault::AfterPublish {
            assert_eq!(loaded.generation(), 2);
            assert_eq!(loaded.entries().len(), 2);
            assert_eq!(
                store.save(&edited, 1).unwrap_err().kind,
                ErrorKind::Conflict
            );
        } else {
            assert_eq!(loaded, original);
            assert_eq!(store.pending_count().unwrap(), 1);
            assert_eq!(store.save(&edited, 1).unwrap().entries().len(), 2);
        }
    }
}

#[test]
fn interrupted_delete_finishes_private_history_purge_after_restart() {
    let temp = Temp::new();
    let mut store = Store::open(temp.0.clone()).unwrap();
    let mut d = store.save(&example(), 0).unwrap();
    d.set_learning_enabled(true).unwrap();
    d = store.save(&d, 1).unwrap();
    d.remove("user:1").unwrap();
    assert_eq!(
        store
            .save_with_fault(&d, 2, SaveFault::AfterPublish)
            .unwrap_err()
            .kind,
        ErrorKind::Unknown
    );
    assert!(temp.generation(2).exists());
    drop(store);
    let mut store = Store::open(temp.0.clone()).unwrap();
    let d = store.load().unwrap();
    assert!(d.entries().is_empty());
    assert_eq!(store.recovery_candidates().unwrap(), vec![3]);
    assert_eq!(store.pending_count().unwrap(), 0);
    assert!(!temp.generation(1).exists());
    assert!(!temp.generation(2).exists());
}

#[test]
fn first_partial_save_is_not_loaded_as_empty_without_explicit_reset() {
    let temp = Temp::new();
    let mut store = Store::open(temp.0.clone()).unwrap();
    let memory = example();
    let identity = memory.to_bytes();
    assert_eq!(
        store
            .save_with_fault(&memory, 0, SaveFault::StorageFull)
            .unwrap_err()
            .kind,
        ErrorKind::Io
    );
    assert_eq!(memory.to_bytes(), identity);
    drop(store);
    let mut store = Store::open(temp.0.clone()).unwrap();
    assert_eq!(store.load().unwrap_err().kind, ErrorKind::RecoveryRequired);
    assert_eq!(store.latest_generation().unwrap(), 0);
    assert!(store.clear_all(0).unwrap().entries().is_empty());
    assert_eq!(store.pending_count().unwrap(), 0);
}

#[test]
fn corrupted_latest_never_becomes_empty_and_recovery_preserves_originals() {
    let temp = Temp::new();
    let mut store = Store::open(temp.0.clone()).unwrap();
    let d = store.save(&example(), 0).unwrap();
    store.save(&d, 1).unwrap();
    let first = fs::read(temp.generation(1)).unwrap();
    let bad = b"invalid retained bytes";
    fs::write(temp.generation(2), bad).unwrap();
    assert_eq!(store.load().unwrap_err().kind, ErrorKind::RecoveryRequired);
    assert_eq!(
        store.save(&d, 1).unwrap_err().kind,
        ErrorKind::RecoveryRequired
    );
    assert_eq!(store.recovery_candidates().unwrap(), vec![1]);
    let recovered = store.recover(1).unwrap();
    assert_eq!(recovered.generation(), 3);
    assert_eq!(recovered.entries(), d.entries());
    assert_eq!(fs::read(temp.generation(1)).unwrap(), first);
    assert_eq!(fs::read(temp.generation(2)).unwrap(), bad);
}

#[test]
fn missing_empty_future_and_damaged_backup_have_explicit_outcomes() {
    let temp = Temp::new();
    let mut store = Store::open(temp.0.clone()).unwrap();
    assert!(store.load().unwrap().entries().is_empty());
    fs::write(temp.generation(1), []).unwrap();
    assert_eq!(store.load().unwrap_err().kind, ErrorKind::RecoveryRequired);
    assert!(store.recovery_candidates().unwrap().is_empty());
    fs::remove_file(temp.generation(1)).unwrap();
    let d = store.save(&example(), 0).unwrap();
    store.save(&d, 1).unwrap();
    fs::write(temp.generation(1), b"bad backup").unwrap();
    assert_eq!(store.load().unwrap().entries().len(), 1);
    fs::write(temp.generation(2), b"# cloudleaf-user-v99\n").unwrap();
    assert_eq!(store.load().unwrap_err().kind, ErrorKind::RecoveryRequired);
    assert!(store.recovery_candidates().unwrap().is_empty());
}

#[test]
fn explicit_clear_all_can_reset_corruption_but_stale_reset_and_valid_rollback_are_rejected() {
    let temp = Temp::new();
    let mut store = Store::open(temp.0.clone()).unwrap();
    fs::write(temp.generation(2), b"corrupt original").unwrap();
    assert_eq!(store.clear_all(1).unwrap_err().kind, ErrorKind::Conflict);
    assert_eq!(fs::read(temp.generation(2)).unwrap(), b"corrupt original");
    let empty = store.clear_all(2).unwrap();
    assert_eq!(empty.generation(), 3);
    assert!(!empty.learning_enabled());
    assert_eq!(store.recovery_candidates().unwrap(), vec![3]);
    let saved = store.save(&empty, 3).unwrap();
    assert_eq!(store.recover(3).unwrap_err().kind, ErrorKind::Conflict);
    assert_eq!(store.load().unwrap(), saved);
}

#[test]
fn migration_v1_is_explicit_and_preserves_original_file() {
    let temp = Temp::new();
    let mut store = Store::open(temp.0.clone()).unwrap();
    let old = rehash(
        "# cloudleaf-user-v1\ngeneration\t1\nnext_id\t2\nentries\t1\nid\ttext\tpinyin\nuser:1\t云笺\tyun jian\n",
    );
    fs::write(temp.generation(1), &old).unwrap();
    let d = store.load().unwrap();
    assert!(!d.learning_enabled());
    assert_eq!(d.get("user:1").unwrap().revision, 1);
    let d = store.save(&d, 1).unwrap();
    assert_eq!(d.generation(), 2);
    assert_eq!(fs::read(temp.generation(1)).unwrap(), old);
    assert!(
        fs::read(temp.generation(2))
            .unwrap()
            .starts_with(b"# cloudleaf-user-v2\n")
    );
}

#[test]
fn malformed_utf8_crlf_duplicate_ids_pairs_and_capacity_are_rejected() {
    let d = example();
    let original = body(&d);
    assert_eq!(
        UserDictionary::from_bytes(&[0xff]).unwrap_err().kind,
        ErrorKind::InvalidData
    );
    let crlf = original.replace("next_id\t2\n", "next_id\t2\r\n");
    assert_eq!(
        UserDictionary::from_bytes(&rehash(&crlf)).unwrap_err().kind,
        ErrorKind::InvalidData
    );
    let bad = original.replace("yun jian", "yun definitelywrong");
    assert!(UserDictionary::from_bytes(&rehash(&bad)).is_err());
    let duplicate = original.replace("entries\t1\n", "entries\t2\n").replace(
        "preferences\t0",
        "user:1\t1\t1\t测试\tce shi\npreferences\t0",
    );
    assert_eq!(
        UserDictionary::from_bytes(&rehash(&duplicate))
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    let duplicate = original
        .replace("next_id\t2", "next_id\t3")
        .replace("entries\t1\n", "entries\t2\n")
        .replace(
            "preferences\t0",
            "user:2\t1\t1\t云笺\tyun jian\npreferences\t0",
        );
    assert_eq!(
        UserDictionary::from_bytes(&rehash(&duplicate))
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    assert_eq!(
        UserDictionary::from_bytes(&vec![b'a'; cloudleaf_user::MAX_BYTES + 1])
            .unwrap_err()
            .kind,
        ErrorKind::Limit
    );
    let huge = original.replace(
        "entries\t1\n",
        &format!("entries\t{}\n", cloudleaf_user::MAX_ENTRIES + 1),
    );
    assert_eq!(
        UserDictionary::from_bytes(&rehash(&huge)).unwrap_err().kind,
        ErrorKind::Limit
    );
}

#[test]
fn changes_and_import_conflicts_are_atomic_and_do_not_enable_imported_learning() {
    let mut d = example();
    let original = d.clone();
    assert!(d.add("云笺", "yun jian").is_err());
    assert_eq!(d, original);
    assert!(d.update("user:1", "云笺", "wrong").is_err());
    assert_eq!(d, original);
    let mut incoming = original.clone();
    incoming.update("user:1", "测试", "ce shi").unwrap();
    incoming.set_learning_enabled(true).unwrap();
    assert_eq!(
        d.import_entries(&incoming.to_bytes()).unwrap_err().kind,
        ErrorKind::Conflict
    );
    assert_eq!(d, original);
    assert_eq!(d.import_entries(&original.to_bytes()).unwrap(), 0);
    let mut empty = UserDictionary::new();
    assert_eq!(empty.import_entries(&incoming.to_bytes()).unwrap(), 1);
    assert!(!empty.learning_enabled());
    let dbg = format!("{d:?} {:?}", d.get("user:1").unwrap());
    assert!(!dbg.contains("云笺"));
}

#[test]
fn learning_is_bounded_off_switch_epoch_and_clear_persist() {
    let temp = Temp::new();
    let mut store = Store::open(temp.0.clone()).unwrap();
    let mut d = example();
    d.set_learning_enabled(true).unwrap();
    let fingerprint = cloudleaf_user::fingerprint("云笺", "yun jian");
    let mut event = SelectionEvent {
        token: "test-session:1".into(),
        origin: SelectionOrigin::Simulation,
        learning_epoch: d.learning_epoch(),
        entries: vec![("user:1".into(), fingerprint.clone())],
    };
    assert_eq!(
        d.record_selection(&event).unwrap_err().kind,
        ErrorKind::Untrusted
    );
    event.origin = SelectionOrigin::IsolatedOwnedWindow;
    assert!(d.record_selection(&event).unwrap());
    assert!(!d.record_selection(&event).unwrap());
    assert_eq!(
        cloudleaf_user::choice_units(d.preference("user:1", &fingerprint)),
        0
    );
    event.token = "test-session:2".into();
    assert!(d.record_selection(&event).unwrap());
    assert_eq!(
        cloudleaf_user::choice_units(d.preference("user:1", &fingerprint)),
        1
    );
    d.set_learning_enabled(false).unwrap();
    event.token = "test-session:3".into();
    assert!(!d.record_selection(&event).unwrap());
    d.set_learning_enabled(true).unwrap();
    assert!(!d.record_selection(&event).unwrap());
    d.clear_learning().unwrap();
    assert_eq!(d.entries().len(), 1);
    assert_eq!(d.preference("user:1", &fingerprint), 0);
    let d = store.save(&d, 0).unwrap();
    drop(store);
    let mut store = Store::open(temp.0.clone()).unwrap();
    assert_eq!(store.load().unwrap(), d);
    assert_eq!(cloudleaf_user::choice_units(u32::MAX), 8);
}

#[test]
fn writer_lock_is_real_and_process_interruption_releases_it() {
    let temp = Temp::new();
    let mut store = Store::open(temp.0.clone()).unwrap();
    store.save(&example(), 0).unwrap();
    assert_eq!(
        Store::open(temp.0.clone()).unwrap_err().kind,
        ErrorKind::Busy
    );
    let exe = std::env::current_exe().unwrap();
    let output = Command::new(&exe)
        .args(["--exact", "store_child_phase", "--nocapture"])
        .env("CLOUDLEAF_STORE_TEST_ROOT", &temp.0)
        .env("CLOUDLEAF_STORE_TEST_ACTION", "busy")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(24));
    drop(store);
    let output = Command::new(&exe)
        .args(["--exact", "store_child_phase", "--nocapture"])
        .env("CLOUDLEAF_STORE_TEST_ROOT", &temp.0)
        .env("CLOUDLEAF_STORE_TEST_ACTION", "interrupt")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(23));
    let mut store = Store::open(temp.0.clone()).unwrap();
    let d = store.load().unwrap();
    assert_eq!(d.generation(), 1);
    assert_eq!(d.entries().len(), 1);
    assert_eq!(store.pending_count().unwrap(), 1);
    let empty = store.clear_all(1).unwrap();
    assert!(empty.entries().is_empty());
    assert!(!empty.learning_enabled());
    assert_eq!(store.recovery_candidates().unwrap(), vec![2]);
    assert_eq!(store.pending_count().unwrap(), 0);
}

#[test]
fn store_child_phase() {
    let Ok(path) = std::env::var("CLOUDLEAF_STORE_TEST_ROOT") else {
        return;
    };
    if std::env::var("CLOUDLEAF_STORE_TEST_ACTION").unwrap() == "busy" {
        assert_eq!(Store::open(path.into()).unwrap_err().kind, ErrorKind::Busy);
        std::process::exit(24);
    }
    let mut store = Store::open(path.into()).unwrap();
    let mut d = store.load().unwrap();
    d.add("测试", "ce shi").unwrap();
    assert!(store.save_with_fault(&d, 1, SaveFault::AfterSync).is_err());
    std::process::exit(23);
}

#[test]
fn relative_parent_and_git_paths_are_rejected_before_creation() {
    assert_eq!(
        Store::open("relative-folder".into()).unwrap_err().kind,
        ErrorKind::InvalidData
    );
    let temp = Temp::new();
    assert_eq!(
        Store::open(temp.0.join("..").join("bad")).unwrap_err().kind,
        ErrorKind::InvalidData
    );
    fs::write(temp.0.join(".git"), "private test marker").unwrap();
    assert_eq!(
        Store::open(temp.0.join("child")).unwrap_err().kind,
        ErrorKind::InvalidData
    );
    assert!(!temp.0.join("child").exists());
}
