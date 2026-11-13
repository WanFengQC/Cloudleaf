use cloudleaf_pinyin::{FuzzyDirection, FuzzyPair, FuzzyRule, PinyinScheme, SchemeConfig};
use cloudleaf_search::{
    Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, search_graph,
};
use std::collections::BTreeMap;

const HEADER: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";

#[test]
fn explicit_preferences_change_order_without_rewriting_interpretation_or_frequency() {
    let source = format!(
        "{HEADER}a\tni\t你\t\tunknown\t\tmechanical\ta\tMIT\nb\tni\t尼\t\tunknown\t\tmechanical\tb\tMIT\nc\tli\t李\t\tunknown\t\tmechanical\tc\tMIT\n"
    );
    let base = Lexicon::from_tsv(&source).unwrap();
    let original = Lattice::build(&base, "ni", &Config::default()).unwrap();
    let baseline = search_graph(&base, &original, &Config::default(), None, None).unwrap();
    assert_eq!(baseline.candidates[0].text, "你");
    let layer = base
        .with_user_layer(HEADER, &BTreeMap::from([("b".into(), 8), ("c".into(), 8)]))
        .unwrap();
    for scheme in [
        SchemeConfig::default(),
        SchemeConfig {
            scheme: PinyinScheme::AbbreviatedPinyin,
            ..Default::default()
        },
        SchemeConfig {
            fuzzy: vec![FuzzyRule {
                pair: FuzzyPair::NL,
                direction: FuzzyDirection::Both,
            }],
            ..Default::default()
        },
    ] {
        let graph = Lattice::build_with_scheme(&layer, "ni", &Config::default(), &scheme).unwrap();
        for method in [
            Method::Reference,
            Method::ExactDag,
            Method::PositionDag,
            Method::Beam,
        ] {
            let result = search_graph(
                &layer,
                &graph,
                &Config {
                    method,
                    ..Default::default()
                },
                None,
                None,
            )
            .unwrap();
            assert!(result.guaranteed);
            assert_eq!(result.candidates[0].text, "尼");
            assert_eq!(result.candidates[0].features.user_preference_units, 8);
            assert!(
                result
                    .candidates
                    .iter()
                    .filter(|p| p.text == "李")
                    .all(|p| p.features.interpretation_cost
                        > result.candidates[0].features.interpretation_cost)
            );
        }
    }
    assert!(layer.entries().iter().all(|e| e.frequency.is_none()));
    let clear = base.with_user_layer(HEADER, &BTreeMap::new()).unwrap();
    let graph = Lattice::build(&clear, "ni", &Config::default()).unwrap();
    assert_eq!(
        baseline.candidates,
        search_graph(&clear, &graph, &Config::default(), None, None)
            .unwrap()
            .candidates
    );
}

#[test]
fn user_words_and_existing_statistics_have_identical_complete_reference_and_exact_results() {
    let base=Lexicon::from_tsv(&format!("{HEADER}a\tni\t你\t\tunknown\t\tmechanical\ta\tMIT\nb\thao\t好\t\tunknown\t\tmechanical\tb\tMIT\nc\tni hao\t你好\t\tunknown\t\tmechanical\tc\tMIT\n")).unwrap();
    let layer = base
        .with_user_layer(
            &format!(
                "{HEADER}user:1\tni hao\t拟好\t\tunknown\t\tuser\tuser:1\tLicenseRef-User-Private\n"
            ),
            &BTreeMap::from([("user:1".into(), 8), ("a".into(), 3)]),
        )
        .unwrap();
    for kind in ["character_bigram", "character_bigram_witten_bell"] {
        let sha = "7".repeat(64);
        let metadata = format!(
            "# cloudleaf-language-v1\nlexicon_sha256\t{sha}\nsource_sha256\t{sha}\nsource_id\tcloudleaf:mechanical-pref\nlicense_id\tMIT\ntokenization\tunicode-scalar-v1\nusage\tmechanical\nkind\t{kind}\ntraining_split\ttrain\nprevious\tnext\tcount\n"
        );
        let model = LanguageModelSnapshot::train(
            &metadata,
            &layer,
            &sha,
            &[
                vec!["你".into(), "好".into()],
                vec!["你".into(), "你".into()],
            ],
        )
        .unwrap();
        for scheme in [
            SchemeConfig::default(),
            SchemeConfig {
                scheme: PinyinScheme::AbbreviatedPinyin,
                ..Default::default()
            },
        ] {
            for raw in ["nihao", "nh", "nihaonihao", "nhnh"] {
                let config = Config {
                    ranker: Ranker::CharacterBigram,
                    output_k: 10,
                    ..Default::default()
                };
                let graph = Lattice::build_with_scheme(&layer, raw, &config, &scheme).unwrap();
                let reference = search_graph(
                    &layer,
                    &graph,
                    &Config {
                        method: Method::Reference,
                        ..config.clone()
                    },
                    Some(model.model()),
                    None,
                )
                .unwrap();
                let exact =
                    search_graph(&layer, &graph, &config, Some(model.model()), None).unwrap();
                assert!(reference.guaranteed && exact.guaranteed, "{raw}");
                assert_eq!(reference.candidates, exact.candidates, "{kind}: {raw}");
            }
        }
    }
}
