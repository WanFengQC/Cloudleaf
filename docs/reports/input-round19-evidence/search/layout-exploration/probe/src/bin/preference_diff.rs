use cloudleaf_pinyin::{
    CorrectionConfig, DoublePinyinLayout, FuzzyDirection, FuzzyPair, FuzzyRule, PinyinScheme,
    SchemeConfig,
};
use cloudleaf_search::{
    BigramModel, Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, search_graph,
};
use std::{collections::BTreeMap, fs};

fn metadata(kind: &str, sha: &str) -> String {
    let tokens = if kind.starts_with("character_") {
        "unicode-scalar-v1"
    } else {
        "stable-entry-ids"
    };
    format!(
        "# cloudleaf-language-v1\nlexicon_sha256\t{sha}\nsource_sha256\t{sha}\nsource_id\tcloudleaf:mechanical-r18-diff\nlicense_id\tMIT\ntokenization\t{tokens}\nusage\tmechanical\nkind\t{kind}\ntraining_split\ttrain\nprevious\tnext\tcount\n"
    )
}

fn corpus() -> Vec<String> {
    let mut values = Vec::new();
    for length in 1..=5_u32 {
        for number in 0..4_usize.pow(length) {
            let mut n = number;
            let mut input = String::new();
            for _ in 0..length {
                input.push([b'a', b'n', b'h', b'i'][n % 4] as char);
                n /= 4;
            }
            values.push(input);
        }
    }
    for input in [
        "zgr",
        "bjdx",
        "zhg",
        "zg",
        "zhonggr",
        "wzgs",
        "nihao",
        "nh",
        "xian",
        "xi'an",
        "nihso",
        "nihoa",
        "ln",
        "niha",
        "ni'",
        "nihao'",
        "ni'hao",
        "lv",
        "nv",
        "lü",
        "LV",
        "xinhang",
        "vhgsrf",
        "bjjkdxxt",
        "nihk",
        "lo",
        "aa",
        "ah",
        "eg",
        "ve",
        "üe",
        "qz",
        "nihaonihao",
        "nihaon",
        "ninhao",
    ] {
        values.push(input.into());
    }
    values.sort();
    values.dedup();
    values
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let data = fs::read_to_string(&args[1]).unwrap();
    let sha = &args[2];
    let base_lex = Lexicon::from_tsv(&data).unwrap();
    let header = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";
    let users = format!(
        "{header}user:1\tni hao\t你好\t\tunknown\t\tuser\tuser:1\tLicenseRef-User-Private\nuser:2\tni hao\t拟好\t\tunknown\t\tuser\tuser:2\tLicenseRef-User-Private\nuser:3\tzhong guo ren\t中国人\t\tunknown\t\tuser\tuser:3\tLicenseRef-User-Private\nuser:4\tna\t那\t\tunknown\t\tuser\tuser:4\tLicenseRef-User-Private\n"
    );
    let preferences = base_lex
        .entries()
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.clone(), [0, 1, 3, 8][i % 4]))
        .chain([
            ("user:1".into(), 1),
            ("user:2".into(), 8),
            ("user:3".into(), 4),
            ("user:4".into(), 2),
        ])
        .collect();
    let lex = base_lex.with_user_layer(&users, &preferences).unwrap();
    println!("USER_LAYER\t{}\t{:?}", lex.entries().len(), preferences);

    let mut lexical_text = metadata("lexical", sha);
    for (i, e) in lex.entries().iter().enumerate() {
        lexical_text.push_str(&format!("<lexical>\t{}\t{}\n", e.id, (i + 1) * 7));
    }
    let lexical = LanguageModelSnapshot::from_tsv(&lexical_text, &lex, sha).unwrap();
    let ids: Vec<_> = lex.entries().iter().map(|e| e.id.clone()).collect();
    let mut word_sequences = Vec::new();
    for i in 0..ids.len() {
        word_sequences.push(vec![
            ids[i].clone(),
            ids[(i + 1) % ids.len()].clone(),
            ids[(i + 3) % ids.len()].clone(),
        ]);
    }
    let bigram =
        LanguageModelSnapshot::train(&metadata("bigram", sha), &lex, sha, &word_sequences).unwrap();
    let mut char_sequences = Vec::new();
    for (i, e) in lex.entries().iter().enumerate() {
        for _ in 0..i % 4 + 1 {
            char_sequences.push(e.text.chars().map(|c| c.to_string()).collect());
        }
    }
    for text in [
        "你好中国人",
        "北京大学",
        "西安你好",
        "农户好",
        "阿安那你",
        "你你你",
        "好好好",
    ] {
        char_sequences.push(text.chars().map(|c| c.to_string()).collect());
    }
    let addone = LanguageModelSnapshot::train(
        &metadata("character_bigram", sha),
        &lex,
        sha,
        &char_sequences,
    )
    .unwrap();
    let witten = LanguageModelSnapshot::train(
        &metadata("character_bigram_witten_bell", sha),
        &lex,
        sha,
        &char_sequences,
    )
    .unwrap();
    for (name, m) in [
        ("lexical", &lexical),
        ("bigram", &bigram),
        ("addone", &addone),
        ("witten", &witten),
    ] {
        fs::write(format!("{name}.tsv"), m.to_tsv()).unwrap();
    }
    let cases = corpus();
    fs::write("inputs.tsv", cases.join("\n") + "\n").unwrap();
    let variants = [
        ("full", SchemeConfig::default()),
        (
            "abbreviated",
            SchemeConfig {
                scheme: PinyinScheme::AbbreviatedPinyin,
                ..Default::default()
            },
        ),
        (
            "fuzzy-all",
            SchemeConfig {
                fuzzy: [
                    FuzzyPair::ZhZ,
                    FuzzyPair::ChC,
                    FuzzyPair::ShS,
                    FuzzyPair::NL,
                    FuzzyPair::InIng,
                    FuzzyPair::EnEng,
                    FuzzyPair::AnAng,
                ]
                .into_iter()
                .map(|pair| FuzzyRule {
                    pair,
                    direction: FuzzyDirection::Both,
                })
                .collect(),
                ..Default::default()
            },
        ),
        (
            "correction",
            SchemeConfig {
                correction: Some(CorrectionConfig::default()),
                ..Default::default()
            },
        ),
        (
            "xiaohe",
            SchemeConfig {
                scheme: PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
                ..Default::default()
            },
        ),
    ];
    let mut totals = BTreeMap::<String, usize>::new();
    let mut failures = 0;
    for (variant, scheme) in variants {
        for raw in &cases {
            let base = Config {
                method: Method::ExactDag,
                ranker: Ranker::Structural,
                output_k: 10,
                context_pool: 20,
                beam_width: 1024,
                max_input_bytes: 64,
                max_letters: 64,
                ..Default::default()
            };
            let graph = match Lattice::build_with_scheme(&lex, raw, &base, &scheme) {
                Ok(graph) => graph,
                Err(error) => {
                    println!("GRAPH_ERROR\t{variant}\t{raw:?}\t{error:?}");
                    *totals.entry("graph_errors".into()).or_default() += 1;
                    continue;
                }
            };
            println!(
                "GRAPH\t{variant}\t{raw:?}\t{:?}\t{:?}\t{:?}",
                graph.budget(),
                graph.edges(),
                graph.scheme_statistics()
            );
            if graph.budget().is_some() {
                *totals.entry("graph_budget".into()).or_default() += 1;
            }
            for (label, ranker, model) in [
                ("structural", Ranker::Structural, None),
                ("weighted", Ranker::Weighted, None),
                ("lexical", Ranker::Lexical, Some(lexical.model())),
                ("bigram", Ranker::Bigram, Some(bigram.model())),
                ("addone", Ranker::CharacterBigram, Some(addone.model())),
                ("witten", Ranker::CharacterBigram, Some(witten.model())),
            ] {
                let model: Option<&BigramModel> = model;
                let reference = search_graph(
                    &lex,
                    &graph,
                    &Config {
                        method: Method::Reference,
                        ranker,
                        ..base.clone()
                    },
                    model,
                    None,
                )
                .unwrap();
                *totals.entry("reference_queries".into()).or_default() += 1;
                if !reference.guaranteed {
                    *totals.entry("reference_incomplete".into()).or_default() += 1;
                }
                println!("REFERENCE\t{variant}\t{label}\t{raw:?}\t{reference:?}");
                for method in [Method::ExactDag, Method::PositionDag, Method::Beam] {
                    if method == Method::PositionDag
                        && !matches!(
                            ranker,
                            Ranker::Structural | Ranker::Weighted | Ranker::Lexical
                        )
                    {
                        *totals.entry("position_unsupported".into()).or_default() += 1;
                        continue;
                    }
                    let result = search_graph(
                        &lex,
                        &graph,
                        &Config {
                            method,
                            ranker,
                            ..base.clone()
                        },
                        model,
                        None,
                    )
                    .unwrap();
                    let comparable = reference.guaranteed && result.guaranteed;
                    let equal = reference.candidates == result.candidates;
                    *totals
                        .entry("candidate_comparisons_attempted".into())
                        .or_default() += 1;
                    if comparable {
                        *totals
                            .entry("complete_candidate_comparisons".into())
                            .or_default() += 1;
                    }
                    if !result.guaranteed {
                        *totals.entry("optimized_incomplete".into()).or_default() += 1;
                    }
                    if result.statistics.beam_pruned > 0 {
                        *totals.entry("beam_pruned_queries".into()).or_default() += 1;
                    }
                    println!(
                        "COMPARE\t{variant}\t{label}\t{raw:?}\t{method:?}\tcomparable={comparable}\tequal={equal}\t{result:?}"
                    );
                    if comparable && !equal {
                        failures += 1;
                        println!("FAIL\t{variant}\t{label}\t{raw:?}\t{method:?}");
                    }
                }
            }
        }
    }
    println!(
        "SUMMARY\tinputs={}\tschemes=5\tfailures={failures}\t{totals:?}\tformal_language_metrics=null",
        cases.len()
    );
    assert_eq!(failures, 0);
}
