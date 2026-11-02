use cloudleaf_translation::{
    DictionaryTrustPolicy, Language, LocalDictionaryProvider, OutputKind, QualityStatus,
    ReadPolicy, TranslationCancellation, TranslationContext, TranslationDirection,
    TranslationError, TranslationKind, TranslationProvider, TranslationRequest,
    dictionary_review_binding,
};
use ring::digest::{SHA256, digest};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    root: PathBuf,
    package: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "cloudleaf-r11-dictionary-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let package = root.join("catalog");
        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("LICENSE.txt"),
            "MIT mechanism fixture; no product data authorization\n",
        )
        .unwrap();
        fs::write(
            package.join("NOTICE.txt"),
            "Original fixture authors; not independent gold\n",
        )
        .unwrap();
        fs::write(
            package.join("SOURCE.json"),
            b"{\"mechanism_fixture\":true}\n",
        )
        .unwrap();
        Self { root, package }
    }

    fn catalog(&self) -> Value {
        json!({"format":"cloudleaf-translation-dictionary-v1","schema_version":1,"version":"fixture-v1",
          "sources":[{"id":"fixture","version":"1","artifact_sha256":"1".repeat(64),"license_id":"MIT",
            "license_file":"LICENSE.txt","license_sha256":sha(&fs::read(self.package.join("LICENSE.txt")).unwrap()),
            "notice_file":"NOTICE.txt","notice_sha256":sha(&fs::read(self.package.join("NOTICE.txt")).unwrap()),
            "source_lock_file":"SOURCE.json","source_lock_sha256":sha(&fs::read(self.package.join("SOURCE.json")).unwrap()),
            "review_status":"approved_bounded_scope","scope":["raw_gloss"]}],
          "entries":[{"id":"fixture-entry","source_id":"fixture","source_record_id":"raw-1","headword":"测试🙂",
            "raw_glosses":["exact raw gloss \"quoted\"🙂","another raw gloss"],"status":"research_only"}],"mappings":[]})
    }

    fn write(&self, value: &Value) -> PathBuf {
        let path = self.package.join("catalog.json");
        fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        path
    }

    fn mapping_catalog(&self) -> Value {
        let mut value = self.catalog();
        value["entries"] = json!([]);
        value["sources"][0]["scope"] = json!(["translation"]);
        value["mappings"] = json!([{"id":"fixture-mapping-not-language-gold","source_id":"fixture","source_text":"测试短语",
            "target_text":"mechanism phrase fixture","source_language":"zh","target_language":"en","kind":"phrase",
            "status":"approved","review_binding_sha256":null}]);
        let binding = dictionary_review_binding(&serde_json::to_vec(&value).unwrap()).unwrap();
        value["mappings"][0]["review_binding_sha256"] = json!(binding);
        value
    }

    fn approval(&self, catalog: &Value) -> DictionaryTrustPolicy {
        let external = self.root.join("external-owner-policy.json");
        let binding = dictionary_review_binding(&serde_json::to_vec(catalog).unwrap()).unwrap();
        let records=catalog["mappings"].as_array().unwrap().iter().map(|row| json!({"mapping_id":row["id"],
            "author_id":"fixture-author-not-a-person", "reviews":[
                {"reviewer_id":"fixture-reviewer-1-not-a-person","decision":"approve","reason":"Mechanism fixture only","review_binding_sha256":binding},
                {"reviewer_id":"fixture-reviewer-2-not-a-person","decision":"approve","reason":"Mechanism fixture only","review_binding_sha256":binding}],
            "adjudication":null})).collect::<Vec<_>>();
        let review = json!({"schema_version":1,"kind":"cloudleaf-translation-dictionary-reviews","review_binding_sha256":binding,"records":records});
        let review_raw = serde_json::to_vec(&review).unwrap();
        fs::write(self.root.join("external-reviews.json"), &review_raw).unwrap();
        let approved = json!({"schema_version":1,"kind":"cloudleaf-translation-dictionary-approval",
            "catalog_sha256":sha(&fs::read(self.package.join("catalog.json")).unwrap()),
            "catalog_version":catalog["version"],
            "review_binding_sha256":binding,
            "approved_mapping_ids":catalog["mappings"].as_array().unwrap().iter().map(|row| row["id"].clone()).collect::<Vec<_>>(),
            "sources":catalog["sources"],"review_document":{"path":"external-reviews.json","sha256":sha(&review_raw)}});
        let raw = serde_json::to_vec(&approved).unwrap();
        fs::write(&external, &raw).unwrap();
        DictionaryTrustPolicy {
            path: external,
            expected_sha256: sha(&raw),
        }
    }

    fn rebind_reviews(&self, trust: &mut DictionaryTrustPolicy, review: &Value) {
        let raw = serde_json::to_vec(review).unwrap();
        fs::write(self.root.join("external-reviews.json"), &raw).unwrap();
        let mut policy: Value = serde_json::from_slice(&fs::read(&trust.path).unwrap()).unwrap();
        policy["review_document"]["sha256"] = json!(sha(&raw));
        let raw = serde_json::to_vec(&policy).unwrap();
        fs::write(&trust.path, &raw).unwrap();
        trust.expected_sha256 = sha(&raw);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // 只清理本夹具创建的进程专属目录，不接受外部删除路径。
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn sha(raw: &[u8]) -> String {
    digest(&SHA256, raw)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn request(
    text: &str,
    source: Language,
    target: Language,
    kind: TranslationKind,
) -> TranslationRequest {
    TranslationRequest {
        text: text.into(),
        direction: TranslationDirection { source, target },
        kind,
        context: TranslationContext {
            session: 1,
            epoch: 1,
            input_revision: 1,
        },
        send_authorized_to: None,
        use_cache: false,
    }
}

fn token() -> TranslationCancellation {
    TranslationCancellation::new(Duration::from_secs(1))
}

#[test]
fn malformed_catalog_cannot_supply_a_review_binding() {
    assert_eq!(
        dictionary_review_binding(b"not json"),
        Err(TranslationError::InvalidConfig)
    );
}

#[test]
fn exact_word_and_reverse_queries_preserve_raw_strings_and_research_quality() {
    let fixture = Fixture::new();
    let path = fixture.write(&fixture.catalog());
    let provider =
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Research, None).unwrap();
    let forward = provider
        .translate(
            &request(
                "测试🙂",
                Language::Chinese,
                Language::English,
                TranslationKind::Word,
            ),
            &token(),
        )
        .unwrap();
    assert_eq!(
        forward.translated_text,
        "exact raw gloss \"quoted\"🙂\nanother raw gloss"
    );
    assert_eq!(forward.quality, QualityStatus::ResearchOnly);
    assert_eq!(forward.output_kind, OutputKind::DictionaryGloss);
    assert_eq!(forward.source_ids, ["fixture-entry"]);
    let reverse = provider
        .translate(
            &request(
                "exact raw gloss \"quoted\"🙂",
                Language::English,
                Language::Chinese,
                TranslationKind::Phrase,
            ),
            &token(),
        )
        .unwrap();
    assert_eq!(reverse.translated_text, "测试🙂");
    assert_eq!(reverse.output_kind, OutputKind::ReverseLookup);
    assert_eq!(
        provider.translate(
            &request(
                "raw gloss",
                Language::English,
                Language::Chinese,
                TranslationKind::Word
            ),
            &token()
        ),
        Err(TranslationError::NoMatch)
    );
    assert_eq!(provider.stats().raw_glosses, 2);
    assert!(!provider.stats().human_identity_or_rights_proven);
    assert!(!provider.capabilities().network);
}

#[test]
fn auto_uses_explicit_lookup_direction_and_sentence_translation_is_absent() {
    let fixture = Fixture::new();
    let provider = LocalDictionaryProvider::load(
        "dictionary",
        &fixture.write(&fixture.catalog()),
        ReadPolicy::Research,
        None,
    )
    .unwrap();
    let result = provider
        .translate(
            &request(
                "测试🙂",
                Language::Auto,
                Language::English,
                TranslationKind::Phrase,
            ),
            &token(),
        )
        .unwrap();
    assert_eq!(result.resolved_source, Language::Chinese);
    assert_eq!(result.output_kind, OutputKind::DictionaryGloss);
    assert_eq!(
        provider.translate(
            &request(
                "测试🙂",
                Language::Chinese,
                Language::English,
                TranslationKind::Sentence
            ),
            &token()
        ),
        Err(TranslationError::UnsupportedTaskKind)
    );
    assert_eq!(
        provider.translate(
            &request(
                "测试🙂",
                Language::Chinese,
                Language::Chinese,
                TranslationKind::Word
            ),
            &token()
        ),
        Err(TranslationError::UnsupportedLanguagePair)
    );
    let cancelled = token();
    cancelled.cancel();
    assert_eq!(
        provider.translate(
            &request(
                "测试🙂",
                Language::Chinese,
                Language::English,
                TranslationKind::Word
            ),
            &cancelled
        ),
        Err(TranslationError::Cancelled)
    );
    assert_eq!(
        provider.translate(
            &request(
                "测试🙂",
                Language::Chinese,
                Language::English,
                TranslationKind::Word
            ),
            &TranslationCancellation::new(Duration::ZERO)
        ),
        Err(TranslationError::Timeout)
    );
}

#[test]
fn strict_schema_identity_references_and_source_scope_are_checked_at_load() {
    let fixture = Fixture::new();
    let original = fixture.catalog();
    for operation in 0..7 {
        let mut value = original.clone();
        match operation {
            0 => value["schema_version"] = json!(true),
            1 => value["extra"] = json!("unknown"),
            2 => {
                let duplicate = value["entries"][0].clone();
                value["entries"].as_array_mut().unwrap().push(duplicate);
            }
            3 => value["entries"][0]["source_id"] = json!("missing"),
            4 => value["sources"][0]["scope"] = json!(["translation"]),
            5 => value["sources"][0]["license_file"] = json!("../outside"),
            _ => value["entries"][0]["status"] = json!("approved"),
        }
        assert!(
            LocalDictionaryProvider::load(
                "dictionary",
                &fixture.write(&value),
                ReadPolicy::Research,
                None
            )
            .is_err()
        );
    }
    let path = fixture.write(&original);
    let raw = fs::read_to_string(&path).unwrap().replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    fs::write(&path, raw).unwrap();
    assert!(
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Research, None).is_err()
    );
    fixture.write(&original);
    fs::write(fixture.package.join("LICENSE.txt"), "changed license").unwrap();
    assert!(
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Research, None).is_err()
    );
}

