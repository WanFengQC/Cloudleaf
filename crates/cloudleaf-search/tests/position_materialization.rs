use cloudleaf_pinyin::{PinyinScheme, SchemeConfig};
use cloudleaf_search::{
    Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, search_graph,
};
use std::collections::BTreeMap;

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEADER: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";

#[test]
fn bounded_position_buckets_keep_every_feature_and_identity() {
    let source = format!(
        "{HEADER}a\tni\t你\t7\tmanual\t\tfixture\ta\tMIT\n\
         b\tni\t你\t1\tmanual\t\tfixture\tb\tMIT\n\
         c\tni\t尼\t\tunknown\t\tfixture\tc\tMIT\n\
         d\tni\t拟\t\tunknown\t\tfixture\td\tMIT\n\
         e\thao\t好\t15\tmanual\t\tfixture\te\tMIT\n\
         f\thao\t号\t\tunknown\t\tfixture\tf\tMIT\n"
    );
    let lexicon = Lexicon::from_tsv(&source)
        .unwrap()
        .with_user_layer(HEADER, &BTreeMap::from([("c".into(), 1)]))
        .unwrap();
    let artifact = format!(
        "# cloudleaf-language-v1\nlexicon_sha256\t{SHA}\nsource_sha256\t{SHA}\nsource_id\tcloudleaf:mechanical\nlicense_id\tMIT\ntokenization\tstable-entry-ids\nusage\tmechanical\nkind\tlexical\ntraining_split\ttrain\nprevious\tnext\tcount\n\
         <lexical>\ta\t31\n<lexical>\tb\t31\n<lexical>\te\t1\n"
    );
    let snapshot = LanguageModelSnapshot::from_tsv(&artifact, &lexicon, SHA).unwrap();
    let abbreviated = SchemeConfig {
        scheme: PinyinScheme::AbbreviatedPinyin,
        ..SchemeConfig::default()
    };

    for (input, scheme) in [
        ("ninih", SchemeConfig::default()),
        ("nnh", abbreviated.clone()),
        ("ninih", abbreviated),
    ] {
        for ranker in [Ranker::Structural, Ranker::Weighted, Ranker::Lexical] {
            for output_k in [1, 2, 7] {
                let reference_config = Config {
                    method: Method::Reference,
                    ranker,
                    output_k,
                    ..Config::default()
                };
                let graph = Lattice::build_with_scheme(&lexicon, input, &reference_config, &scheme)
                    .unwrap();
                let position_config = Config {
                    method: Method::PositionDag,
                    ..reference_config.clone()
                };
                let run = |config| {
                    search_graph(&lexicon, &graph, config, Some(snapshot.model()), None).unwrap()
                };
                let reference = run(&reference_config);
                let result = run(&position_config);
                assert!(result.guaranteed && reference.guaranteed);
                assert!(!result.candidates.is_empty());
                assert_eq!(result.raw_fallback, reference.raw_fallback);
                assert_eq!(
                    result.candidates, reference.candidates,
                    "{input}/{ranker:?}/{output_k}"
                );
                assert!(result.statistics.expansions <= reference.statistics.expansions);
            }
        }
    }
}
