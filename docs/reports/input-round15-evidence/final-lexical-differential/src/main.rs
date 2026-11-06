use cloudleaf_search::{Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, search_graph};

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn main() {
    let rows = [
        ("a", "甲"), ("a", "乙"), ("a", "甲"), ("an", "甲"),
        ("an", "丙"), ("na", "乙"), ("na", "甲乙"), ("a na", "甲乙"),
        ("an a", "甲甲"), ("nan", "丙"), ("a a", "甲甲"), ("a a a", "甲甲甲"),
        ("an an", "丙丙"), ("na na", "乙乙"), ("a na", "甲乙"), ("a a", "甲甲"),
    ];
    let mut queries = 0;
    let mut comparisons = 0;
    for seed in 0..16 {
        let mut text = String::from("# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n");
        for (i, (pinyin, word)) in rows.iter().enumerate() {
            text.push_str(&format!("word-{i:02}\t{pinyin}\t{word}\t\tunknown\t\tmechanical\trecord-{i}\tMIT\n"));
        }
        let lexicon = Lexicon::from_tsv(&text).unwrap();
        let mut artifact = format!("# cloudleaf-language-v1\nlexicon_sha256\t{SHA}\nsource_sha256\t{SHA}\nsource_id\tmechanical\nlicense_id\tMIT\ntokenization\tstable-entry-ids\nusage\tmechanical\nkind\tlexical\ntraining_split\ttrain\nprevious\tnext\tcount\n");
        for (i, entry) in lexicon.entries().iter().enumerate() {
            if (i + seed) % 5 != 0 {
                let count = match seed % 4 {
                    0 => 1,
                    1 => 2_u64.pow(((i + seed) % 12) as u32) - 1 + u64::from((i + seed) % 12 == 0),
                    _ => 1 + ((seed * 751 + i * 1013) % 10_000) as u64,
                };
                artifact.push_str(&format!("<lexical>\t{}\t{count}\n", entry.id));
            }
        }
        let model = LanguageModelSnapshot::from_tsv(&artifact, &lexicon, SHA).unwrap();
        assert!(lexicon.entries().iter().all(|entry| entry.frequency.is_none()));
        for raw in ["a", "aa", "aaa", "aaaa", "aaaaa", "aaaaaa", "an", "ana", "anana", "ananan", "nana", "nanana", "a'na", "an'a", "a'a'a", "a'", "q", "aanana"] {
            for k in [1, 2, 5, 10, 20] {
                queries += 1;
                let config = Config { method: Method::Reference, ranker: Ranker::Lexical, output_k: k,
                    beam_width: 4096, max_expansions: 2_000_000, max_resident_paths: 200_000,
                    ..Config::default() };
                let graph = Lattice::build(&lexicon, raw, &config).unwrap();
                let reference = search_graph(&lexicon, &graph, &config, Some(model.model()), None).unwrap();
                assert!(reference.guaranteed, "reference budget seed={seed} raw={raw:?} k={k}");
                assert!(reference.statistics.budget.is_none());
                for method in [Method::ExactDag, Method::PositionDag, Method::Beam] {
                    comparisons += 1;
                    let result = search_graph(&lexicon, &graph, &Config { method, ..config.clone() }, Some(model.model()), None).unwrap();
                    assert!(result.guaranteed, "budget/prune seed={seed} raw={raw:?} k={k} method={method:?}");
                    assert_eq!(result.candidates, reference.candidates, "seed={seed} raw={raw:?} k={k} method={method:?}");
                    assert_eq!(result.waiting, reference.waiting);
                    assert_eq!(result.raw_fallback, reference.raw_fallback);
                }
            }
        }
    }
    println!("{{\"seeds\":16,\"queries\":{queries},\"method_comparisons\":{comparisons},\"all_candidate_path_feature_results_equal\":true,\"budget_failures\":0,\"formal_language_metrics\":null}}");
}
