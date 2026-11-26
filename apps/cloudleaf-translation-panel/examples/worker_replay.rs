use cloudleaf_translation::{
    OutputKind, QualityStatus, Secret, SecretStore, TranslationError, TranslationKind, WorkerStage,
};
use cloudleaf_translation_panel::controller::{PanelController, ViewStatus};
use cloudleaf_translation_panel::platform::{CredentialStore, PlatformError};
use cloudleaf_translation_panel::settings::{
    DirectionChoice, ProviderChoice, Settings, SettingsRepository,
};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

struct OfflineCredentials;

impl SecretStore for OfflineCredentials {
    fn get(&self, _: &str) -> Result<Secret, TranslationError> {
        panic!("离线面板不得读取凭据")
    }
}

impl CredentialStore for OfflineCredentials {
    fn set(&self, _: &str, _: Secret) -> Result<(), PlatformError> {
        panic!("离线面板不得写入凭据")
    }
    fn delete(&self, _: &str) -> Result<bool, PlatformError> {
        panic!("离线面板不得删除凭据")
    }
}

#[derive(Deserialize)]
struct Case {
    id: String,
    direction: DirectionChoice,
    source: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let [catalog, glossary, cases, directory]: [PathBuf; 4] = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "worker_replay CATALOG GLOSSARY CASES SETTINGS_DIRECTORY")?;
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(cases)?)?;
    assert!(!cases.is_empty());
    std::fs::create_dir(&directory)?;
    let settings = Settings {
        provider: ProviderChoice::OfflineModel,
        offline_catalog: Some(catalog),
        offline_glossary: Some(glossary),
        total_timeout_secs: 30,
        ..Settings::default()
    };
    SettingsRepository::new(directory.clone()).save(&settings)?;
    let mut panel = PanelController::new(directory.clone(), None, Arc::new(OfflineCredentials))?;
    wait(&mut panel, |panel| panel.status != ViewStatus::Preparing);
    assert_eq!(panel.status, ViewStatus::Idle, "{:?}", panel.error);
    assert!(panel.supports_kind(TranslationKind::Sentence));
    assert!(!panel.supports_auto());
    assert!(panel.worker_snapshot().is_some_and(|worker| {
        worker.stage == WorkerStage::NotStarted && worker.child_pid.is_none()
    }));
    println!("{}", json!({"event": "idle", "pid": null}));
    panel.set_kind(TranslationKind::Sentence);
    let mut pid = None;
    let mut failures = Vec::new();
    for case in &cases {
        panel.set_direction(case.direction);
        panel.set_text(case.source.clone());
        panel.start()?;
        wait_finished(&mut panel);
        if panel.status != ViewStatus::Completed {
            failures.push(case.id.as_str());
            println!(
                "{}",
                json!({"event": "failed", "id": case.id, "error": panel.error})
            );
            continue;
        }
        let result = panel.result().expect("已完成结果");
        let worker = panel.worker_snapshot().expect("工作进程观测");
        assert!(worker.matches_result(&case.source, result));
        assert!(worker.child_pid.is_some());
        assert_eq!(
            *pid.get_or_insert(worker.child_pid.unwrap()),
            worker.child_pid.unwrap()
        );
        let raw = panel.copy_text()?;
        assert_eq!(raw, result.output.translated_text);
        assert_eq!(result.output.output_kind, OutputKind::Translation);
        assert_eq!(result.output.quality, QualityStatus::Generated);
        assert_eq!(panel.text, case.source);
        let integrity = panel.integrity_report().expect("一致性检查");
        let findings: Vec<_> = integrity
            .findings
            .iter()
            .map(|finding| format!("{:?}:{:?}", finding.kind, finding.status))
            .collect();
        let metrics = worker.metrics.expect("已绑定推理指标");
        println!(
            "{}",
            json!({
                "event": "result", "id": case.id, "raw": raw, "pid": pid,
                "model": worker.model_identity, "findings": findings,
                "review": integrity.requires_acknowledgement(),
                "elapsed_ns": panel.elapsed.expect("请求耗时").as_nanos(),
                "threads": metrics.compute_threads, "load_ns": metrics.load_ns,
                "inference_ns": metrics.inference_ns,
            })
        );
    }
    assert!(pid.is_some());

    let source = cases.last().expect("回放样本").source.clone();
    panel.set_text(source.clone());
    panel.start()?;
    panel.cancel();
    assert_eq!(panel.status, ViewStatus::Cancelled);
    assert_eq!(panel.text, source);
    assert_eq!(panel.copy_text(), Err(TranslationError::StaleResult));
    assert!(panel.integrity_report().is_none());
    println!(
        "{}",
        json!({"event": "cancelled", "original_preserved": true})
    );

    panel.apply(Settings {
        offline_catalog: Some(directory.join("missing-catalog.json")),
        ..settings.clone()
    })?;
    wait(&mut panel, |panel| panel.status != ViewStatus::Preparing);
    assert_eq!(panel.status, ViewStatus::ProviderUnavailable);
    assert!(
        panel
            .error
            .as_ref()
            .is_some_and(|error| error.contains("NotInstalled"))
    );
    println!("{}", json!({"event": "missing", "error": panel.error}));

    let malformed = directory.join("corrupt-catalog.json");
    std::fs::write(&malformed, b"{")?;
    panel.apply(Settings {
        offline_catalog: Some(malformed),
        ..settings
    })?;
    wait(&mut panel, |panel| panel.status != ViewStatus::Preparing);
    assert_eq!(panel.status, ViewStatus::Idle, "{:?}", panel.error);
    panel.set_text("保留测试原文。".into());
    panel.start()?;
    wait_finished(&mut panel);
    assert_eq!(panel.status, ViewStatus::Failed, "{:?}", panel.error);
    assert_eq!(panel.copy_text(), Err(TranslationError::StaleResult));
    assert!(panel.integrity_report().is_none());
    let rejected = panel.worker_snapshot().expect("目录拒绝状态");
    assert_eq!(
        rejected.error,
        Some(TranslationError::OfflineWorker(
            cloudleaf_translation_ipc::IpcError::ModelInvalid
        ))
    );
    println!(
        "{}",
        json!({"event": "corrupt", "error": panel.error, "pid": rejected.child_pid})
    );

    panel.begin_close();
    wait(&mut panel, PanelController::close_ready);
    let stopped = panel.worker_snapshot().expect("工作进程退出状态");
    assert_eq!(stopped.stage, WorkerStage::Stopped);
    assert_eq!(stopped.child_pid, None);
    println!(
        "{}",
        json!({"event": "stopped", "pid": null, "previous_pid": pid})
    );
    assert!(failures.is_empty(), "回放失败: {failures:?}");
    Ok(())
}

fn wait_finished(panel: &mut PanelController) {
    wait(panel, |panel| {
        !matches!(panel.status, ViewStatus::Queued | ViewStatus::Running)
    });
}

fn wait(panel: &mut PanelController, condition: impl Fn(&PanelController) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        panel.tick();
        if condition(panel) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{:?} {:?} {:?}",
            panel.status,
            panel.error,
            panel.worker_snapshot()
        );
        thread::sleep(Duration::from_millis(2));
    }
}