#[test]
fn research_and_pending_records_never_become_production_from_self_claims() {
    let fixture = Fixture::new();
    let path = fixture.write(&fixture.catalog());
    assert!(matches!(
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Production, None),
        Err(TranslationError::DataNotApproved)
    ));
    let catalog = fixture.mapping_catalog();
    let path = fixture.write(&catalog);
    let research =
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Research, None).unwrap();
    let request = request(
        "测试短语",
        Language::Chinese,
        Language::English,
        TranslationKind::Phrase,
    );
    assert_eq!(
        research.translate(&request, &token()).unwrap().quality,
        QualityStatus::ResearchOnly
    );
    assert!(matches!(
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Production, None),
        Err(TranslationError::DataNotApproved)
    ));
    let mut pending = catalog;
    pending["mappings"][0]["status"] = json!("pending");
    let provider = LocalDictionaryProvider::load(
        "dictionary",
        &fixture.write(&pending),
        ReadPolicy::Research,
        None,
    )
    .unwrap();
    assert_eq!(
        provider.translate(&request, &token()).unwrap().quality,
        QualityStatus::Pending
    );
}

#[test]
fn explicit_directed_mapping_needs_external_current_policy_and_is_not_automatically_reversed() {
    let fixture = Fixture::new();
    let catalog = fixture.mapping_catalog();
    let path = fixture.write(&catalog);
    let trust = fixture.approval(&catalog);
    let provider =
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Production, Some(&trust))
            .unwrap();
    let result = provider
        .translate(
            &request(
                "测试短语",
                Language::Chinese,
                Language::English,
                TranslationKind::Phrase,
            ),
            &token(),
        )
        .unwrap();
    assert_eq!(result.quality, QualityStatus::Approved);
    assert_eq!(result.output_kind, OutputKind::Translation);
    assert!(provider.stats().production_policy_bound);
    assert!(!provider.stats().human_identity_or_rights_proven);
    assert_eq!(
        provider.translate(
            &request(
                "mechanism phrase fixture",
                Language::English,
                Language::Chinese,
                TranslationKind::Phrase
            ),
            &token()
        ),
        Err(TranslationError::NoMatch)
    );
    let internal = DictionaryTrustPolicy {
        path: fixture.package.join("inside-policy.json"),
        expected_sha256: trust.expected_sha256.clone(),
    };
    fs::copy(&trust.path, &internal.path).unwrap();
    assert!(matches!(
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Production, Some(&internal)),
        Err(TranslationError::DataNotApproved)
    ));
    fs::write(&trust.path, b"{}\n").unwrap();
    assert!(matches!(
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Production, Some(&trust)),
        Err(TranslationError::DataNotApproved)
    ));
}

