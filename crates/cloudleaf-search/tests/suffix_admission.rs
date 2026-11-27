use cloudleaf_pinyin::{BaseMatch, PhoneticArc, PinyinScheme, SchemeConfig, interpret};
use cloudleaf_search::{
    Budget, Config, InterpretationCost, Lattice, Lexicon, Method, Ranker, SchemeAdmission,
    profile_scheme_query_with_admission, search_graph,
};
use std::collections::BTreeMap;

const HEADER: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";

fn abbreviation() -> SchemeConfig {
    SchemeConfig {
        scheme: PinyinScheme::AbbreviatedPinyin,
        ..SchemeConfig::default()
    }
}

fn exhaustive(
    arcs: &[PhoneticArc],
    position: usize,
    syllables: &[&str],
    mut path: Vec<usize>,
    out: &mut Vec<(usize, Vec<usize>)>,
) {
    if let Some((first, rest)) = syllables.split_first() {
        for (index, arc) in arcs.iter().enumerate() {
            if arc.start == position && arc.syllable == *first {
                path.push(index);
                exhaustive(arcs, arc.end, rest, path.clone(), out);
                path.pop();
            }
        }
    } else {
        out.push((position, path));
    }
}

#[test]
fn complete_graph_retains_all_origins_multiple_boundaries_and_ranked_paths() {
    let base = Lexicon::from_tsv(&format!(
        "{HEADER}one\txian\t先\t\tunknown\tbase\toriginal\t1\tMIT\ntwo\txi an\t西安\t\tunknown\tseparate\toriginal\t2\tMIT\nthree\txi\t西\t\tunknown\t\toriginal\t3\tMIT\nfour\tan\t安\t\tunknown\t\toriginal\t4\tMIT\n"
    ))
    .unwrap();
    let users = format!(
        "{HEADER}user:1\txi an\t溪岸\t\tunknown\t\tuser\tuser:1\tLicenseRef-User-Private\n"
    );
    let lexicon = base.with_user_layer(&users, &BTreeMap::new()).unwrap();
    let scheme = abbreviation();
    let config = Config {
        output_k: 10,
        ..Config::default()
    };
    let mut inputs = vec![
        "xa".into(),
        "xian".into(),
        "xi'an".into(),
        "xaxa".into(),
        "xixian".into(),
    ];
    for length in 1..=5 {
        for mask in 0..1 << length {
            inputs.push(
                (0..length)
                    .map(|bit| if mask & (1 << bit) == 0 { 'x' } else { 'a' })
                    .collect::<String>(),
            );
        }
    }
    for raw in inputs.iter().map(String::as_str) {
        let phonetic = interpret(raw, &scheme).unwrap();
        let mut reference = BTreeMap::<_, Vec<Vec<usize>>>::new();
        for start in 0..phonetic.keys.len() {
            for (entry_index, entry) in lexicon.entries().iter().enumerate() {
                for (end, path) in {
                    let syllables: Vec<_> = entry.pinyin.split_whitespace().collect();
                    let mut out = Vec::new();
                    exhaustive(&phonetic.arcs, start, &syllables, Vec::new(), &mut out);
                    out
                } {
                    reference
                        .entry((start, end, entry_index))
                        .or_default()
                        .push(path);
                }
            }
        }
        let build = |admission| {
            profile_scheme_query_with_admission(&lexicon, raw, &config, &scheme, None, admission)
                .0
                .unwrap()
        };
        let (forward, old) = build(SchemeAdmission::Forward);
        for admission in [SchemeAdmission::Balanced, SchemeAdmission::ReachableSuffix] {
            let (graph, result) = build(admission);
            assert_eq!(graph.edges(), forward.edges(), "{raw}");
            assert_eq!(result, old, "{raw}");
            assert_eq!(graph.edges().len(), reference.len(), "{raw}");
            for (index, edge) in graph.edges().iter().enumerate() {
                let paths = &reference[&(edge.start, edge.end, edge.entry)];
                let evidence = graph.interpretation(index).unwrap();
                assert_eq!(Some(evidence), forward.interpretation(index));
                let (cost, indexes) = paths
                    .iter()
                    .map(|indexes| {
                        let mut cost = InterpretationCost::default();
                        for &index in indexes {
                            match phonetic.arcs[index].base {
                                BaseMatch::Abbreviation => cost.abbreviations += 1,
                                BaseMatch::Prefix => cost.prefixes += 1,
                                _ => {}
                            }
                        }
                        (cost, indexes)
                    })
                    .min()
                    .unwrap();
                assert_eq!(evidence.cost, cost, "{raw}");
                assert!(
                    indexes
                        .iter()
                        .map(|&i| &phonetic.arcs[i])
                        .eq(&evidence.arcs)
                );
                assert_eq!(edge.raw_start, phonetic.raw_offsets[edge.start]);
                assert_eq!(edge.raw_end, phonetic.raw_offsets[edge.end]);
            }
            let complete = search_graph(
                &lexicon,
                &graph,
                &Config {
                    method: Method::Reference,
                    ..config.clone()
                },
                None,
                None,
            )
            .unwrap();
            assert_eq!(complete.candidates, result.candidates, "{raw}");
        }
    }
}

#[test]
fn fixed_capacity_can_preserve_a_complete_path_and_keeps_incomplete_status() {
    let mut text = HEADER.to_owned();
    for index in 0..24 {
        let word = char::from_u32(0x4e00 + index).unwrap();
        text.push_str(&format!(
            "one-{index}\tni\t{word}\t\tunknown\t\tfixture\t{index}\tMIT\n"
        ));
    }
    text.push_str("two\tkan\t看\t\tunknown\t\tfixture\ttwo\tMIT\n");
    let lexicon = Lexicon::from_tsv(&text).unwrap();
    let config = Config {
        method: Method::PositionDag,
        ranker: Ranker::Structural,
        max_edges: 8,
        ..Config::default()
    };
    let scheme = abbreviation();
    let (forward, old) = profile_scheme_query_with_admission(
        &lexicon,
        "nk",
        &config,
        &scheme,
        None,
        SchemeAdmission::Forward,
    )
    .0
    .unwrap();
    assert_eq!(forward.budget(), Some(Budget::GraphEdges));
    assert!(old.candidates.is_empty());
    let graph = Lattice::build_with_scheme(&lexicon, "nk", &config, &scheme).unwrap();
    assert_eq!(graph.edges().len(), config.max_edges);
    assert_eq!(graph.budget(), Some(Budget::GraphEdges));
    let result = search_graph(&lexicon, &graph, &config, None, None).unwrap();
    assert!(!result.guaranteed);
    assert!(!result.candidates.is_empty());
    assert!(
        result
            .candidates
            .iter()
            .all(|candidate| candidate.text.ends_with('看'))
    );

    let graph = Lattice::build_with_scheme(&lexicon, "nq", &config, &scheme).unwrap();
    assert!(
        graph
            .edges()
            .iter()
            .any(|edge| edge.start == 0 && edge.end == 1)
    );
    assert!(
        !search_graph(&lexicon, &graph, &config, None, None)
            .unwrap()
            .guaranteed
    );
}
