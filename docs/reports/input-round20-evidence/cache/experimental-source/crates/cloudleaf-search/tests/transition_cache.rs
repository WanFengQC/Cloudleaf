use cloudleaf_pinyin::{PinyinScheme, SchemeConfig};
use cloudleaf_search::{Config, Lattice, Lexicon, TransitionCache, search_graph};

fn lexicon(text: &str) -> Lexicon {
    Lexicon::from_tsv(&format!(
        "# cloudleaf-dictionary-v1\nid\tpinyin\ttext\tfrequency\tannotation\tsource\na\tni\t{text}\t1\t\tfixture\nb\thao\t好\t1\t\tfixture\nc\tni hao\t你好\t1\t\tfixture\n"
    )).unwrap()
}

#[test]
fn memoized_transitions_preserve_complete_search_and_reset_identity() {
    let lex = lexicon("你");
    let other = lexicon("泥");
    let config = Config::default();
    let scheme = SchemeConfig {
        scheme: PinyinScheme::AbbreviatedPinyin,
        ..SchemeConfig::default()
    };
    let mut cache = TransitionCache::default();
    for raw in ["n", "nh", "nhi", "nh", "n", "nihao", "niha", "nihao"] {
        let cached =
            Lattice::build_with_scheme_cached(&lex, raw, &config, &scheme, &mut cache).unwrap();
        let cold = Lattice::build_with_scheme(&lex, raw, &config, &scheme).unwrap();
        assert_eq!(cached.edges(), cold.edges());
        assert_eq!(cached.scheme_statistics(), cold.scheme_statistics());
        for index in 0..cached.edges().len() {
            assert_eq!(cached.interpretation(index), cold.interpretation(index));
        }
        assert_eq!(
            search_graph(&lex, &cached, &config, None, None).unwrap(),
            search_graph(&lex, &cold, &config, None, None).unwrap()
        );
    }
    assert!(cache.statistics().hits > 0);
    assert!(cache.statistics().misses > 0);
    assert!(cache.estimated_bytes() <= TransitionCache::capacity_bytes());
    let cached =
        Lattice::build_with_scheme_cached(&other, "nh", &config, &scheme, &mut cache).unwrap();
    let cold = Lattice::build_with_scheme(&other, "nh", &config, &scheme).unwrap();
    assert_eq!(
        search_graph(&other, &cached, &config, None, None).unwrap(),
        search_graph(&other, &cold, &config, None, None).unwrap()
    );
    assert_eq!(cache.statistics().invalidations, 1);
}

#[test]
fn default_full_path_never_allocates_cache() {
    let lex = lexicon("你");
    let mut cache = TransitionCache::default();
    let graph = Lattice::build_with_scheme_cached(
        &lex,
        "nihao",
        &Config::default(),
        &SchemeConfig::default(),
        &mut cache,
    )
    .unwrap();
    assert!(!graph.edges().is_empty());
    assert_eq!(cache.statistics().lookups, 0);
    assert_eq!(cache.statistics().resident_entries, 0);
    assert!(cache.estimated_bytes() < 1024);
}
