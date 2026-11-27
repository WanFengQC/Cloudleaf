use super::*;
use cloudleaf_settings::{
    DictionaryArtifact, LanguageResourceRepository, ResourceAction, ResourceAdmission,
    ResourceReceipt, ResourceSlot, ResourceState,
};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

const BASE_SHA: &str = "03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431";
const ALTERNATE_SHA: &str = "1b13b199440c0ca9b59074de9100f58c6a0b847b12a7b986e1b752084ff60f8e";
const LAYER_SHA: &str = "edcb27080fac434637c83985eebd4c4f62d12e09943e4278a69fdc000b01e430";
const MODEL_SHA: &str = "36dc4d9057bc70e3494fe397da09e8ab6f187c435bd735b66a7734f9d414fce5";

struct OwnedChild(Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().is_ok_and(|status| status.is_none()) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn path(key: &str) -> PathBuf {
    std::env::var_os(key).map(PathBuf::from).expect(key)
}

fn dispatch(local: &LocalHost, key: Key, outcome: CommitOutcome) {
    local
        .process(InputEvent::Key(key), None, &|_| outcome)
        .unwrap();
}

fn signal(directory: &Path, name: &str) {
    std::fs::write(directory.join(name), []).unwrap();
}

fn host_child(directory: &Path, compose: bool) {
    let args = vec![
        path("CLOUDLEAF_R29_BASE").to_str().unwrap().to_owned(),
        BASE_SHA.into(),
        path("CLOUDLEAF_R29_MODEL").to_str().unwrap().to_owned(),
        MODEL_SHA.into(),
        "--settings-dir".into(),
        directory.to_str().unwrap().to_owned(),
    ];
    let opts = translation::LaunchOptions::parse(&args).unwrap();
    let (local, _) = configured_host(&opts).unwrap();
    let mut runtime = resources::Runtime::start(directory.to_owned(), &local).unwrap();
    let deadline = Instant::now() + Duration::from_secs(70);
    let mut composing = None;
    let mut unknown = false;
    while !directory.join("stop").exists() {
        if compose && composing.is_none() && directory.join("compose").exists() {
            for key in "nihao".chars().map(Key::Char).chain([Key::Left]) {
                dispatch(&local, key, CommitOutcome::Applied);
            }
            composing = Some(local.host.engine.borrow().frame());
            signal(directory, "composing");
        }
        if composing.is_some() && !unknown && directory.join("enter-unknown").exists() {
            dispatch(&local, Key::Space, CommitOutcome::Unknown);
            assert!(local.host.quarantined.get());
            assert!(local.retained.borrow().contains("nihao"));
            assert!(local.document.borrow().is_empty());
            unknown = true;
            signal(directory, "unknown");
        }
        if unknown && directory.join("resume").exists() {
            local.reset().unwrap();
            assert!(local.document.borrow().is_empty());
            composing = None;
            unknown = false;
            std::fs::remove_file(directory.join("compose")).unwrap();
        }
        runtime.receive();
        apply_resources(&local, &mut runtime, None);
        runtime.publish(&local, None);
        if let Some(frame) = &composing
            && !unknown
        {
            assert_eq!(&local.host.engine.borrow().frame(), frame);
            assert_eq!(&local.host.projection.borrow().frame(), frame);
        }
        assert!(local.document.borrow().is_empty());
        assert!(Instant::now() < deadline, "资源宿主等待中断");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_receipts(
    repository: &LanguageResourceRepository,
    predicate: impl Fn(&[ResourceReceipt]) -> bool,
) -> Vec<ResourceReceipt> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let receipts = repository.receipts().unwrap();
        if receipts.len() == 2 && predicate(&receipts) {
            return receipts;
        }
        assert!(Instant::now() < deadline, "资源回执等待超时：{receipts:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "需要已经绑定 SHA 的真实 CC-CEDICT、Unihan、ResearchOnly 补充及 lexical 文件"]
fn real_dictionary_lifecycle_two_owned_processes() {
    if let Some(mode) = std::env::var_os("CLOUDLEAF_R29_CHILD") {
        host_child(&path("CLOUDLEAF_R29_HOST_DIR"), mode == "compose");
        return;
    }
    let directory = std::env::temp_dir().join(format!(
        "cloudleaf-r29-hosts-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let dir = directory.canonicalize().unwrap();
    let layer = dir.join("supplement.tsv");
    std::fs::copy(path("CLOUDLEAF_R29_SUPPLEMENT"), &layer).unwrap();
    let repo = LanguageResourceRepository::new(dir.clone());
    let executable = std::env::current_exe().unwrap();
    let mut children = ["compose", "passive"].map(|mode| {
        OwnedChild(
            Command::new(&executable)
                .args([
                    "--ignored",
                    "real_dictionary_lifecycle_two_owned_processes",
                    "--nocapture",
                ])
                .env("CLOUDLEAF_R29_CHILD", mode)
                .env("CLOUDLEAF_R29_HOST_DIR", &dir)
                .stdin(Stdio::null())
                .spawn()
                .unwrap(),
        )
    });
    let applied = |revision, base, supplement, model| {
        let receipts = wait_receipts(&repo, |rows| {
            rows.iter()
                .all(|r| r.active_revision == revision && r.state == ResourceState::Applied)
        });
        for r in &receipts {
            assert_eq!(r.active.base.runtime_sha256, base);
            assert_eq!(r.active.model_sha256.as_deref(), model);
            assert_eq!(
                r.active
                    .supplement
                    .as_ref()
                    .map(|s| s.runtime_sha256.as_str()),
                supplement
            );
            if let Some(layer) = &r.active.supplement {
                assert_eq!(layer.admission, ResourceAdmission::ResearchOnly);
            }
        }
        assert_ne!(receipts[0].process_id, receipts[1].process_id);
        assert_eq!(
            receipts[0].active.snapshot_sha256,
            receipts[1].active.snapshot_sha256
        );
        println!(
            "资源修订 {revision} 两宿主收敛，快照 {}",
            receipts[0].active.snapshot_sha256
        );
    };
    applied(1, BASE_SHA, None, Some(MODEL_SHA));
    let select = |slot, file, sha: &str| {
        ResourceAction::Select(slot, DictionaryArtifact::new(file, sha.into()).unwrap())
    };
    let save = |action| {
        repo.save(repo.load().unwrap().revision, action)
            .unwrap()
            .revision
    };
    let selected = save(select(ResourceSlot::Supplement, layer.clone(), LAYER_SHA));
    applied(selected, BASE_SHA, None, Some(MODEL_SHA));
    signal(&dir, "compose");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !dir.join("composing").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let enabled = save(ResourceAction::SetSupplementEnabled(true));
    let deferred = wait_receipts(&repo, |rows| {
        rows.iter()
            .any(|r| r.state == ResourceState::Deferred && r.desired_revision == enabled)
            && rows
                .iter()
                .any(|r| r.active_revision == enabled && r.state == ResourceState::Applied)
    });
    assert!(deferred.iter().any(|r| r.active_revision == selected));
    signal(&dir, "enter-unknown");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !dir.join("unknown").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    signal(&dir, "resume");
    applied(enabled, BASE_SHA, Some(LAYER_SHA), None);
    let alternate = save(select(
        ResourceSlot::Base,
        path("CLOUDLEAF_R29_ALTERNATE"),
        ALTERNATE_SHA,
    ));
    applied(alternate, ALTERNATE_SHA, Some(LAYER_SHA), None);
    let backed = save(ResourceAction::Rollback(ResourceSlot::Base));
    applied(backed, BASE_SHA, Some(LAYER_SHA), None);
    let disabled = save(ResourceAction::SetSupplementEnabled(false));
    applied(disabled, BASE_SHA, None, Some(MODEL_SHA));

    // 已启用、已验证的文件在磁盘随后损坏，各宿主保留内存旧快照，独立回退。
    let activated = save(ResourceAction::SetSupplementEnabled(true));
    applied(activated, BASE_SHA, Some(LAYER_SHA), None);
    std::fs::write(&layer, b"broken").unwrap();
    let requested = save(select(
        ResourceSlot::Base,
        path("CLOUDLEAF_R29_ALTERNATE"),
        ALTERNATE_SHA,
    ));
    let rejected = wait_receipts(&repo, |rows| {
        rows.iter()
            .all(|r| r.desired_revision == requested && r.state == ResourceState::Rejected)
    });
    assert!(rejected.iter().all(|r| r.active_revision == activated
        && r.active.base.runtime_sha256 == BASE_SHA
        && r.active.supplement.as_ref().unwrap().runtime_sha256 == LAYER_SHA));
    let reverted = save(ResourceAction::Rollback(ResourceSlot::Supplement));
    applied(reverted, ALTERNATE_SHA, None, None);
    let restored = save(ResourceAction::Rollback(ResourceSlot::Base));
    applied(restored, BASE_SHA, None, Some(MODEL_SHA));
    signal(&dir, "stop");
    for child in &mut children {
        assert!(child.0.wait().unwrap().success());
    }
    std::fs::remove_dir_all(directory).unwrap();
}
