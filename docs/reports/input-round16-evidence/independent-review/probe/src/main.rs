use cloudleaf_search::{Config, LanguageModelSnapshot, Lattice, Lexicon, Ranker, search_graph};

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn lexicon(extra: bool, text: &str) -> Lexicon {
    let mut words = format!("# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\tma\t{text}\t\tunknown\t\tfixture\ta\tMIT\n");
    if extra { words.push_str("b\tna\t乙\t\tunknown\t\tfixture\tb\tMIT\n"); }
    Lexicon::from_tsv(&words).unwrap()
}

fn main() {
    let first = lexicon(false, "甲");
    let header = format!("# cloudleaf-language-v1\nlexicon_sha256\t{SHA}\nsource_sha256\t{SHA}\nsource_id\tmechanical\nlicense_id\tMIT\ntokenization\tunicode-scalar-v1\nusage\tmechanical\nkind\tcharacter_bigram\ntraining_split\ttrain\nprevious\tnext\tcount\n");
    let trained = vec![vec!["甲".to_string()]; 20];
    let model = LanguageModelSnapshot::train(&header, &first, SHA, &trained).unwrap();
    let config = Config { ranker: Ranker::CharacterBigram, ..Config::default() };
    let larger = lexicon(true, "甲");
    let larger_graph = Lattice::build(&larger, "na", &config).unwrap();
    let larger_result = std::panic::catch_unwind(|| search_graph(&larger, &larger_graph, &config, Some(model.model()), None));
    match &larger_result {
        Err(_) => println!("larger_dictionary=PANIC"),
        Ok(Err(e)) => println!("larger_dictionary=REJECTED {e:?}"),
        Ok(Ok(r)) => println!("larger_dictionary=ACCEPTED {r:?}"),
    }
    let changed = lexicon(false, "乙");
    let changed_graph = Lattice::build(&changed, "ma", &config).unwrap();
    let changed_result = search_graph(&changed, &changed_graph, &config, Some(model.model()), None);
    println!("same_size_changed_text={changed_result:?}");
    assert!(larger_result.is_ok_and(|r| r.is_err()), "公共入口应拒绝模型预计算词典不符，不能panic");
    assert!(changed_result.is_err(), "同长度不同正文词典不能静默使用旧EntryScore");
}
