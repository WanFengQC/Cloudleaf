use cloudleaf_search::{LanguageModelSnapshot, Lexicon};

fn main() {
    let lexicon = Lexicon::from_tsv("# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\tma\t甲\t\tunknown\t\tfixture\ta\tMIT\nb\tna\t乙\t\tunknown\t\tfixture\tb\tMIT\n").unwrap();
    let sha = "a".repeat(64);
    let header = format!("# cloudleaf-language-v1\nlexicon_sha256\t{sha}\nsource_sha256\t{sha}\nsource_id\tfixture\nlicense_id\tMIT\ntokenization\tstable-entry-ids\nusage\tmechanical\nkind\tbigram\ntraining_split\ttrain\nprevious\tnext\tcount\n");
    let sequences = vec![vec![String::from("a")]];
    let model = LanguageModelSnapshot::train(&header, &lexicon, &sha, &sequences).unwrap();
    println!("clean-training:\n{}", model.to_tsv());
    let poisoned_header = format!("{header}<bos>\tb\t100\nb\t<eos>\t100\n");
    let injected = LanguageModelSnapshot::train(&poisoned_header, &lexicon, &sha, &sequences);
    println!("training-header-injection-accepted={}", injected.is_ok());
    if let Ok(model) = injected { println!("injected-training:\n{}", model.to_tsv()); }
    let disconnected = format!("{header}<bos>\ta\t1\na\t<eos>\t1\nb\tb\t100\n");
    println!("disconnected-cycle-accepted={}", LanguageModelSnapshot::from_tsv(&disconnected, &lexicon, &sha).is_ok());
    let mut mutable = model;
    mutable.metadata.usage = String::from("production");
    mutable.metadata.lexicon_sha256 = "b".repeat(64);
    println!("post-validation-mutation:\n{}", mutable.to_tsv());
}
