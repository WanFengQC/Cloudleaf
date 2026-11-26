use cloudleaf_translation_integrity::{
    Direction, FindingKind, FindingStatus, Glossary, GlossaryEntry, GlossaryError, GlossaryTarget,
    IntegrityReport, LetterCase, check_translation,
};

use Direction::*;
use FindingKind::*;
use FindingStatus::*;

fn compare(source: &str, output: &str, glossary: &Glossary) -> IntegrityReport {
    check_translation(source, output, ChineseToEnglish, glossary)
}

fn statuses(report: &IntegrityReport) -> Vec<(FindingKind, FindingStatus)> {
    report
        .findings
        .iter()
        .map(|finding| (finding.kind, finding.status))
        .collect()
}

#[test]
fn round21_raw_model_output_exposes_currency_and_term_loss() {
    let glossary = Glossary::parse_tsv(
        "cloudleaf-glossary\t1\t21\nzh-en\ttrue\tsensitive\tCloudleaf\tpreserve\t\n",
    )
    .unwrap();
    let input = "2026年11月2日，Cloudleaf 发布了第13轮研发计划，预算为123.45元。";
    let raw =
        "On 2 November 2026, Claudelaf released the 13th round of R & D with a budget of $123.45.";
    let report = compare(input, raw, &glossary);
    assert_eq!(report.glossary_version, 21);
    assert_eq!(
        statuses(&report),
        vec![
            (Date, EquivalentRepresentation),
            (Term, Missing),
            (Number, EquivalentRepresentation),
            (Money, Changed)
        ]
    );
    let money = &report.findings[3];
    assert_eq!(&input[money.source.clone().unwrap()], "123.45元");
    assert_eq!(&raw[money.output.clone().unwrap()], "$123.45");
    assert!(report.requires_acknowledgement());
}

#[test]
fn distinguishes_equivalent_billing_currency_and_ambiguous_symbols() {
    for (source, output, expected) in [
        ("123.45元", "CNY 123.45", EquivalentRepresentation),
        ("123.45元", "USD 123.45", Changed),
        ("$123.45", "USD 123.45", Ambiguous),
        ("$123.45", "USD 124.45", Changed),
        ("C$123.45", "CAD 123.45", EquivalentRepresentation),
        ("100日元", "¥100", Ambiguous),
        ("GBP 5", "£5", EquivalentRepresentation),
        ("-$12.00", "$-12", Ambiguous),
        (
            "RMB -005.50",
            "-5.50 Chinese yuan",
            EquivalentRepresentation,
        ),
        ("CNY 5", "CNY 5 USD", Unverifiable),
        ("(CNY 5)", "CNY 5", Unverifiable),
    ] {
        assert_eq!(
            statuses(&compare(source, output, &Glossary::default())),
            vec![(Money, expected)],
            "{source} / {output}"
        );
    }
}

#[test]
fn local_calendar_equivalences_and_invalid_dates() {
    let empty = Glossary::default();
    for (source, output, expected) in [
        (
            "2026年11月25日",
            "November 25, 2026",
            EquivalentRepresentation,
        ),
        ("2026-11-25", "25 Nov. 2026", EquivalentRepresentation),
        ("2026/11/25", "11/25/2026", EquivalentRepresentation),
        ("１１／１２／２０２６", "November 12, 2026", Ambiguous),
        (
            "2024年2月29日",
            "29 February 2024",
            EquivalentRepresentation,
        ),
        ("2026年2月29日", "March 1, 2026", Unverifiable),
        ("2026年11月25日", "November 24, 2026", Changed),
    ] {
        assert_eq!(
            statuses(&compare(source, output, &empty)),
            vec![(Date, expected)],
            "{source} / {output}"
        );
    }
}

#[test]
fn groups_numbers_and_normalizes_unicode_decimal_without_floating_point() {
    let empty = Glossary::default();
    let report = compare(
        "−１，２３４．５０与+003.0和12.50％",
        "-1234.5 and 3 and 12.5%",
        &empty,
    );
    assert_eq!(
        statuses(&report),
        vec![
            (Number, EquivalentRepresentation),
            (Number, EquivalentRepresentation),
            (Percentage, EquivalentRepresentation)
        ]
    );
    assert!(!report.requires_acknowledgement());
    assert_eq!(
        statuses(&compare("1.234,5", "1234.5", &empty)),
        vec![(Number, Unverifiable)]
    );
    assert_eq!(
        statuses(&compare("3.5e2", "350", &empty)),
        vec![(Number, Unverifiable)]
    );
    assert_eq!(
        statuses(&compare("0.5", ".5", &empty)),
        vec![(Number, EquivalentRepresentation)]
    );
    assert_eq!(
        statuses(&compare("第21", "21st", &empty)),
        vec![(Number, EquivalentRepresentation)]
    );
    assert_eq!(
        statuses(&compare("第11", "11st", &empty)),
        vec![(Number, Unverifiable)]
    );
}

