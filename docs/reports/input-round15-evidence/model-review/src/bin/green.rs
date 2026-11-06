use cloudleaf_search::{LanguageModelSnapshot, Lexicon};

fn main() {
    let lexicon = Lexicon::from_tsv("# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\tma\t甲\t\tunknown\t\tfixture\ta\tMIT\nb\tna\t乙\t\tunknown\t\tfixture\tb\tMIT\n").unwrap();
    let sha = "a".repeat(64);
    let header = format!("# cloudleaf-language-v1\nlexicon_sha256\t{sha}\nsource_sha256\t{sha}\nsource_id\tfixture\nlicense_id\tMIT\ntokenization\tstable-entry-ids\nusage\tmechanical\nkind\tbigram\ntraining_split\ttrain\nprevious\tnext\tcount\n");
    let sequences = vec![vec![String::from("a")]];
    let model = LanguageModelSnapshot::train(&header, &lexicon, &sha, &sequences).unwrap();
    assert_eq!(model.metadata().usage, "mechanical");
    let poisoned_header = format!("{header}<bos>\tb\t100\nb\t<eos>\t100\n");
    assert!(LanguageModelSnapshot::train(&poisoned_header, &lexicon, &sha, &sequences).is_err());
    let disconnected = format!("{header}<bos>\ta\t1\na\t<eos>\t1\nb\tb\t100\n");
    assert!(LanguageModelSnapshot::from_tsv(&disconnected, &lexicon, &sha).is_err());
    let restored = LanguageModelSnapshot::from_tsv(&model.to_tsv(), &lexicon, &sha).unwrap();
    assert_eq!(restored.to_tsv(), model.to_tsv());
    println!("metadata-read-only=true training-header-injection-rejected=true disconnected-cycle-rejected=true valid-training-roundtrip=true");
}
