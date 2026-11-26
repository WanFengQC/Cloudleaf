use cloudleaf_pinyin::{PinyinScheme, SchemeConfig};
use cloudleaf_search::{
    Budget, Cancellation, Config, ErrorKind, LanguageModelSnapshot, Lattice, Lexicon, Method,
    Ranker, search_graph, search_graph_cancellable,
};

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    *state >> 32
}

#[test]
fn seeded_exact_search_keeps_full_candidates_across_graph_sizes() {
    let mut state = 0x13c8be321f924e6d;
    let mut count = 0;
    for words_per_syllable in [2, 3, 5, 8] {
        let mut tsv = String::from(
            "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n",
        );
        let mut sentences = Vec::new();
        for (offset, pinyin) in ["ma", "na", "ma na"].into_iter().enumerate() {
            for item in 0..words_per_syllable {
                let symbol = char::from_u32(0x4e00 + (item % 4) as u32).unwrap();
                let text = if offset == 2 {
                    format!("{symbol}{symbol}")
                } else {
                    symbol.to_string()
                };
                tsv.push_str(&format!("w-{offset}-{item:02}\t{pinyin}\t{text}\t\tunknown\t\tfixture\tr-{offset}-{item}\tMIT\n"));
            }
        }
        for _ in 0..16 {
            let row = (0..5)
                .map(|_| {
                    char::from_u32(0x4e00 + (next(&mut state) % 6) as u32)
                        .unwrap()
                        .to_string()
                })
                .collect();
            sentences.push(row);
        }
        let lexicon = Lexicon::from_tsv(&tsv).unwrap();
        let metadata = format!(
            "# cloudleaf-language-v1\nlexicon_sha256\t{SHA}\nsource_sha256\t{SHA}\nsource_id\tcloudleaf:mechanical-r25\nlicense_id\tMIT\ntokenization\tunicode-scalar-v1\nusage\tmechanical\nkind\tcharacter_bigram_witten_bell\ntraining_split\ttrain\nprevious\tnext\tcount\n"
        );
        let model = LanguageModelSnapshot::train(&metadata, &lexicon, SHA, &sentences).unwrap();
        for _ in 0..14 {
            let length = (next(&mut state) % 4 + 1) as usize;
            let mut raw = String::new();
            for index in 0..length {
                if index != 0 && next(&mut state).is_multiple_of(3) {
                    raw.push('\'');
                }
                raw.push_str(if next(&mut state).is_multiple_of(2) {
                    "ma"
                } else {
                    "na"
                });
            }
            for scheme in [
                SchemeConfig::default(),
                SchemeConfig {
                    scheme: PinyinScheme::AbbreviatedPinyin,
                    ..SchemeConfig::default()
                },
            ] {
                for output_k in [1, 5, 10] {
                    let config = Config {
                        output_k,
                        ranker: Ranker::CharacterBigram,
                        ..Config::default()
                    };
                    let graph =
                        Lattice::build_with_scheme(&lexicon, &raw, &config, &scheme).unwrap();
                    let actual =
                        search_graph(&lexicon, &graph, &config, Some(model.model()), None).unwrap();
                    let reference = search_graph(
                        &lexicon,
                        &graph,
                        &Config {
                            method: Method::Reference,
                            ..config
                        },
                        Some(model.model()),
                        None,
                    )
                    .unwrap();
                    assert!(
                        actual.guaranteed && reference.guaranteed,
                        "{raw}: {:?}",
                        reference.statistics
                    );
                    assert_eq!(
                        actual.candidates, reference.candidates,
                        "{raw}, {:?}, {words_per_syllable}, {output_k}",
                        scheme.scheme
                    );
                    assert_eq!(actual.raw_fallback, raw);
                    count += actual.candidates.len();
                }
            }
        }
        let limited = Config {
            ranker: Ranker::CharacterBigram,
            max_resident_paths: 1,
            ..Config::default()
        };
        let graph = Lattice::build(&lexicon, "mana", &limited).unwrap();
        let stopped = search_graph(&lexicon, &graph, &limited, Some(model.model()), None).unwrap();
        assert_eq!(stopped.statistics.budget, Some(Budget::ResidentPaths));
        assert_eq!(stopped.raw_fallback, "mana");
        assert!(!stopped.guaranteed);
        let cancel = Cancellation::default();
        cancel.cancel();
        assert_eq!(
            search_graph_cancellable(
                &lexicon,
                &graph,
                &limited,
                Some(model.model()),
                None,
                &cancel
            )
            .unwrap_err()
            .kind,
            ErrorKind::Cancelled
        );
    }
    assert!(count > 1_000);
    println!("Round 25 完整随机差分候选：{count}");
}
