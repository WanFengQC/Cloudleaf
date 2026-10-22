//! 独立评估宿主；文件IO、模型计数及输出全部位于核心搜索调用之外。
use cloudleaf_search::{BigramModel, Config, Lattice, Lexicon, Method, Ranker, search_graph};
use std::{error::Error, fs, hint::black_box, time::Instant};

fn quote(value: &str) -> String {
    let mut result = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => result.push_str(&format!("\\u{:04x}", c as u32)),
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

fn table<'a>(
    text: &'a str,
    magic: &str,
    header: &str,
    width: usize,
) -> Result<Vec<Vec<&'a str>>, Box<dyn Error>> {
    let mut lines = text.lines();
    if lines.next() != Some(magic) || lines.next() != Some(header) {
        return Err("评估资源版本或表头无效".into());
    }
    let rows: Vec<Vec<_>> = lines.map(|line| line.split('\t').collect()).collect();
    if rows.iter().any(|row| row.len() != width) {
        return Err("评估资源字段无效".into());
    }
    Ok(rows)
}

fn percentile(samples: &[u128], percentage: usize) -> u128 {
    samples[(samples.len() * percentage).div_ceil(100).saturating_sub(1)]
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--inspect-lexicon" {
        let started = Instant::now();
        let lexicon = Lexicon::from_tsv(&fs::read_to_string(&args[2])?)?;
        println!(
            "{{\"entries\":{},\"trie_nodes\":{},\"load_ns\":{},\"missing_frequencies\":{}}}",
            lexicon.entries().len(),
            lexicon.trie_nodes(),
            started.elapsed().as_nanos(),
            lexicon
                .entries()
                .iter()
                .filter(|e| e.frequency.is_none())
                .count()
        );
        return Ok(());
    }
    if args.len() != 10 {
        return Err(
            "参数：lexicon train cases method ranker iterations warmup beam output_k".into(),
        );
    }
    let method = match args[4].as_str() {
        "reference" => Method::Reference,
        "exact" => Method::ExactDag,
        "beam" => Method::Beam,
        _ => return Err("搜索方法无效".into()),
    };
    let ranker = match args[5].as_str() {
        "structural" => Ranker::Structural,
        "weighted" => Ranker::Weighted,
        "bigram" => Ranker::Bigram,
        "context" => Ranker::ContextRerank,
        _ => return Err("评分方法无效".into()),
    };
    let iterations: usize = args[6].parse()?;
    let warmup: usize = args[7].parse()?;
    if !(1..=10000).contains(&iterations) || warmup > 1000 {
        return Err("测量次数超限".into());
    }
    let config = Config {
        method,
        ranker,
        beam_width: args[8].parse()?,
        output_k: args[9].parse()?,
        ..Config::default()
    };
    config.validate()?;
    let started = Instant::now();
    let lexicon = Lexicon::from_tsv(&fs::read_to_string(&args[1])?)?;
    let load_ns = started.elapsed().as_nanos();
    let started = Instant::now();
    let training = fs::read_to_string(&args[2])?;
    let rows = table(
        &training,
        "# cloudleaf-search-training-v1",
        "family\tsequence\tmultiplicity",
        3,
    )?;
    let mut sequences = Vec::new();
    for row in rows {
        let multiplicity: usize = row[2].parse()?;
        if multiplicity == 0 || multiplicity > 10000 || sequences.len() + multiplicity > 100000 {
            return Err("训练计数超限".into());
        }
        let tokens: Vec<String> = row[1].split(' ').map(str::to_owned).collect();
        if tokens
            .iter()
            .any(|token| !lexicon.entries().iter().any(|entry| &entry.id == token))
        {
            return Err("训练词条不在快照中".into());
        }
        sequences.extend(std::iter::repeat_n(tokens, multiplicity));
    }
    let model = BigramModel::train(&sequences)?;
    let training_ns = started.elapsed().as_nanos();
    let cases = fs::read_to_string(&args[3])?;
    let rows = table(
        &cases,
        "# cloudleaf-search-cases-v1",
        "id\tsplit\tfamily\tinput\tleft_word\tkind",
        6,
    )?;
    let mut outputs = Vec::new();
    for row in rows {
        let left = (!row[4].is_empty()).then_some(row[4]);
        let query = || {
            let graph = Lattice::build(&lexicon, row[3], &config)?;
            search_graph(&lexicon, &graph, &config, Some(&model), left)
        };
        for _ in 0..warmup {
            let _ = black_box(query());
        }
        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            let _ = black_box(query());
            samples.push(start.elapsed().as_nanos());
        }
        samples.sort_unstable();
        let (status, result_json) = match query() {
            Ok(result) => {
                let candidates: Vec<_> = result.candidates.iter().map(|candidate| {
                    let ids = candidate.entry_ids.iter().map(|id| quote(id)).collect::<Vec<_>>().join(",");
                    let f = &candidate.features;
                    format!("{{\"text\":{},\"score\":{},\"ids\":[{}],\"features\":{{\"words\":{},\"prefixes\":{},\"frequency_units\":{},\"missing_frequencies\":{},\"bigram_units\":{},\"context_units\":{}}}}}", quote(&candidate.text), candidate.score, ids, f.words, f.prefixes, f.frequency_units, f.missing_frequencies, f.bigram_units, f.context_units)
                }).collect();
                let s = result.statistics;
                (
                    "ok",
                    format!(
                        "\"candidates\":[{}],\"guaranteed\":{},\"finite_pool_only\":{},\"fallback_preserved\":{},\"waiting\":{},\"statistics\":{{\"graph_edges\":{},\"expansions\":{},\"resident_high_water\":{},\"beam_pruned\":{},\"budget\":{}}}",
                        candidates.join(","),
                        result.guaranteed,
                        result.finite_pool_only,
                        result.raw_fallback == row[3],
                        result.waiting,
                        s.graph_edges,
                        s.expansions,
                        s.resident_high_water,
                        s.beam_pruned,
                        s.budget
                            .map(|b| quote(&format!("{b:?}")))
                            .unwrap_or("null".into())
                    ),
                )
            }
            Err(error) => (
                "rejected",
                format!("\"error_kind\":{}", quote(&format!("{:?}", error.kind))),
            ),
        };
        outputs.push(format!("{{\"id\":{},\"split\":{},\"family\":{},\"kind\":{},\"status\":{},\"p50_ns\":{},\"p95_ns\":{},\"p99_ns\":{},\"max_ns\":{},{}}}", quote(row[0]), quote(row[1]), quote(row[2]), quote(row[5]), quote(status), percentile(&samples, 50), percentile(&samples, 95), percentile(&samples, 99), samples.last().unwrap(), result_json));
    }
    println!(
        "{{\"language_accuracy_valid\":false,\"lexicon_entries\":{},\"trie_nodes\":{},\"load_ns\":{},\"training_ns\":{},\"training_sequences\":{},\"iterations_per_case\":{},\"warmup_per_case\":{},\"beam_width\":{},\"output_k\":{},\"cases\":[{}]}}",
        lexicon.entries().len(),
        lexicon.trie_nodes(),
        load_ns,
        training_ns,
        sequences.len(),
        iterations,
        warmup,
        config.beam_width,
        config.output_k,
        outputs.join(",")
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("评估宿主失败：{error}");
        std::process::exit(1);
    }
}
