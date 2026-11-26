use cloudleaf_input::{DoublePinyinLayout, FuzzyDirection, PinyinScheme};
use cloudleaf_settings::{
    InputFuzzyPair, InputPreferences, InputPreferencesRepository, InputScheme, InputSettingsError,
    InputSettingsStatus,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "cloudleaf-settings-r27-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn round27_profile_maps_all_effective_choices_and_rejects_stale_edits() {
    let fixture = Fixture::new();
    let repository = InputPreferencesRepository::new(fixture.0.clone());
    let initial = repository.load().unwrap();
    assert_eq!(initial.status, InputSettingsStatus::DefaultMissing);
    assert!(!initial.settings.mixed_input);
    let changed = InputPreferences {
        scheme: InputScheme::Xiaohe,
        page_size: 8,
        max_candidates: 12,
        mixed_input: true,
        fuzzy_pairs: vec![InputFuzzyPair::ZhZ, InputFuzzyPair::InIng],
        ..initial.settings
    };
    let saved = repository.save(&changed).unwrap();
    assert_eq!(saved.revision, changed.revision + 1);
    assert_eq!(
        saved.scheme_config().scheme,
        PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe)
    );
    assert!(
        saved
            .scheme_config()
            .fuzzy
            .iter()
            .all(|rule| rule.direction == FuzzyDirection::Both)
    );
    assert_eq!(saved.input_config().page_size, 8);
    assert_eq!(saved.input_config().max_candidates, 12);
    assert_eq!(repository.load().unwrap().settings, saved);
    assert_eq!(
        repository.save(&changed),
        Err(InputSettingsError::StaleRevision),
    );
    let illegal = InputPreferences {
        correction: true,
        ..saved.clone()
    };
    assert_eq!(
        repository.save(&illegal),
        Err(InputSettingsError::InvalidConfig),
    );
    assert_eq!(repository.load().unwrap().settings, saved);
}

#[test]
fn round27_corrupt_primary_falls_back_without_rewriting_and_future_format_is_protected() {
    let fixture = Fixture::new();
    let repository = InputPreferencesRepository::new(fixture.0.clone());
    let first = repository
        .save(&InputPreferences {
            scheme: InputScheme::Abbreviated,
            ..InputPreferences::default()
        })
        .unwrap();
    repository
        .save(&InputPreferences {
            scheme: InputScheme::Full,
            ..first.clone()
        })
        .unwrap();
    let primary = fixture.0.join("input-settings.json");
    let backup = fixture.0.join("input-settings.json.bak");
    let original_backup = fs::read(&backup).unwrap();
    fs::write(&primary, b"truncated").unwrap();
    let recovered = repository.load().unwrap();
    assert_eq!(recovered.status, InputSettingsStatus::Backup);
    assert_eq!(recovered.settings, first);
    assert_eq!(fs::read(&primary).unwrap(), b"truncated");
    repository.save(&recovered.settings).unwrap();
    assert_eq!(fs::read(&backup).unwrap(), original_backup);
    let mut future = first;
    future.schema_version = 8;
    let bytes = serde_json::to_vec(&future).unwrap();
    fs::write(&primary, &bytes).unwrap();
    assert_eq!(
        repository.load(),
        Err(InputSettingsError::UnsupportedVersion)
    );
    assert_eq!(
        repository.save(&recovered.settings),
        Err(InputSettingsError::UnsupportedVersion)
    );
    assert_eq!(fs::read(&primary).unwrap(), bytes);
}
