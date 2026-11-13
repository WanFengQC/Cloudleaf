use cloudleaf_search::{Config, FrequencyKind, Lattice, Lexicon};
use std::collections::BTreeMap;

const HEADER: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";

#[test]
fn overlay_preserves_base_indices_and_puts_user_words_inside_the_lattice() {
    let base = Lexicon::from_tsv(&format!(
        "{HEADER}z-base\tyun\t云\t\tunknown\t\tfixture\tz-base\tMIT\n"
    ))
    .unwrap();
    let users = format!(
        "{HEADER}user:1\tyun jian\t云笺\t\tunknown\t\tuser\tuser:1\tLicenseRef-User-Private\n"
    );
    let combined = base.with_user_layer(&users, &BTreeMap::new()).unwrap();
    assert_eq!(combined.entries()[0], base.entries()[0]);
    assert_eq!(combined.entries()[1].id, "user:1");
    assert_eq!(combined.entries()[1].frequency_kind, FrequencyKind::Unknown);
    assert_eq!(combined.entries()[1].frequency, None);
    assert_eq!(combined.user_preference(0), 0);
    assert_eq!(combined.user_preference(1), 1);
    let graph = Lattice::build(&combined, "yunjian", &Config::default()).unwrap();
    assert!(
        graph
            .edges()
            .iter()
            .any(|e| e.start == 0 && e.end == 7 && e.entry == 1)
    );
    assert!(base.entries().iter().all(|e| e.source != "user"));
    assert!(combined.shares_base_with(&base));
    assert!(combined.incremental_estimated_bytes() < 16384);
}

#[test]
fn foreign_or_conflicting_user_sources_and_unknown_preference_ids_are_rejected() {
    let base = Lexicon::from_tsv(&format!(
        "{HEADER}base\tyun\t云\t\tunknown\t\tfixture\tbase\tMIT\n"
    ))
    .unwrap();
    for bad in [
        "user:1\tyun\t云\t\tunknown\t\tforeign\tuser:1\tMIT\n",
        "base\tyun\t云\t\tunknown\t\tuser\tbase\tLicenseRef-User-Private\n",
    ] {
        assert!(
            base.with_user_layer(&format!("{HEADER}{bad}"), &BTreeMap::new())
                .is_err()
        );
    }
    assert!(
        base.with_user_layer(HEADER, &BTreeMap::from([("missing".into(), 1)]))
            .is_err()
    );
    let user =
        format!("{HEADER}user:1\tyun\t云\t\tunknown\t\tuser\tuser:1\tLicenseRef-User-Private\n");
    assert!(
        base.with_user_layer(&user, &BTreeMap::from([("user:1".into(), u32::MAX)]))
            .is_err()
    );
}
