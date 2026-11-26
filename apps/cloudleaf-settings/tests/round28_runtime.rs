use cloudleaf_settings::{
    InputPreferences, InputPreferencesRepository, InputScheme, InputSettingsError,
    InputSettingsStatus, RuntimeSettingsReason, RuntimeSettingsStatus, RuntimeSettingsWatch,
    read_runtime_receipts,
};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Profile(PathBuf);
impl Profile {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!("cloudleaf-r28-{}-{nonce}", std::process::id())))
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wait_for<T>(mut check: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(result) = check() {
            return result;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "后台设置状态超时");
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn writer(profile: &Path, page: &str) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_save_input_settings", "--nocapture"])
        .env("CLOUDLEAF_R28_SETTINGS_TEST_PROFILE", profile)
        .env("CLOUDLEAF_R28_SETTINGS_TEST_PAGE", page)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn child_save_input_settings() {
    let Some(profile) = std::env::var_os("CLOUDLEAF_R28_SETTINGS_TEST_PROFILE") else {
        return;
    };
    let preferences = InputPreferences {
        page_size: std::env::var("CLOUDLEAF_R28_SETTINGS_TEST_PAGE")
            .unwrap()
            .parse()
            .unwrap(),
        max_candidates: 12,
        scheme: InputScheme::Abbreviated,
        mixed_input: true,
        ..InputPreferences::default()
    };
    match InputPreferencesRepository::new(profile.into()).save(&preferences) {
        Ok(saved) => println!("saved_revision={}", saved.revision),
        Err(InputSettingsError::StaleRevision) => println!("stale_revision"),
        Err(error) => panic!("{error}"),
    }
}

#[test]
fn live_hosts_observe_cross_process_save_and_keep_independent_receipts() {
    let profile = Profile::new();
    let first = RuntimeSettingsWatch::start(profile.0.clone()).unwrap();
    let second = RuntimeSettingsWatch::start(profile.0.clone()).unwrap();
    for watcher in [&first, &second] {
        let initial = wait_for(|| watcher.poll()).unwrap();
        assert_eq!(initial.status, InputSettingsStatus::DefaultMissing);
    }
    let child = writer(&profile.0, "3").wait_with_output().unwrap();
    assert!(child.status.success());
    assert!(
        String::from_utf8(child.stdout)
            .unwrap()
            .contains("saved_revision=2")
    );
    let first_update = wait_for(|| first.poll()).unwrap();
    let second_update = wait_for(|| second.poll()).unwrap();
    assert_eq!(first_update, second_update);
    let active = first_update.settings;
    assert_eq!((active.revision, active.page_size), (2, 3));
    wait_for(|| {
        first
            .publish(
                2,
                &active,
                Some("test-dictionary"),
                None,
                RuntimeSettingsStatus::Applied,
                None,
            )
            .then_some(())
    });
    wait_for(|| {
        second
            .publish(
                1,
                &InputPreferences::default(),
                None,
                Some(2),
                RuntimeSettingsStatus::Deferred,
                Some(RuntimeSettingsReason::CompositionOrCommit),
            )
            .then_some(())
    });
    let receipts = wait_for(|| {
        let observed = read_runtime_receipts(&profile.0).unwrap();
        (observed.len() == 2).then_some(observed)
    });
    assert_ne!(receipts[0].host_id, receipts[1].host_id);
    assert!(receipts.iter().any(|receipt| {
        receipt.status == RuntimeSettingsStatus::Applied
            && receipt.active.revision == 2
            && receipt.generation == 2
            && receipt.dictionary_version.as_deref() == Some("test-dictionary")
    }));
    assert!(receipts.iter().any(|receipt| {
        receipt.status == RuntimeSettingsStatus::Deferred
            && receipt.active.revision == 1
            && receipt.pending_revision == Some(2)
    }));
    drop(second);
    wait_for(|| (read_runtime_receipts(&profile.0).unwrap().len() == 1).then_some(()));
    drop(first);
    wait_for(|| {
        read_runtime_receipts(&profile.0)
            .unwrap()
            .is_empty()
            .then_some(())
    });
}

#[test]
fn concurrent_writers_cannot_publish_different_values_at_the_same_revision() {
    let profile = Profile::new();
    let first = writer(&profile.0, "3");
    let second = writer(&profile.0, "4");
    let mut results = Vec::new();
    for child in [first, second] {
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        results.push(String::from_utf8(output.stdout).unwrap());
    }
    assert_eq!(
        results
            .iter()
            .filter(|output| output.contains("saved_revision=2"))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|output| output.contains("stale_revision"))
            .count(),
        1
    );
    assert_eq!(
        InputPreferencesRepository::new(profile.0.clone())
            .load()
            .unwrap()
            .settings
            .revision,
        2
    );
}
