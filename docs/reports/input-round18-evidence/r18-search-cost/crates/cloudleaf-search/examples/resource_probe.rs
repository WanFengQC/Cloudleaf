//! 真实词条资源实验宿主：读盘与报告位于搜索计时之外，不计算语言准确率。
use cloudleaf_search::{Config, Lattice, Lexicon, MatchKind, Method, Ranker, search_graph};
use std::{error::Error, fs, hint::black_box, time::Instant};

fn quote(value: &str) -> String {
    let mut output = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c.is_control() => output.push_str(&format!("\\u{:04x}", c as u32)),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}

fn strings(values: &[String]) -> String {
    values
        .iter()
        .map(|value| quote(value))
        .collect::<Vec<_>>()
        .join(",")
}

fn percentile(values: &[u128], percentage: usize) -> u128 {
    values[(values.len() * percentage).div_ceil(100) - 1]
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 11 {
        return Err("参数：runtime cases method beam iterations warmup max_edges max_expansions max_resident output_k".into());
    }
    let method = match args[3].as_str() {
        "reference" => Method::Reference,
        "exact" => Method::ExactDag,
        "position" => Method::PositionDag,
        "beam" => Method::Beam,
        _ => return Err("搜索方法无效".into()),
    };
    let iterations: usize = args[5].parse()?;
    let warmup: usize = args[6].parse()?;
    if !(1..=10000).contains(&iterations) || warmup > 1000 {
        return Err("测量次数超限".into());
    }
    let config = Config {
        method,
        ranker: Ranker::Structural,
        beam_width: args[4].parse()?,
        max_edges: args[7].parse()?,
        max_expansions: args[8].parse()?,
        max_resident_paths: args[9].parse()?,
        output_k: args[10].parse()?,
        ..Config::default()
    };
    config.validate()?;
    let configuration = format!(
        "{{\"method\":{},\"beam_width\":{},\"max_edges\":{},\"max_expansions\":{},\"max_resident_paths\":{},\"max_input_bytes\":{},\"max_letters\":{},\"output_k\":{},\"context_pool\":{},\"ranker\":\"Structural\"}}",
        quote(&args[3]),
        config.beam_width,
        config.max_edges,
        config.max_expansions,
        config.max_resident_paths,
        config.max_input_bytes,
        config.max_letters,
        config.output_k,
        config.context_pool
    );
    let started = Instant::now();
    let raw = fs::read_to_string(&args[1])?;
    if !raw.starts_with("# cloudleaf-lexicon-v2\n") {
        return Err("真实资源宿主仅接受v2".into());
    }
    let lexicon = Lexicon::from_tsv(&raw)?;
    let load_ns = started.elapsed().as_nanos();
    drop(raw);
    let cases = fs::read_to_string(&args[2])?;
    let mut lines = cases.lines();
    if lines.next() != Some("# cloudleaf-search-resource-cases-v1")
        || lines.next() != Some("id\tfamily\tinput\tkind")
    {
        return Err("资源探针版本或表头无效".into());
    }
    let mut output = Vec::new();
    for row in lines {
        let fields: Vec<_> = row.split('\t').collect();
        if fields.len() != 4 {
            return Err("资源探针字段数无效".into());
        }
        let query = || {
            let graph = Lattice::build(&lexicon, fields[2], &config)?;
            let result = search_graph(&lexicon, &graph, &config, None, None)?;
            Ok::<_, cloudleaf_search::SearchError>((graph, result))
        };
        for _ in 0..warmup {
            let _ = black_box(query());
        }
        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let started = Instant::now();
            let _ = black_box(query());
            samples.push(started.elapsed().as_nanos());
        }
        let samples_json = samples
            .iter()
            .map(u128::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut sorted_samples = samples.clone();
        sorted_samples.sort_unstable();
        let details = match query() {
            Ok((graph, result)) => {
                // 整词边仅作为结构对照；图截断时不声称整词边集合完整。
                let mut whole: Vec<_> = graph
                    .edges()
                    .iter()
                    .filter(|edge| edge.start == 0 && edge.end == graph.letters().len())
                    .map(|edge| {
                        let entry = &lexicon.entries()[edge.entry];
                        let score = if edge.kind == MatchKind::Exact {
                            -100
                        } else {
                            -400
                        };
                        (score, entry.id.clone(), entry.text.clone())
                    })
                    .collect();
                whole.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
                let mut unique = std::collections::BTreeSet::new();
                whole.retain(|value| unique.insert(value.2.clone()));
                whole.truncate(config.output_k);
                let whole = whole
                    .iter()
                    .map(|(_, _, text)| text.clone())
                    .collect::<Vec<_>>();
                let candidates = result.candidates.iter().map(|candidate| {
                    let f = &candidate.features;
                    // 输出每个词条的来源记录和许可，派生候选正文不会被重新声明为项目MIT。
                    let records = candidate.edge_indices.iter().map(|index| {
                        let entry = &lexicon.entries()[graph.edges()[*index].entry];
                        format!("{{\"id\":{},\"source\":{},\"source_record_id\":{},\"license_id\":{}}}", quote(&entry.id), quote(&entry.source), quote(&entry.source_record_id), quote(&entry.license_id))
                    }).collect::<Vec<_>>().join(",");
                    format!("{{\"text\":{},\"score\":{},\"ids\":[{}],\"records\":[{}],\"features\":{{\"words\":{},\"prefixes\":{},\"frequency_units\":{},\"missing_frequencies\":{},\"bigram_units\":{},\"context_units\":{}}}}}", quote(&candidate.text), candidate.score, strings(&candidate.entry_ids), records, f.words, f.prefixes, f.frequency_units, f.missing_frequencies, f.bigram_units, f.context_units)
                }).collect::<Vec<_>>().join(",");
                let s = result.statistics;
                format!(
                    "\"status\":\"ok\",\"candidates\":[{}],\"whole_entry_candidates\":[{}],\"whole_entry_complete\":{},\"fallback_preserved\":{},\"waiting\":{},\"guaranteed\":{},\"statistics\":{{\"graph_edges\":{},\"expansions\":{},\"resident_high_water\":{},\"beam_pruned\":{},\"budget\":{}}}",
                    candidates,
                    strings(&whole),
                    graph.budget().is_none(),
                    result.raw_fallback == fields[2],
                    result.waiting,
                    result.guaranteed,
                    s.graph_edges,
                    s.expansions,
                    s.resident_high_water,
                    s.beam_pruned,
                    s.budget
                        .map(|value| quote(&format!("{value:?}")))
                        .unwrap_or("null".into())
                )
            }
            Err(error) => format!(
                "\"status\":\"rejected\",\"error_kind\":{},\"candidates\":[],\"whole_entry_candidates\":[],\"whole_entry_complete\":false,\"guaranteed\":false,\"fallback_preserved\":true",
                quote(&format!("{:?}", error.kind))
            ),
        };
        output.push(format!("{{\"id\":{},\"family\":{},\"input\":{},\"kind\":{},\"raw_fallback\":{},\"samples_ns\":[{}],\"p50_ns\":{},\"p95_ns\":{},\"p99_ns\":{},\"max_ns\":{},{}}}", quote(fields[0]), quote(fields[1]), quote(fields[2]), quote(fields[3]), quote(fields[2]), samples_json, percentile(&sorted_samples, 50), percentile(&sorted_samples, 95), percentile(&sorted_samples, 99), sorted_samples.last().unwrap(), details));
    }
    println!(
        "{{\"format\":\"cloudleaf-search-resource-observation-v2\",\"configuration\":{},\"language_accuracy_valid\":false,\"frequency_feature_enabled\":false,\"language_model_loaded\":false,\"lexicon_entries\":{},\"trie_nodes\":{},\"load_ns\":{},\"missing_frequencies\":{},\"iterations_per_case\":{},\"warmup_per_case\":{},\"cases\":[{}]}}",
        configuration,
        lexicon.entries().len(),
        lexicon.trie_nodes(),
        load_ns,
        lexicon
            .entries()
            .iter()
            .filter(|entry| entry.frequency.is_none())
            .count(),
        iterations,
        warmup,
        output.join(",")
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("真实词条资源实验失败：{error}");
        std::process::exit(1);
    }
}
