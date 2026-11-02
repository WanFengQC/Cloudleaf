use cloudleaf_translation_panel::settings::{ProviderChoice, Settings, SettingsRepository};
use std::path::PathBuf;

#[test]
fn schema2_migration_adds_no_model_and_preserves_offline_start() {
    let directory =
        std::env::temp_dir().join(format!("cloudleaf-r13-migration-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut value = serde_json::to_value(Settings::default()).unwrap();
    value["schema_version"] = 2.into();
    value["network_enabled"] = true.into();
    value.as_object_mut().unwrap().remove("offline_catalog");
    std::fs::write(
        directory.join("settings.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    let loaded = SettingsRepository::new(directory.clone()).load().unwrap();
    assert_eq!(loaded.settings.schema_version, 3);
    assert!(loaded.settings.offline_catalog.is_none());
    assert!(!loaded.settings.network_enabled);
    assert!(loaded.notice.is_some());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn model_catalog_is_distinct_from_online_model_and_dictionary() {
    let settings = Settings {
        provider: ProviderChoice::OfflineModel,
        offline_catalog: Some(PathBuf::from("models/catalog.json")),
        dictionary_path: Some(PathBuf::from("dictionary/catalog.json")),
        model: "online-model-kept".into(),
        ..Settings::default()
    };
    settings.validate().unwrap();
    let restored: Settings =
        serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
    assert_eq!(restored, settings);
    assert!(!restored.network_enabled);
    assert!(!format!("{restored:?}").contains("models/catalog"));
    let mut invalid = settings;
    invalid.offline_catalog = Some(PathBuf::new());
    assert!(invalid.validate().is_err());
}