#[test]
fn numeric_counts_and_unexpected_output_are_preserved() {
    let empty = Glossary::default();
    assert_eq!(
        statuses(&compare("2、2、7", "7, 2", &empty)),
        vec![
            (Number, ExactPreserved),
            (Number, Missing),
            (Number, ExactPreserved)
        ]
    );
    let report = compare("2盒", "2 boxes and 7 boxes", &empty);
    assert_eq!(
        statuses(&report),
        vec![(Number, ExactPreserved), (Number, Unverifiable)]
    );
    assert!(report.findings[1].source.is_none());
}

#[test]
fn clock_and_units_do_not_perform_conversions() {
    let empty = Glossary::default();
    for (source, output, kind, expected) in [
        ("下午3点05分", "3:05 p.m.", Time, EquivalentRepresentation),
        ("15:05", "3:05 pm", Time, EquivalentRepresentation),
        ("凌晨 00:12", "12:12 AM", Time, EquivalentRepresentation),
        ("12:14 UTC", "12:14 PST", Time, Unverifiable),
        ("25:70", "23:10", Time, Unverifiable),
        ("12.5 公斤", "12.5 kg", Unit, EquivalentRepresentation),
        ("38摄氏度", "38°C", Unit, EquivalentRepresentation),
        ("100厘米", "1 meter", Unit, Changed),
        ("5毫升", "5", Unit, Changed),
        ("5毫升", "5 mL", Unit, EquivalentRepresentation),
        ("45英寸", "45 inches", Unit, EquivalentRepresentation),
        ("8.5升", "8.5 liters", Unit, EquivalentRepresentation),
        ("38度", "38°C", Unit, Ambiguous),
        ("5 ML", "5 mL", Unit, Ambiguous),
        ("5 KG", "5", Unit, Changed),
        ("5 pounds", "5 lb", Unit, Ambiguous),
        ("百分之12.50", "12.5%", Percentage, EquivalentRepresentation),
    ] {
        assert_eq!(
            statuses(&compare(source, output, &empty)),
            vec![(kind, expected)],
            "{source} / {output}"
        );
    }
}

#[test]
fn term_direction_longest_match_case_boundary_and_disabled_entries() {
    let glossary = Glossary::parse_tsv(concat!(
        "cloudleaf-glossary\t1\t7\n",
        "zh-en\ttrue\tsensitive\tRTX 3060\tpreserve\t\n",
        "zh-en\ttrue\tascii-insensitive\tUSB-C\tpreserve\t\n",
        "zh-en\ttrue\tsensitive\tLeaf\tpreserve\t\n",
        "zh-en\ttrue\tsensitive\tLeaf Pro\tpreserve\t\n",
        "zh-en\tfalse\tsensitive\tDisabled\tpreserve\t\n",
        "en-zh\ttrue\tsensitive\tiPhone 12 mini\tpreserve\t\n",
        "en-zh\ttrue\tascii-insensitive\tCandle\ttranslate\t烛芯\n",
    ))
    .unwrap();
    let input = "接入RTX 3060、USB-C、Leaf Pro和Disabled。";
    let raw = "Works with RTX 3060, usb-c, Leaf Pro, and Disabled2.";
    assert_eq!(
        statuses(&compare(input, raw, &glossary)),
        vec![
            (Term, ExactPreserved),
            (Term, EquivalentRepresentation),
            (Term, ExactPreserved)
        ]
    );
    assert_eq!(
        statuses(&compare("USB-C", "theUSB-C", &glossary)),
        vec![(Term, Missing)]
    );
    assert_eq!(
        statuses(&compare("Leaf Pro", "Leaf Prototype", &glossary)),
        vec![(Term, Missing)]
    );
    assert_eq!(
        statuses(&compare("Leaf", "Leafing", &glossary)),
        vec![(Term, Missing)]
    );
    assert_eq!(
        statuses(&compare("Leaf", "leaf", &glossary)),
        vec![(Term, Missing)]
    );
    let reverse = check_translation(
        "CANDLE for iPhone 12 mini",
        "iPhone 12 mini 的烛芯",
        EnglishToChinese,
        &glossary,
    );
    assert_eq!(
        statuses(&reverse),
        vec![(Term, EquivalentRepresentation), (Term, ExactPreserved)]
    );
    assert!(!reverse.requires_acknowledgement());
}

#[test]
fn recurring_terms_require_one_target_occurrence_each() {
    let glossary = Glossary::new(
        1,
        vec![GlossaryEntry {
            direction: ChineseToEnglish,
            source: "别名".into(),
            target: GlossaryTarget::Translate("Alias".into()),
            case: LetterCase::Sensitive,
            enabled: true,
        }],
    )
    .unwrap();
    let report = compare("别名和别名", "Alias applies", &glossary);
    assert_eq!(
        statuses(&report),
        vec![(Term, EquivalentRepresentation), (Term, Missing)]
    );
}

