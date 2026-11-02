use cloudleaf_offline::{MarianProvider, ModelState, import_catalog, inspect_catalog, sha256_file, uninstall_catalog};
use cloudleaf_translation::TranslationError;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cloudleaf-offline-test-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn catalog(&self, value: serde_json::Value) -> PathBuf {
        let path = self.0.join("catalog.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // 仅清理测试自己创建的唯一临时目录。
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn missing_catalog_is_not_installed_and_never_loads() {
    let fixture = Fixture::new();
    let path = fixture.0.join("missing.json");
    assert_eq!(inspect_catalog(&path).state, ModelState::NotInstalled);
    assert!(matches!(MarianProvider::load(&path), Err(TranslationError::ModelNotFound)));
}

#[test]
fn unknown_schema_empty_and_extra_fields_are_incompatible() {
    let fixture = Fixture::new();
    for value in [json!({"schema":2,"models":[]}), json!({"schema":1,"models":[]}),
        json!({"schema":1,"models":[],"download_url":"https://example.test"})] {
        let path = fixture.catalog(value);
        assert_eq!(inspect_catalog(&path).state, ModelState::Incompatible);
    }
}

#[test]
fn traversal_reference_is_rejected_before_import_and_original_is_kept() {
    let fixture = Fixture::new();
    let path = fixture.catalog(json!({"schema":1,"models":[{"manifest":"../outside.json","sha256":"0".repeat(64)}]}));
    let managed = fixture.0.join("managed");
    assert_eq!(import_catalog(&path, &managed), Err(TranslationError::InvalidConfig));
    assert!(path.is_file());
    assert!(!managed.exists());
}

#[test]
fn modified_manifest_hash_never_becomes_ready() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("manifest.json"), b"{}").unwrap();
    let path = fixture.catalog(json!({"schema":1,"models":[{"manifest":"manifest.json","sha256":"0".repeat(64)}]}));
    assert_eq!(inspect_catalog(&path).state, ModelState::Invalid);
}

#[test]
fn arbitrary_model_identity_and_incomplete_files_are_rejected() {
    let fixture = Fixture::new();
    let manifest = fixture.0.join("manifest.json");
    fs::write(&manifest, serde_json::to_vec(&json!({"schema":1,"model_id":"not-reviewed","direction":"en-zh",
        "source_url":"https://example.test","revision":"0".repeat(40),"architecture":"custom-code",
        "runtime":"candle-0.11.1-cpu","tokenizer":"sentencepiece-rs-0.2.2","license_id":"Apache-2.0",
        "license_status":"production","files":[]})).unwrap()).unwrap();
    let path = fixture.catalog(json!({"schema":1,"models":[{"manifest":"manifest.json","sha256":sha256_file(&manifest).unwrap()}]}));
    assert_eq!(inspect_catalog(&path).state, ModelState::Incompatible);
    assert!(matches!(MarianProvider::load(&path), Err(TranslationError::InvalidConfig)));
    assert_eq!(uninstall_catalog(&path, &fixture.0), Err(TranslationError::InvalidConfig));
    assert!(path.is_file());
    assert!(manifest.is_file());
}

#[test]
fn oversized_metadata_is_not_parsed() {
    let fixture = Fixture::new();
    let path = fixture.0.join("catalog.json");
    fs::write(&path, vec![b' ';65537]).unwrap();
    assert_eq!(inspect_catalog(&path).state, ModelState::Incompatible);
}
