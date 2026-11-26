//! 固定研发样本评估。正式 gold 审批由独立执行器处理。
use cloudleaf_input::{
    CorrectionConfig, DoublePinyinLayout, FuzzyDirection, FuzzyPair, FuzzyRule, PinyinScheme,
    SchemeConfig,
};
use cloudleaf_search::{
    BigramModel, Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, ResearchRanker,
    SearchResult, WordContextModel, expected_reachability, search_graph,
};
use ring::digest::{SHA256, digest};
use std::error::Error;
use std::fs;
use std::time::Instant;

type Outcome<T> = Result<T, Box<dyn Error>>;
const MAX_INPUT: usize = 64;
const TOP_K: usize = 10;
const POOL: usize = 50;

fn verified(path: &str, expected: &str) -> Outcome<String> {
    let text = fs::read_to_string(path)?;
    let sha: String = digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if sha != expected {
        return Err("输入资源 SHA-256 不匹配".into());
    }
    Ok(text)
}

fn scheme(kind: &str) -> Option<SchemeConfig> {
    let mut value = SchemeConfig::default();
    match kind {
        "full" => {}
        "abbreviated" | "mixed" => value.scheme = PinyinScheme::AbbreviatedPinyin,
        "xiaohe" => value.scheme = PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
        "fuzzy-nl" => value.fuzzy.push(FuzzyRule {
            pair: FuzzyPair::NL,
            direction: FuzzyDirection::Both,
        }),
        "correction" => value.correction = Some(CorrectionConfig::default()),
        _ => return None,
    }
    Some(value)
}

fn config(ranker: Ranker, k: usize) -> Config {
    Config {
        ranker,
        output_k: k,
        context_pool: k.max(20),
        max_input_bytes: MAX_INPUT,
        max_letters: MAX_INPUT,
        max_expansions: 200_000,
        max_resident_paths: 50_000,
        method: if matches!(ranker, Ranker::Structural | Ranker::Lexical) {
            Method::PositionDag
        } else {
            Method::ExactDag
        },
        ..Config::default()
    }
}

fn result(id: &str, mode: &str, outcome: &SearchResult, elapsed: u128, pool: usize) {
    println!(
        "RUN\t{id}\t{mode}\t{elapsed}\t{:?}\t{}\t{}\t{}\t{}\t{pool}",
        outcome.statistics.budget,
        outcome.guaranteed,
        outcome.statistics.graph_edges,
        outcome.statistics.expansions,
        outcome.statistics.resident_high_water
    );
}

fn baseline(
    id: &str,
    lexicon: &Lexicon,
    graph: &Lattice<'_>,
    modes: [(&str, Ranker, Option<&BigramModel>); 4],
) {
    for (mode, ranker, language) in modes {
        let start = Instant::now();
        match search_graph(lexicon, graph, &config(ranker, TOP_K), language, None) {
            Ok(value) => {
                result(
                    id,
                    mode,
                    &value,
                    start.elapsed().as_nanos(),
                    value.candidates.len(),
                );
                for (index, candidate) in value.candidates.iter().enumerate() {
                    println!(
                        "CAND\t{id}\t{mode}\t{}\t{}\t{}\t{:?}\t{:?}\t{:?}",
                        index + 1,
                        candidate.text,
                        candidate.score,
                        candidate.features,
                        candidate.entry_ids,
                        candidate.edge_indices
                    );
                }
            }
            Err(error) => println!("ERROR\t{id}\t{mode}\t{error}"),
        }
    }
}

