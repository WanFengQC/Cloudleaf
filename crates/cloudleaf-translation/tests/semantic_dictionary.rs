use cloudleaf_translation::{
    ReadPolicy, SemanticAnnotationIdentity, SemanticDictionary, TranslationError,
};
use ring::digest::{SHA256, digest};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(value: &Value) -> (Self, String) {
        let path = std::env::temp_dir().join(format!(
            "cloudleaf-semantic-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let data = serde_json::to_vec(value).unwrap();
        std::fs::write(&path, &data).unwrap();
        let hash = digest(&SHA256, &data)
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        (Self(path), hash)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn fixture() -> Value {
    let sha = "a".repeat(64);
    let facts = json!([{"source_id":"test-source","unit_id":"mechanical-unit",
                        "pointer":"fixture:1", "sha256":sha}]);
    let long = "preserved ".repeat(45).trim_end().to_owned();
    json!({
        "format":"cloudleaf-semantic-lookup-v1", "schema_version":1, "scope":"research_only",
        "bundle_manifest_sha256":sha, "content_sha256":sha,
        "catalogs":[
            {"namespace":"zh","language":"zh","manifest_sha256":sha,"facts_sha256":sha,"runtime_sha256":sha},
            {"namespace":"en","language":"en","manifest_sha256":sha,"facts_sha256":sha,"runtime_sha256":sha}
        ],
        "sources":[{"id":"test-source","version":"0","artifact_sha256":sha,"license_id":"fixture","status":"research_only"}],
        "entries":[
            {"namespace":"zh","entry_id":"entry:1","headword":"你好"},
            {"namespace":"en","entry_id":"entry:2","headword":"greeting"}
        ],
        "senses":[
            {"id":"sense:1","namespace":"zh","entry_id":"entry:1","language":"zh","definition":"hello",
             "english_source_gloss":true,"part_of_speech":null,"assertions":facts},
            {"id":"sense:2","namespace":"zh","entry_id":"entry:1","language":"zh","definition":long,
             "english_source_gloss":true,"part_of_speech":null,"assertions":facts},
            {"id":"sense:3","namespace":"en","entry_id":"entry:2","language":"en","definition":"greeting",
             "english_source_gloss":false,
             "part_of_speech":{"namespace":"en","entry_id":"entry:2","tag":"noun","tagset":"fixture","tagset_version":"1"},
             "assertions":facts}
        ],
        "relations":[{"id":"relation:1","source_sense_id":"sense:1","target_sense_id":"sense:3",
                      "relation":"related","assertions":facts}],
        "phrases":[{"id":"phrase:1","namespace":"zh","entry_id":"entry:1","kind":"multiword",
                    "components":[{"namespace":"zh","entry_id":"entry:1"}],"assertions":facts}]
    })
}

#[test]
fn semantic_queries_retain_senses_and_directed_relations_with_source() {
    let (file, sha) = TempFile::new(&fixture());
    let dictionary = SemanticDictionary::load(&file.0, &sha, ReadPolicy::Research).unwrap();
    let zh = dictionary.headword("zh", "你好");
    assert_eq!(zh.total, 2);
    assert!(zh.hits[1].sense.definition.len() > 256);
    assert_eq!(zh.hits[0].sense.assertions[0].source_id, "test-source");
    let en = dictionary.english_gloss_exact("hello");
    assert_eq!(en.total, 1);
    assert_eq!(en.hits[0].entry.entry_id, "entry:1");
    assert_eq!(dictionary.english_gloss_exact("greeting").total, 0);
    assert_eq!(dictionary.english_gloss_keyword("hELLO").total, 1);
    assert_eq!(dictionary.english_gloss_keyword("preserved").total, 1);
    assert_eq!(dictionary.english_gloss_keyword("pre").total, 0);
    assert_eq!(
        dictionary
            .english_gloss_keyword("preserved preserved")
            .total,
        0
    );
    let meaning = dictionary.headword("en", "greeting");
    assert_eq!(
        meaning.hits[0].sense.part_of_speech.as_ref().unwrap().tag,
        "noun"
    );
    let relations = dictionary.relations("sense:3");
    assert_eq!(relations.outgoing_total, 0);
    assert_eq!(relations.incoming[0].source_sense_id, "sense:1");
    assert_eq!(
        dictionary.relations("sense:1").outgoing[0].target_sense_id,
        "sense:3"
    );
    assert_eq!(
        dictionary.phrases_for_entry("zh", "entry:1")[0].id,
        "phrase:1"
    );
}

#[test]
fn index_requires_research_policy_and_exact_bytes_and_rejects_orphan_sense() {
    let mut value = fixture();
    let (file, sha) = TempFile::new(&value);
    assert!(matches!(
        SemanticDictionary::load(&file.0, &sha, ReadPolicy::Production),
        Err(TranslationError::DataNotApproved)
    ));
    assert!(matches!(
        SemanticDictionary::load(&file.0, &"b".repeat(64), ReadPolicy::Research),
        Err(TranslationError::InvalidConfig)
    ));
    value["relations"][0]["target_sense_id"] = json!("sense:orphan");
    let (invalid, digest) = TempFile::new(&value);
    assert!(matches!(
        SemanticDictionary::load(&invalid.0, &digest, ReadPolicy::Research),
        Err(TranslationError::InvalidConfig)
    ));
}

#[test]
fn annotation_requires_same_dictionary_context_and_result_version() {
    let (file, sha) = TempFile::new(&fixture());
    let dictionary = SemanticDictionary::load(&file.0, &sha, ReadPolicy::Research).unwrap();
    let identity = SemanticAnnotationIdentity {
        session: 1,
        input_revision: 7,
        candidate_id: "candidate:old".into(),
        dictionary_runtime_sha256: "a".repeat(64),
        bundle_manifest_sha256: dictionary.bundle_manifest_sha256().into(),
        lookup_index_sha256: dictionary.lookup_index_sha256().into(),
    };
    let update = dictionary
        .prepare_annotation(identity.clone(), "zh", "entry:1")
        .unwrap();
    assert_eq!(update.glosses.total, 2);
    assert!(update.is_current(&identity));
    for next in [
        SemanticAnnotationIdentity {
            session: 2,
            ..identity.clone()
        },
        SemanticAnnotationIdentity {
            input_revision: 8,
            ..identity.clone()
        },
        SemanticAnnotationIdentity {
            candidate_id: "candidate:new".into(),
            ..identity.clone()
        },
        SemanticAnnotationIdentity {
            bundle_manifest_sha256: "x".repeat(64),
            ..identity.clone()
        },
        SemanticAnnotationIdentity {
            lookup_index_sha256: "x".repeat(64),
            ..identity.clone()
        },
    ] {
        assert!(!update.is_current(&next));
    }
    let mismatched = SemanticAnnotationIdentity {
        dictionary_runtime_sha256: "b".repeat(64),
        ..identity
    };
    assert!(
        dictionary
            .prepare_annotation(mismatched, "zh", "entry:1")
            .is_err()
    );
}
