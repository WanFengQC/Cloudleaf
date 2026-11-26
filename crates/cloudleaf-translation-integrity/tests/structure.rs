use cloudleaf_translation_integrity::{
    Direction, FindingStatus, Glossary, IntegrityFinding, check_translation,
};

fn advisory(direction: Direction, source: &str, output: &str, kind: &str) -> Vec<IntegrityFinding> {
    check_translation(source, output, direction, &Glossary::default())
        .findings
        .into_iter()
        .filter(|finding| format!("{:?}", finding.kind) == kind)
        .collect()
}

#[test]
fn missing_sentence_and_temporal_branch_are_advisory_only() {
    let source = "先核对地址。完成后把箱子交给快递员。";
    let output = "Check the address first.";
    let found = advisory(
        Direction::ChineseToEnglish,
        source,
        output,
        "ClauseCoverage",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].status, FindingStatus::Unverifiable);
    assert_eq!(
        &source[found[0].source.clone().unwrap()],
        "完成后把箱子交给快递员。"
    );

    let found = advisory(
        Direction::EnglishToChinese,
        "We'll save a seat and call you when we arrive.",
        "我们会预留座位。",
        "TemporalMarker",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].status, FindingStatus::Unverifiable);

    let present = advisory(
        Direction::EnglishToChinese,
        "We'll save a seat and call you after we arrive.",
        "我们会预留座位，到达后会给你打电话。",
        "TemporalMarker",
    );
    assert!(present.is_empty());
}

#[test]
fn punctuation_inside_literal_does_not_create_a_sentence() {
    let report = check_translation(
        "请在3:20 p.m.查看https://example.test/report.pdf。",
        "Check https://example.test/report.pdf at 15:20.",
        Direction::ChineseToEnglish,
        &Glossary::default(),
    );
    assert!(!report.requires_acknowledgement(), "{report:?}");
}

#[test]
fn condition_and_named_subject_gaps_remain_unverifiable() {
    let source = "如果 A 箱比 B 箱轻，就先取 A 箱，否则先取 B 箱。";
    let output = "If a box is lighter, take that box first.";
    for kind in ["ConditionalMarker", "ConditionSubject"] {
        let found = advisory(Direction::ChineseToEnglish, source, output, kind);
        assert_eq!(found.len(), 1, "{kind}");
        assert_eq!(found[0].status, FindingStatus::Unverifiable);
    }
    let aligned = "If box A is lighter than box B, pick up box A first, otherwise take box B.";
    let report = check_translation(
        source,
        aligned,
        Direction::ChineseToEnglish,
        &Glossary::default(),
    );
    assert!(!report.requires_acknowledgement(), "{report:?}");
}

#[test]
fn distinct_labels_keep_measurement_bindings_through_reordering() {
    let source = "丙袋装有七公斤货物，丁袋装有9公斤零件。";
    let reordered = "Bag D holds 9 kg of parts and bag C holds seven kg of goods.";
    let accepted = check_translation(
        source,
        reordered,
        Direction::ChineseToEnglish,
        &Glossary::default(),
    );
    assert!(!accepted.requires_acknowledgement(), "{accepted:?}");
    let swapped = "Bag D holds 7 kg of parts and bag C holds nine kg of goods.";
    let found = advisory(
        Direction::ChineseToEnglish,
        source,
        swapped,
        "QuantityBinding",
    );
    assert_eq!(found.len(), 2);
    assert!(
        found
            .iter()
            .all(|item| item.status == FindingStatus::Changed)
    );
    for (finding, (left, right)) in found.iter().zip([("七公斤", "nine kg"), ("9公斤", "7 kg")])
    {
        assert_eq!(&source[finding.source.clone().unwrap()], left);
        assert_eq!(&swapped[finding.output.clone().unwrap()], right);
    }
}

#[test]
fn command_actions_and_explicit_setting_are_separate_evidence() {
    let found = advisory(
        Direction::ChineseToEnglish,
        "请勿重新连接水泵。",
        "Do not disconnect the pump.",
        "CommandAction",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].status, FindingStatus::Unverifiable);

    let found = advisory(
        Direction::EnglishToChinese,
        "Please set worker2 to off.",
        "请将 worker2 设为开启。",
        "SwitchSetting",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].status, FindingStatus::Changed);

    let report = check_translation(
        "Please set worker2 to off.",
        "请将 worker2 设为关闭。",
        Direction::EnglishToChinese,
        &Glossary::default(),
    );
    assert!(!report.requires_acknowledgement(), "{report:?}");
}

#[test]
fn limited_cardinal_and_time_equivalences_preserve_old_boundaries() {
    use cloudleaf_translation_integrity::FindingKind::{Number, Time, Unit};
    let examples = [
        ("十二件货物", "twelve items", Number),
        ("二十五个名额", "twenty-five slots", Number),
        ("两公斤", "two kg", Unit),
        ("下午3点20分", "3.20 p.m.", Time),
        ("下午5点", "5 pm", Time),
        ("5.25", "5.25", Number),
    ];
    for (source, output, kind) in examples {
        let report = check_translation(
            source,
            output,
            Direction::ChineseToEnglish,
            &Glossary::default(),
        );
        assert_eq!(report.findings.len(), 1, "{source} {report:?}");
        assert_eq!(report.findings[0].kind, kind);
        assert!(!report.requires_acknowledgement(), "{source} {report:?}");
    }
    let report = check_translation(
        "零件到了，随便挑一个办法",
        "The parts arrived, choose one way",
        Direction::ChineseToEnglish,
        &Glossary::default(),
    );
    assert!(!report.requires_acknowledgement(), "{report:?}");
}