fn main() -> Outcome<()> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let character_pool = args.last().is_some_and(|value| value == "--character-pool");
    if character_pool {
        args.pop();
    }
    let [
        runtime,
        lex_sha,
        word,
        word_sha,
        lexical,
        lexical_sha,
        addone,
        addone_sha,
        witten,
        witten_sha,
        input,
    ] = args.as_slice()
    else {
        return Err(
            "usage: research_rank RUNTIME SHA WORD SHA LEXICAL SHA ADDONE SHA WITTEN SHA CASES_TSV"
                .into(),
        );
    };
    let begun = Instant::now();
    let lexicon = Lexicon::from_tsv(&verified(runtime, lex_sha)?)?;
    let lexicon_load = begun.elapsed().as_nanos();
    let begun = Instant::now();
    let words = WordContextModel::from_tsv(&verified(word, word_sha)?, &lexicon, lex_sha)?;
    let word_load = begun.elapsed().as_nanos();
    let begun = Instant::now();
    let lexical =
        LanguageModelSnapshot::from_tsv(&verified(lexical, lexical_sha)?, &lexicon, lex_sha)?;
    let addone =
        LanguageModelSnapshot::from_tsv(&verified(addone, addone_sha)?, &lexicon, lex_sha)?;
    let witten =
        LanguageModelSnapshot::from_tsv(&verified(witten, witten_sha)?, &lexicon, lex_sha)?;
    let prior_load = begun.elapsed().as_nanos();
    println!(
        "SOURCE\t{lex_sha}\t{word_sha}\t{}\t{}\t{}\t{}\t{}\t{}\t{lexicon_load}\t{word_load}\t{prior_load}",
        words.corpus_sha256(),
        lexicon.entries().len(),
        words.estimated_bytes(),
        words.vocabulary_size(),
        words.pair_count(),
        witten.estimated_bytes()
    );
    let samples = fs::read_to_string(input)?;
    if samples.len() > 1024 * 1024 {
        return Err("研发样本文件超限".into());
    }
    let modes = [
        ("structural", Ranker::Structural, None),
        ("lexical", Ranker::Lexical, Some(lexical.model())),
        (
            "character-addone",
            Ranker::CharacterBigram,
            Some(addone.model()),
        ),
        (
            "character-witten",
            Ranker::CharacterBigram,
            Some(witten.model()),
        ),
    ];
    let pool_ranker = if character_pool {
        Ranker::CharacterBigram
    } else {
        Ranker::Structural
    };
    let pool_config = config(pool_ranker, POOL);
    let pool_model = character_pool.then(|| witten.model());
    for line in samples
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let mut row = line.split('\t');
        let [Some(id), Some(kind), Some(raw), Some(expected), None] =
            std::array::from_fn(|_| row.next())
        else {
            return Err("样本应含标识、方案、拼音及正文四列".into());
        };
        println!("CASE\t{id}\t{kind}\t{raw}\t{expected}");
        let Some(scheme) = scheme(kind) else {
            println!("REACH\t{id}\tunsupported");
            continue;
        };
        let begin = Instant::now();
        let graph = match Lattice::build_with_scheme(&lexicon, raw, &pool_config, &scheme) {
            Ok(value) => value,
            Err(error) => {
                println!("REACH\t{id}\tinvalid");
                println!("ERROR\t{id}\tlattice\t{error}");
                continue;
            }
        };
        println!("LATTICE\t{id}\t{}", begin.elapsed().as_nanos());
        let reached = if expected.is_empty() {
            "no-target"
        } else {
            match expected_reachability(&graph, expected, 200_000) {
                Some(true) => "yes",
                Some(false) => "no",
                None => "incomplete",
            }
        };
        println!("REACH\t{id}\t{reached}");
        baseline(id, &lexicon, &graph, modes);
        let pool_start = Instant::now();
        let pool = search_graph(&lexicon, &graph, &pool_config, pool_model, None)?;
        let pool_cost = pool_start.elapsed().as_nanos();
        let pool_size = pool.candidates.len();
        let pool_hit = if expected.is_empty() {
            "-"
        } else if pool
            .candidates
            .iter()
            .any(|candidate| candidate.text == expected)
        {
            "yes"
        } else {
            "no"
        };
        println!(
            "POOL\t{id}\t{pool_size}\t{pool_hit}\t{:?}\t{}",
            pool.statistics.budget, pool.guaranteed
        );
        for (name, ranker, char_model) in [
            ("unigram", ResearchRanker::Unigram, None),
            ("word", ResearchRanker::Word, None),
            (
                "finite-character",
                ResearchRanker::Character,
                Some(witten.model()),
            ),
            ("mixed", ResearchRanker::Mixed, Some(witten.model())),
        ] {
            let begin = Instant::now();
            let ordered = words.rank_pool(&graph, &pool, ranker, char_model)?;
            result(
                id,
                name,
                &pool,
                begin.elapsed().as_nanos() + pool_cost,
                pool_size,
            );
            for (index, row) in ordered.iter().enumerate() {
                let candidate = &pool.candidates[row.pool_index];
                println!(
                    "CAND\t{id}\t{name}\t{}\t{}\t{}\t{:?} {:?}\t{:?}\t{:?}",
                    index + 1,
                    candidate.text,
                    row.final_units,
                    row,
                    candidate.features,
                    candidate.entry_ids,
                    candidate.edge_indices
                );
            }
        }
    }
    Ok(())
}
