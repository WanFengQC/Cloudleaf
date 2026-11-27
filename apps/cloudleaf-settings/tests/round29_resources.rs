use cloudleaf_input::SearchSnapshot;
use cloudleaf_settings::{
    DictionaryArtifact, LanguageResourceIdentity, LanguageResourceManager,
    LanguageResourceRepository, ResourceAction, ResourceAdmission, ResourceError, ResourceReceipt,
    ResourceSlot, ResourceState,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const BASE: &str = "03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431";
const UNIHAN: &str = "1b13b199440c0ca9b59074de9100f58c6a0b847b12a7b986e1b752084ff60f8e";
const SUPPLEMENT: &str = "edcb27080fac434637c83985eebd4c4f62d12e09943e4278a69fdc000b01e430";
const HEADER: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("cloudleaf-r29-res-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn artifact(&self, filename: &str, sha256: &str) -> DictionaryArtifact {
        DictionaryArtifact::new(self.0.join(filename), sha256.to_owned()).unwrap()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn slots_reject_cross_role_and_missing_registered_artifacts() {
    let root = Temp::new();
    let repo = LanguageResourceRepository::new(root.0.clone());
    let base = root.artifact("base.tsv", BASE);
    let supplement = root.artifact("supplement.tsv", SUPPLEMENT);
    let original = repo.load().unwrap();
    assert!(!original.supplement_enabled);
    assert_eq!(
        repo.save(
            original.revision,
            ResourceAction::Select(ResourceSlot::Base, supplement.clone())
        ),
        Err(ResourceError::Unapproved)
    );
    assert_eq!(
        repo.save(
            original.revision,
            ResourceAction::Select(ResourceSlot::Supplement, base.clone())
        ),
        Err(ResourceError::Unapproved)
    );
    assert_eq!(
        repo.save(
            original.revision,
            ResourceAction::Select(ResourceSlot::Base, base)
        ),
        Err(ResourceError::Io)
    );
    assert_eq!(
        repo.save(
            original.revision,
            ResourceAction::SetSupplementEnabled(true)
        ),
        Err(ResourceError::InvalidSettings)
    );
    assert_eq!(repo.load().unwrap(), original);
}

#[test]
fn signed_off_identity_does_not_admit_wrong_bytes_and_receipts_keep_two_live_hosts() {
    let root = Temp::new();
    let repo = LanguageResourceRepository::new(root.0.clone());
    let bad = root.artifact("tampered.tsv", BASE);
    std::fs::write(bad.path(), HEADER).unwrap();
    let mut config = repo.load().unwrap();
    assert_eq!(
        repo.save(1, ResourceAction::Select(ResourceSlot::Base, bad.clone())),
        Err(ResourceError::VerifyResource)
    );
    assert_eq!(repo.load().unwrap(), config);
    config.revision = 2;
    config.base = Some(bad);
    let startup = Arc::new(
        SearchSnapshot::from_verified_tsv(
            HEADER,
            "588b52a863ea4a9428689ddab8cb8c5eaba12c25c3c6dd765a7bf6ee85ea7719",
        )
        .unwrap(),
    );
    let manager = LanguageResourceManager;
    assert!(matches!(
        manager.prepare(&config, &startup, None),
        Err(ResourceError::VerifyResource)
    ));
    let mut receipt = ResourceReceipt {
        host_id: "host-1".into(),
        process_id: 1,
        desired_revision: config.revision,
        active_revision: 1,
        generation: 1,
        active: LanguageResourceIdentity::startup(&startup, None),
        user_generation: Some(7),
        state: ResourceState::Rejected,
        error: Some(ResourceError::VerifyResource.to_string()),
    };
    repo.publish_receipt(&receipt).unwrap();
    receipt.host_id = "host-2".into();
    receipt.process_id = 2;
    receipt.state = ResourceState::Deferred;
    receipt.error = None;
    repo.publish_receipt(&receipt).unwrap();
    let reports = repo.receipts().unwrap();
    assert_eq!(reports[0].state, ResourceState::Rejected);
    assert_eq!(reports[1].state, ResourceState::Deferred);
    assert_eq!(reports[0].active, reports[1].active);
    assert_eq!(reports[1].user_generation, Some(7));
    assert_eq!(
        reports[1].active.base.admission,
        ResourceAdmission::StartupUnregistered
    );
}

#[test]
#[ignore = "需要本地固定 CC-CEDICT、Unihan、传统字形词典和 lexical 模型"]
fn fixed_dictionary_layers_and_model_restore() {
    let root = Temp::new();
    let repo = LanguageResourceRepository::new(root.0.clone());
    let case = |key: &str, sha: &str| {
        DictionaryArtifact::new(std::env::var_os(key).unwrap().into(), sha.to_owned()).unwrap()
    };
    let base = case("CLOUDLEAF_ROUND29_BASE", BASE);
    let unihan = case("CLOUDLEAF_ROUND29_UNIHAN", UNIHAN);
    let supplement = case("CLOUDLEAF_ROUND29_SUPPLEMENT", SUPPLEMENT);
    let startup = Arc::new(
        SearchSnapshot::from_verified_tsv(&std::fs::read_to_string(base.path()).unwrap(), BASE)
            .unwrap(),
    );
    let model_tsv =
        std::fs::read_to_string(std::env::var_os("CLOUDLEAF_ROUND29_MODEL").unwrap()).unwrap();
    let model = Arc::new(
        cloudleaf_input::LanguageSnapshot::from_verified_tsv(
            &model_tsv,
            "36dc4d9057bc70e3494fe397da09e8ab6f187c435bd735b66a7734f9d414fce5",
            &startup,
        )
        .unwrap(),
    );
    let manager = LanguageResourceManager;

    let selected = repo
        .save(1, ResourceAction::Select(ResourceSlot::Base, base.clone()))
        .unwrap();
    let fixed = manager.prepare(&selected, &startup, Some(&model)).unwrap();
    assert_eq!(fixed.snapshot.entries().len(), 123_544);
    assert!(Arc::ptr_eq(fixed.language.as_ref().unwrap(), &model));
    assert_eq!(fixed.identity.base.runtime_sha256, BASE);
    assert_eq!(
        repo.save(
            1,
            ResourceAction::Select(ResourceSlot::Base, unihan.clone())
        ),
        Err(ResourceError::StaleRevision)
    );

    let left = repo
        .save(
            selected.revision,
            ResourceAction::Select(ResourceSlot::Base, unihan.clone()),
        )
        .unwrap();
    let alternate = manager.prepare(&left, &startup, Some(&model)).unwrap();
    assert_eq!(alternate.snapshot.entries().len(), 44_431);
    assert_eq!(alternate.identity.base.runtime_sha256, UNIHAN);
    assert!(alternate.language.is_none());
    let reverted = repo
        .save(left.revision, ResourceAction::Rollback(ResourceSlot::Base))
        .unwrap();
    assert_eq!(reverted.base, Some(base));
    assert_eq!(reverted.previous_base, Some(unihan.clone()));
    assert!(Arc::ptr_eq(
        &manager
            .prepare(&reverted, &startup, Some(&model))
            .unwrap()
            .snapshot,
        &startup
    ));

    let selected = repo
        .save(
            reverted.revision,
            ResourceAction::Select(ResourceSlot::Supplement, supplement.clone()),
        )
        .unwrap();
    assert!(
        manager
            .prepare(&selected, &startup, Some(&model))
            .unwrap()
            .identity
            .supplement
            .is_none()
    );
    let enabled = repo
        .save(
            selected.revision,
            ResourceAction::SetSupplementEnabled(true),
        )
        .unwrap();
    let both = manager.prepare(&enabled, &startup, Some(&model)).unwrap();
    assert_eq!(both.snapshot.entries().len(), 127_354);
    assert_ne!(both.snapshot.version(), startup.version());
    let reserved = SearchSnapshot::managed_resource_capacity_bounds();
    let actual = both.snapshot.capacity_bounds();
    assert!(actual.max_frame_bytes <= reserved.max_frame_bytes);
    assert!(actual.max_diagnostic_bytes <= reserved.max_diagnostic_bytes);
    assert_eq!(
        both.identity.supplement.as_ref().unwrap().admission,
        ResourceAdmission::ResearchOnly
    );
    assert!(both.language.is_none());

    let damaged = root.artifact("bad-layer.tsv", SUPPLEMENT);
    std::fs::copy(supplement.path(), damaged.path()).unwrap();
    let changed = repo
        .save(
            enabled.revision,
            ResourceAction::Select(ResourceSlot::Supplement, damaged.clone()),
        )
        .unwrap();
    assert_eq!(changed.previous_supplement, Some(supplement.clone()));
    assert_eq!(changed.base, reverted.base);
    std::fs::write(damaged.path(), HEADER).unwrap();
    assert!(matches!(
        manager.prepare(&changed, &startup, Some(&model)),
        Err(ResourceError::VerifyResource)
    ));
    assert_eq!(both.snapshot.entries().len(), 127_354);
    let alternative = root.artifact("alternative-layer.tsv", SUPPLEMENT);
    std::fs::copy(supplement.path(), alternative.path()).unwrap();
    let recovered_selection = repo
        .save(
            changed.revision,
            ResourceAction::Select(ResourceSlot::Supplement, alternative.clone()),
        )
        .unwrap();
    assert_eq!(
        recovered_selection.previous_supplement,
        Some(supplement.clone())
    );
    let rolled_back = repo
        .save(
            recovered_selection.revision,
            ResourceAction::Rollback(ResourceSlot::Supplement),
        )
        .unwrap();
    assert_eq!(rolled_back.supplement, Some(supplement));
    assert_eq!(rolled_back.previous_supplement, Some(alternative));
    assert_eq!(
        manager
            .prepare(&rolled_back, &startup, Some(&model))
            .unwrap()
            .identity,
        both.identity
    );
    let disabled = repo
        .save(
            rolled_back.revision,
            ResourceAction::SetSupplementEnabled(false),
        )
        .unwrap();
    let returned = manager.prepare(&disabled, &startup, Some(&model)).unwrap();
    assert_eq!(returned.identity.base.runtime_sha256, BASE);
    assert!(returned.identity.supplement.is_none());
    assert!(Arc::ptr_eq(returned.language.as_ref().unwrap(), &model));

    let at_startup = repo
        .save(disabled.revision, ResourceAction::ResetBase)
        .unwrap();
    let reselected = repo
        .save(
            at_startup.revision,
            ResourceAction::Select(ResourceSlot::Base, unihan.clone()),
        )
        .unwrap();
    assert_eq!(reselected.base, Some(unihan));
    assert_eq!(reselected.previous_base, None);
    let startup_again = repo
        .save(
            reselected.revision,
            ResourceAction::Rollback(ResourceSlot::Base),
        )
        .unwrap();
    assert_eq!(startup_again.base, None);
    let returned = manager
        .prepare(&startup_again, &startup, Some(&model))
        .unwrap();
    assert!(Arc::ptr_eq(&returned.snapshot, &startup));
    assert!(Arc::ptr_eq(returned.language.as_ref().unwrap(), &model));

    let before_rollback = repo.load().unwrap();
    let damaged_previous = before_rollback.previous_supplement.clone().unwrap();
    std::fs::write(damaged_previous.path(), HEADER).unwrap();
    assert_eq!(
        repo.save(
            before_rollback.revision,
            ResourceAction::Rollback(ResourceSlot::Supplement)
        ),
        Err(ResourceError::VerifyResource)
    );
    assert_eq!(repo.load().unwrap(), before_rollback);
}

#[test]
fn repeatedly_invalid_selection_keeps_the_desired_revision_and_rollback_record() {
    let root = Temp::new();
    let repo = LanguageResourceRepository::new(root.0.clone());
    let default = repo.load().unwrap();
    for name in ["bad-first.tsv", "bad-next.tsv"] {
        let candidate = root.artifact(name, BASE);
        std::fs::write(candidate.path(), HEADER).unwrap();
        assert_eq!(
            repo.save(
                default.revision,
                ResourceAction::Select(ResourceSlot::Base, candidate)
            ),
            Err(ResourceError::VerifyResource)
        );
    }
    assert_eq!(repo.load().unwrap(), default);
}