#[test]
fn production_needs_actual_review_material_in_addition_to_external_policy() {
    let fixture = Fixture::new();
    let catalog = fixture.mapping_catalog();
    let path = fixture.write(&catalog);
    let mut trust = fixture.approval(&catalog);
    let mut policy: Value = serde_json::from_slice(&fs::read(&trust.path).unwrap()).unwrap();
    policy.as_object_mut().unwrap().remove("review_document");
    let raw = serde_json::to_vec(&policy).unwrap();
    fs::write(&trust.path, &raw).unwrap();
    trust.expected_sha256 = sha(&raw);
    assert!(matches!(
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Production, Some(&trust)),
        Err(TranslationError::DataNotApproved)
    ));
}

#[test]
fn actual_review_file_needs_current_independent_declarations_and_complete_denominator() {
    let fixture = Fixture::new();
    let catalog = fixture.mapping_catalog();
    let path = fixture.write(&catalog);
    for operation in 0..7 {
        let mut trust = fixture.approval(&catalog);
        let mut review: Value =
            serde_json::from_slice(&fs::read(fixture.root.join("external-reviews.json")).unwrap())
                .unwrap();
        match operation {
            0 => {
                review["records"][0]["reviews"][1]["reviewer_id"] =
                    review["records"][0]["reviews"][0]["reviewer_id"].clone()
            }
            1 => {
                review["records"][0]["reviews"][0]["reviewer_id"] =
                    review["records"][0]["author_id"].clone()
            }
            2 => review["records"][0]["reviews"][0]["reason"] = json!(""),
            3 => {
                review["records"][0]["reviews"][0]["review_binding_sha256"] = json!("0".repeat(64))
            }
            4 => review["records"][0]["reviews"][0]["decision"] = json!("reject"),
            5 => review["records"] = json!([]),
            _ => {
                let row = review["records"][0].clone();
                review["records"].as_array_mut().unwrap().push(row);
            }
        }
        fixture.rebind_reviews(&mut trust, &review);
        assert!(
            matches!(
                LocalDictionaryProvider::load(
                    "dictionary",
                    &path,
                    ReadPolicy::Production,
                    Some(&trust)
                ),
                Err(TranslationError::DataNotApproved)
            ),
            "case {operation}"
        );
    }
    let mut trust = fixture.approval(&catalog);
    let mut review: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("external-reviews.json")).unwrap())
            .unwrap();
    review["records"][0]["reviews"][0]["decision"] = json!("reject");
    let binding = review["review_binding_sha256"].clone();
    review["records"][0]["adjudication"] = json!({"adjudicator_id":"fixture-independent-arbiter-not-a-person","decision":"approve","reason":"mechanical arbitration fixture","review_binding_sha256":binding});
    fixture.rebind_reviews(&mut trust, &review);
    assert!(
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Production, Some(&trust))
            .is_ok()
    );
    review["records"][0]["adjudication"]["adjudicator_id"] =
        review["records"][0]["reviews"][1]["reviewer_id"].clone();
    fixture.rebind_reviews(&mut trust, &review);
    assert!(matches!(
        LocalDictionaryProvider::load("dictionary", &path, ReadPolicy::Production, Some(&trust)),
        Err(TranslationError::DataNotApproved)
    ));
}