#[test]
fn invalid_glossary_and_overlapping_case_rules_are_rejected_without_echoing_terms() {
    let rows = concat!(
        "cloudleaf-glossary\t1\t7\n",
        "zh-en\ttrue\tsensitive\tSecretRepo\tpreserve\t\n",
        "zh-en\ttrue\tascii-insensitive\tsecretrepo\ttranslate\tconfidential\n",
    );
    let error = Glossary::parse_tsv(rows).err().unwrap();
    assert_eq!(error, GlossaryError::ConflictingEntries(1, 2));
    assert!(!error.to_string().contains("SecretRepo"));
    assert!(matches!(
        Glossary::parse_tsv("cloudleaf-glossary\t2\t1\n"),
        Err(GlossaryError::InvalidHeader)
    ));
    assert!(matches!(
        Glossary::parse_tsv(
            "cloudleaf-glossary\t1\t1\nzh-en\ttrue\tsensitive\tThing\tpreserve\twrong"
        ),
        Err(GlossaryError::InvalidRow(2))
    ));
}

#[test]
fn provider_reported_truncation_needs_acknowledgement() {
    let mut report = compare("已确认全文。", "Done.", &Glossary::default());
    assert!(!report.requires_acknowledgement());
    report.mark_truncated();
    assert_eq!(statuses(&report), vec![(Truncated, Unverifiable)]);
    assert!(report.requires_acknowledgement());
}

#[test]
fn glossary_cannot_override_a_conflicting_numeric_fact() {
    let glossary = Glossary::parse_tsv(
        "cloudleaf-glossary\t1\t1\nzh-en\ttrue\tsensitive\tCNY 5\ttranslate\tUSD 5\n",
    )
    .unwrap();
    assert_eq!(
        statuses(&compare("CNY 5", "USD 5", &glossary)),
        vec![(Term, EquivalentRepresentation), (Money, Changed)]
    );
}

#[test]
fn technical_literals_are_compared_as_whole_occurrences() {
    let source =
        "请读取build.log，访问https://example.com/x?id=204，再核对v2.0.1、AX-204和AX-204。";
    let output =
        "Read Build.log, visit (https://example.com/x?id=205), then check v2.0.1 and AX-204.";
    let report = compare(source, output, &Glossary::default());
    let evidence = report
        .findings
        .iter()
        .map(|finding| {
            (
                finding.kind,
                finding.status,
                finding.source.clone().map(|span| &source[span]),
                finding.output.clone().map(|span| &output[span]),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        evidence,
        [
            (FileName, Changed, Some("build.log"), Some("Build.log")),
            (
                Url,
                Changed,
                Some("https://example.com/x?id=204"),
                Some("https://example.com/x?id=205")
            ),
            (Version, ExactPreserved, Some("v2.0.1"), Some("v2.0.1")),
            (ModelCode, ExactPreserved, Some("AX-204"), Some("AX-204")),
            (ModelCode, Missing, Some("AX-204"), None),
        ]
    );
    assert!(report.requires_acknowledgement());
}

#[test]
fn literal_boundaries_preserve_distinct_time_and_ignore_email_usernames() {
    let source = "Send log.txt@example.com a note at 2:30 p.m. Open https://example.com/5.";
    let output = "下午3点30分通知log.txt@example.com。访问https://example.com/5。";
    let report = check_translation(source, output, EnglishToChinese, &Glossary::default());
    assert_eq!(statuses(&report), [(Time, Changed), (Url, ExactPreserved)]);
}

#[test]
fn negation_marker_mismatch_requests_review_without_claiming_scope() {
    let pairs = [
        ("请勿断开电源。", "Disconnect the power.", true),
        ("确认。禁止打开盖子。", "Confirmed. Open the lid.", true),
        (
            "请不要断开电源。",
            "Please don't disconnect the power.",
            false,
        ),
        ("请勿断开电源。", "Do not reconnect the power.", false),
        ("无需准备。", "Not everyone is arriving early.", false),
        ("开启。", "Do nothing.", false),
    ];
    for (source, output, warn) in pairs {
        let report = compare(source, output, &Glossary::default());
        assert_eq!(
            report.requires_acknowledgement(),
            warn,
            "{source} / {output}"
        );
        if warn {
            assert_eq!(report.findings[0].kind, NegationMarker);
            assert_eq!(report.findings[0].status, Unverifiable);
        }
    }

    let report = compare(
        "请勿断开电源。请勿打开盖子。",
        "Do not open the lid.",
        &Glossary::default(),
    );
    assert_eq!(statuses(&report), [(NegationMarker, Unverifiable)]);
    assert_eq!(report.findings[0].source, None);
    assert_eq!(report.findings[0].output, None);
}
