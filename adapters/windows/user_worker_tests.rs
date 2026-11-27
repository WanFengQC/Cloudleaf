//! 实际worker/磁盘/候选链路的逻辑回归；没有HWND，不构成真实Applied或系统验收。
use super::*;
use cloudleaf_input::{InputConfig, InputEngine};
use cloudleaf_types::{InputEvent, Key};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const BASE: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\nbase-yun\tyun\t云\t\tunknown\t\tfixture\tbase-yun\tMIT\nbase-jian\tjian\t简\t\tunknown\t\tfixture\tbase-jian\tMIT\n";
const BASE_SHA: &str = "53af9758907064091743f0cd8499875fad9ea12ffaea023d1f93caa691c1aa44";

fn receive(worker: &Worker) -> Reply {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(reply) = worker.poll() {
            return reply;
        }
        assert!(Instant::now() < deadline, "有界worker没有及时返回结果");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn ready(worker: &Worker) -> Prepared {
    match receive(worker) {
        Reply::Ready(value) => value,
        Reply::Error(message) => panic!("实际worker拒绝测试任务：{message}"),
        Reply::Rebased(_, _) => panic!("预期用户词库操作，收到词典重绑结果"),
    }
}

#[test]
fn actual_worker_persists_user_candidates_and_gates_learning_across_restart() {
    let temp = std::env::temp_dir().canonicalize().unwrap();
    let directory = temp.join(format!(
        "cloudleaf-r19-worker-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let base = Arc::new(SearchSnapshot::from_verified_tsv(BASE, BASE_SHA).unwrap());
    let mut worker = Worker::start(base.clone(), None, directory.clone());
    let initial = ready(&worker);
    assert!(!initial.learning);
    assert_eq!(initial.generation, 0);
    drop(initial);

    worker
        .send(Work::Manage(user_panel::Command::Add {
            text: "云笺".into(),
            pinyin: "yun jian".into(),
        }))
        .unwrap();
    let added = ready(&worker);
    assert!(
        added
            .snapshot
            .entries()
            .iter()
            .any(|e| e.source == "user" && e.text == "云笺")
    );
    assert!(
        added
            .listing
            .contains(&format!("generation={}；", added.generation))
    );
    drop(added);
    worker
        .send(Work::Manage(user_panel::Command::Save))
        .unwrap();
    let saved = ready(&worker);
    assert_eq!(saved.generation, 2);
    drop(saved);

    worker
        .send(Work::Manage(user_panel::Command::ToggleLearning))
        .unwrap();
    let active = ready(&worker);
    assert!(active.learning);
    let generation = active.generation;
    let mut engine = InputEngine::with_search(
        active.snapshot.clone(),
        cloudleaf_search::Config {
            output_k: 10,
            ..Default::default()
        },
        InputConfig::default(),
    )
    .unwrap();
    engine.handle(InputEvent::Activate);
    for key in "yunjian".chars() {
        engine.handle(InputEvent::Key(Key::Char(key)));
    }
    let frame = engine.frame();
    let chosen = frame.candidates.iter().find(|c| c.text == "云笺").unwrap();
    let request = engine
        .select_candidate(frame.revision, &chosen.id)
        .commit
        .unwrap();
    let evidence = engine.pending_selection().unwrap().clone();
    assert_eq!(request.id, evidence.commit_id);
    assert!(evidence.entries.iter().any(|(id, _)| id == "user:1"));

    // 此测试显式检查来源标签的逻辑，不把构造标签冒充窗口写入确认。
    let tagged = learning_event(&evidence, "worker-fixture", active.epoch);
    let mut simulated = tagged.clone();
    simulated.origin = SelectionOrigin::Simulation;
    worker
        .send(Work::Learn {
            event: simulated,
            generation,
        })
        .unwrap();
    assert!(matches!(receive(&worker), Reply::Error(_)));
    worker
        .send(Work::Learn {
            event: tagged.clone(),
            generation,
        })
        .unwrap();
    let learned = ready(&worker);
    assert!(learned.listing.contains("选择=1"));
    let learned_generation = learned.generation;
    let restart_version = learned.snapshot.version().to_owned();
    drop(active);
    drop(learned);

    worker
        .send(Work::Learn {
            event: tagged.clone(),
            generation: learned_generation,
        })
        .unwrap();
    assert!(
        matches!(receive(&worker), Reply::Error(_)),
        "重复确认不能重复学习"
    );
    let mut later = tagged.clone();
    later.token = "worker-fixture:later".into();
    worker
        .send(Work::Learn {
            event: later,
            generation,
        })
        .unwrap();
    assert!(
        matches!(receive(&worker), Reply::Error(_)),
        "旧词库generation不能学习"
    );
    worker.shutdown();

    let mut worker = Worker::start(base, None, directory.clone());
    let restored = ready(&worker);
    assert_eq!(restored.snapshot.version(), restart_version);
    assert!(restored.learning && restored.listing.contains("选择=1"));
    assert!(
        restored
            .snapshot
            .entries()
            .iter()
            .any(|e| e.id == "user:1" && e.text == "云笺")
    );
    drop(restored);
    worker
        .send(Work::Manage(user_panel::Command::ToggleLearning))
        .unwrap();
    let disabled = ready(&worker);
    assert!(!disabled.learning);
    let disabled_generation = disabled.generation;
    let mut off_event = tagged;
    off_event.token = "worker-fixture:off".into();
    off_event.learning_epoch = disabled.epoch;
    drop(disabled);
    worker
        .send(Work::Learn {
            event: off_event,
            generation: disabled_generation,
        })
        .unwrap();
    assert!(matches!(receive(&worker), Reply::Error(_)));
    worker
        .send(Work::Manage(user_panel::Command::Delete {
            id: "user:1".into(),
        }))
        .unwrap();
    let deleted = ready(&worker);
    assert!(!deleted.snapshot.entries().iter().any(|e| e.id == "user:1"));
    assert!(!deleted.learning);
    drop(deleted);
    worker.shutdown();

    let mut store = Store::open(directory.clone()).unwrap();
    let disk = store.load().unwrap();
    assert!(disk.entries().is_empty());
    assert!(disk.preferences().is_empty());
    assert!(!disk.learning_enabled());
    drop(store);
    assert_eq!(directory.parent().unwrap().canonicalize().unwrap(), temp);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn full_worker_queues_refuse_more_work_and_shutdown_releases_the_actual_store_lock() {
    let temp = std::env::temp_dir().canonicalize().unwrap();
    let directory = temp.join(format!(
        "cloudleaf-r20-backpressure-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let base = Arc::new(SearchSnapshot::from_verified_tsv(BASE, BASE_SHA).unwrap());
    let mut worker = Worker::start(base, None, directory.clone());
    drop(ready(&worker));
    let mut accepted = 0;
    let mut refused = false;
    // 不消费真实结果队列，worker最终在有界回传处等待；派发仍只能try_send。
    for _ in 0..64 {
        match worker.send(Work::Manage(user_panel::Command::Save)) {
            Ok(()) => accepted += 1,
            Err(_) => {
                refused = true;
                break;
            }
        }
    }
    assert!(refused && accepted > 0);
    worker.shutdown();
    let mut store = Store::open(directory.clone()).unwrap();
    assert_eq!(store.load().unwrap().generation(), accepted);
    drop(store);
    assert_eq!(directory.parent().unwrap().canonicalize().unwrap(), temp);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn invalid_import_preserves_the_saved_user_layer_and_worker_remains_usable() {
    let temp = std::env::temp_dir().canonicalize().unwrap();
    let directory = temp.join(format!(
        "cloudleaf-r20-import-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let invalid = directory.join("own-invalid-fixture.tsv");
    std::fs::write(&invalid, b"invalid package").unwrap();
    let base = Arc::new(SearchSnapshot::from_verified_tsv(BASE, BASE_SHA).unwrap());
    let mut worker = Worker::start(base, None, directory.clone());
    drop(ready(&worker));
    worker
        .send(Work::Manage(user_panel::Command::Add {
            text: "云笺".into(),
            pinyin: "yun jian".into(),
        }))
        .unwrap();
    let saved = ready(&worker);
    let generation = saved.generation;
    let version = saved.snapshot.version().to_owned();
    drop(saved);
    worker
        .send(Work::Manage(user_panel::Command::Import {
            path: invalid.to_string_lossy().into_owned(),
        }))
        .unwrap();
    assert!(matches!(receive(&worker), Reply::Error(_)));
    worker
        .send(Work::Manage(user_panel::Command::Reload))
        .unwrap();
    let reloaded = ready(&worker);
    assert_eq!(reloaded.generation, generation);
    assert_eq!(reloaded.snapshot.version(), version);
    assert!(
        reloaded
            .snapshot
            .entries()
            .iter()
            .any(|entry| entry.id == "user:1")
    );
    drop(reloaded);
    worker.shutdown();
    let mut store = Store::open(directory.clone()).unwrap();
    assert_eq!(store.load().unwrap().generation(), generation);
    drop(store);
    assert_eq!(directory.parent().unwrap().canonicalize().unwrap(), temp);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn rebase_preserves_user_generation_preference_and_private_store() {
    let temp = std::env::temp_dir().canonicalize().unwrap();
    let directory = temp.join(format!(
        "cloudleaf-r29-user-rebase-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let original = Arc::new(SearchSnapshot::from_verified_tsv(BASE, BASE_SHA).unwrap());
    let updated =
        format!("{BASE}base-xuexi\txue xi\t学习\t\tunknown\t\tfixture\tbase-xuexi\tMIT\n");
    let replaced = Arc::new(
        SearchSnapshot::from_verified_tsv(
            &updated,
            "f3d3b4648b06538e89cb90ea06d856db1dcdc590f640b78848f65af388bd2373",
        )
        .unwrap(),
    );
    let mut worker = Worker::start(original.clone(), None, directory.clone());
    drop(ready(&worker));
    worker
        .send(Work::Manage(user_panel::Command::Add {
            text: "云笺".into(),
            pinyin: "yun jian".into(),
        }))
        .unwrap();
    drop(ready(&worker));
    worker
        .send(Work::Manage(user_panel::Command::ToggleLearning))
        .unwrap();
    let learning = ready(&worker);
    let selected = cloudleaf_user::fingerprint("云笺", "yun jian");
    worker
        .send(Work::Learn {
            event: SelectionEvent {
                token: "r29-owned:1".into(),
                origin: SelectionOrigin::IsolatedOwnedWindow,
                learning_epoch: learning.epoch,
                entries: vec![("user:1".into(), selected.clone())],
            },
            generation: learning.generation,
        })
        .unwrap();
    drop(learning);
    let learned = ready(&worker);
    let expected = learned.generation;
    assert!(learned.listing.contains("选择=1"));
    worker
        .send(Work::PrepareRebase {
            revision: 29,
            base: replaced.clone(),
            language: None,
        })
        .unwrap();
    let Reply::Rebased(29, Ok(staged)) = receive(&worker) else {
        panic!("用户层未能为新资源重新准备");
    };
    assert_eq!(staged.generation, expected);
    assert_eq!(staged.epoch, learned.epoch);
    assert_ne!(staged.snapshot.version(), learned.snapshot.version());
    assert_eq!(staged.listing, learned.listing);
    worker
        .send(Work::AcceptRebase {
            base: replaced,
            language: None,
        })
        .unwrap();
    worker
        .send(Work::Manage(user_panel::Command::Reload))
        .unwrap();
    let current = ready(&worker);
    assert_eq!(current.snapshot.version(), staged.snapshot.version());
    assert_eq!(current.generation, expected);
    worker.shutdown();
    let mut store = Store::open(directory.clone()).unwrap();
    let disk = store.load().unwrap();
    assert_eq!(disk.generation(), expected);
    assert_eq!(disk.preference("user:1", &selected), 1);
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}