#[test]
fn changing_mapping_invalidates_review_even_when_external_file_hash_is_renewed() {
    let fixture = Fixture::new();
    let mut catalog = fixture.mapping_catalog();
    fixture.write(&catalog);
    let _old = fixture.approval(&catalog);
    catalog["mappings"][0]["target_text"] = json!("changed meaning");
    let path = fixture.write(&catalog);
    let new_trust = fixture.approval(&catalog);
    assert!(matches!(
        LocalDictionaryProvider::load(
            "dictionary",
            &path,
            ReadPolicy::Production,
            Some(&new_trust)
        ),
        Err(TranslationError::DataNotApproved)
    ));
}

#[test]
fn lookup_limits_reject_instead_of_silently_dropping_sources() {
    let fixture = Fixture::new();
    let mut catalog = fixture.catalog();
    let template = catalog["entries"][0].clone();
    catalog["entries"] = json!(
        (0..65)
            .map(|index| {
                let mut row = template.clone();
                row["id"] = json!(format!("entry-{index}"));
                row["headword"] = json!(format!("词{index}"));
                row["raw_glosses"] = json!(["shared raw gloss"]);
                row
            })
            .collect::<Vec<_>>()
    );
    let provider = LocalDictionaryProvider::load(
        "dictionary",
        &fixture.write(&catalog),
        ReadPolicy::Research,
        None,
    )
    .unwrap();
    assert_eq!(
        provider.translate(
            &request(
                "shared raw gloss",
                Language::English,
                Language::Chinese,
                TranslationKind::Word
            ),
            &token()
        ),
        Err(TranslationError::ResponseTooLarge)
    );
}
