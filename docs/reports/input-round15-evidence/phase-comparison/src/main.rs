//! 显式阶段诊断；默认旧入口与观测入口逐次配对，全部数据和报告IO在计时外。
use cloudleaf_search::{
    Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Phase, Profile, Ranker, SearchError, SearchResult,
    profile_query, search_graph,
};
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

fn details(value: &Result<(Lattice<'_>, SearchResult), SearchError>) -> String {
    match value {
        Err(error) => format!("{{\"status\":\"rejected\",\"error_kind\":{},\"error_message\":{},\"candidates\":[]}}", quote(&format!("{:?}", error.kind)), quote(error.message)),
        Ok((_, result)) => {
            let candidates = result.candidates.iter().map(|candidate| {
                let f = &candidate.features;
                format!("{{\"text\":{},\"score\":{},\"entry_ids\":[{}],\"edge_indices\":{:?},\"features\":{{\"words\":{},\"prefixes\":{},\"frequency_units\":{},\"missing_frequencies\":{},\"lexical_units\":{},\"lexical_observed\":{},\"lexical_missing\":{},\"bigram_units\":{},\"context_units\":{}}}}}",
                    quote(&candidate.text), candidate.score, strings(&candidate.entry_ids), candidate.edge_indices,
                    f.words, f.prefixes, f.frequency_units, f.missing_frequencies, f.lexical_units, f.lexical_observed, f.lexical_missing, f.bigram_units, f.context_units)
            }).collect::<Vec<_>>().join(",");
            let s = &result.statistics;
            format!("{{\"status\":\"ok\",\"candidates\":[{candidates}],\"raw_fallback\":{},\"waiting\":{},\"guaranteed\":{},\"finite_pool_only\":{},\"statistics\":{{\"graph_edges\":{},\"expansions\":{},\"resident_high_water\":{},\"beam_pruned\":{},\"budget\":{}}}}}",
                quote(&result.raw_fallback), result.waiting, result.guaranteed, result.finite_pool_only,
                s.graph_edges, s.expansions, s.resident_high_water, s.beam_pruned,
                s.budget.map(|value| quote(&format!("{value:?}"))).unwrap_or("null".into()))
        }
    }
}

fn equal(
    left: &Result<(Lattice<'_>, SearchResult), SearchError>,
    right: &Result<(Lattice<'_>, SearchResult), SearchError>,
) -> bool {
    match (left, right) {
        (Err(left), Err(right)) => left == right,
        (Ok((a, x)), Ok((b, y))) => {
            x == y
                && a.raw() == b.raw()
                && a.letters() == b.letters()
                && a.raw_offsets() == b.raw_offsets()
                && a.edges() == b.edges()
                && a.waiting() == b.waiting()
                && a.budget() == b.budget()
        }
        _ => false,
    }
}

fn graph_diagnostic(value: &Result<(Lattice<'_>, SearchResult), SearchError>) -> String {
    match value {
        Err(error) => format!("{{\"error_kind\":{}}}", quote(&format!("{:?}", error.kind))),
        // 只保存本次有界图；绝不Debug整份被借用的大词典。
        Ok((graph, _)) => format!(
            "{{\"raw\":{},\"letters\":{},\"raw_offsets\":{:?},\"edges_debug\":{},\"waiting\":{},\"budget\":{}}}",
            quote(graph.raw()),
            quote(graph.letters()),
            graph.raw_offsets(),
            quote(&format!("{:?}", graph.edges())),
            graph.waiting(),
            graph
                .budget()
                .map(|value| quote(&format!("{value:?}")))
                .unwrap_or("null".into())
        ),
    }
}
fn profile_json(profile: &Profile) -> String {
    let phases = [
        Phase::QueryTotal,
        Phase::Normalize,
        Phase::Lattice,
        Phase::SearchValidate,
        Phase::Traversal,
        Phase::Scoring,
        Phase::BucketSortDedup,
        Phase::FinalSortDedup,
        Phase::ContextRerank,
        Phase::Materialize,
    ];
    format!(
        "{{\"schema_version\":2,\"clock_reads\":{},\"phases\":{{{}}}}}",
        profile.clock_reads(),
        phases
            .iter()
            .map(|phase| format!(
                "{}:{}",
                quote(phase.name()),
                profile
                    .phase(*phase)
                    .map(|value| format!("{{\"ns\":{},\"calls\":{}}}", value.nanos, value.calls))
                    .unwrap_or("null".into())
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 14 {
        return Err("参数：runtime cases method ranker model lexicon_sha beam iterations warmup max_edges max_expansions max_resident output_k".into());
    }
    let method = match args[3].as_str() {
        "reference" => Method::Reference,
        "exact" => Method::ExactDag,
        "position" => Method::PositionDag,
        "beam" => Method::Beam,
        _ => return Err("未知策略".into()),
    };
    let iterations: usize = args[8].parse()?;
    let warmup: usize = args[9].parse()?;
    if !(1..=10000).contains(&iterations) || warmup > 1000 {
        return Err("次数超限".into());
    }
    let config = Config {
        method,
        ranker: match args[4].as_str() {
            "structural" => Ranker::Structural,
            "lexical" => Ranker::Lexical,
            _ => return Err("未知评分".into()),
        },
        beam_width: args[7].parse()?,
        max_edges: args[10].parse()?,
        max_expansions: args[11].parse()?,
        max_resident_paths: args[12].parse()?,
        output_k: args[13].parse()?,
        ..Config::default()
    };
    config.validate()?;
    let started = Instant::now();
    let raw = fs::read_to_string(&args[1])?;
    if !raw.starts_with("# cloudleaf-lexicon-v2\n") {
        return Err("仅接受v2词库".into());
    }
    let lexicon = Lexicon::from_tsv(&raw)?;
    drop(raw);
    let load_ns = started.elapsed().as_nanos();
    let model_text = fs::read_to_string(&args[5])?;
    let snapshot = LanguageModelSnapshot::from_tsv(&model_text, &lexicon, &args[6])?;
    drop(model_text);
    let model = Some(snapshot.model());
    let cases = fs::read_to_string(&args[2])?;
    let mut lines = cases.lines();
    if lines.next() != Some("# cloudleaf-search-resource-cases-v1")
        || lines.next() != Some("id\tfamily\tinput\tkind")
    {
        return Err("案例表头错误".into());
    }
    let mut output = Vec::new();
    for row in lines {
        let fields: Vec<_> = row.split('\t').collect();
        if fields.len() != 4 {
            return Err("案例字段错误".into());
        }
        let plain = || {
            Lattice::build(&lexicon, fields[2], &config).and_then(|graph| {
                search_graph(&lexicon, &graph, &config, model, None).map(|result| (graph, result))
            })
        };
        let observed = || profile_query(&lexicon, fields[2], &config, model, None);
        for _ in 0..warmup {
            let _ = black_box(plain());
            let _ = black_box(observed());
        }
        let mut samples = Vec::new();
        let mut last = None;
        for index in 0..iterations {
            // 奇偶轮反转顺序；两个入口结果在计时外逐次完整核对。
            let (left, plain_ns, right, profile, profile_outer_ns) = if index % 2 == 0 {
                let start = Instant::now();
                let left = black_box(plain());
                let plain_ns = start.elapsed().as_nanos();
                let start = Instant::now();
                let (right, profile) = black_box(observed());
                (left, plain_ns, right, profile, start.elapsed().as_nanos())
            } else {
                let start = Instant::now();
                let (right, profile) = black_box(observed());
                let profile_outer_ns = start.elapsed().as_nanos();
                let start = Instant::now();
                let left = black_box(plain());
                let plain_ns = start.elapsed().as_nanos();
                (left, plain_ns, right, profile, profile_outer_ns)
            };
            let same = equal(&left, &right);
            samples.push(format!("{{\"plain_ns\":{plain_ns},\"profile_outer_ns\":{profile_outer_ns},\"plain_first\":{},\"equal\":{},\"profile\":{}}}",index%2==0,same,profile_json(&profile)));
            if !same {
                output.push(format!("{{\"id\":{},\"family\":{},\"input\":{},\"kind\":{},\"samples\":[{}],\"failed_sample_index\":{},\"left\":{},\"right\":{},\"left_graph\":{},\"right_graph\":{}}}",quote(fields[0]),quote(fields[1]),quote(fields[2]),quote(fields[3]),samples.join(","),index,details(&left),details(&right),graph_diagnostic(&left),graph_diagnostic(&right)));
                println!(
                    "{{\"schema_version\":1,\"experiment\":\"cloudleaf-stage-profile-v1\",\"partial\":true,\"failed\":true,\"language_accuracy_valid\":false,\"configuration\":{},\"cases\":[{}]}}",
                    quote(&format!("{config:?}")),
                    output.join(",")
                );
                return Err("观测入口改变算法结果/预算/图".into());
            }
            last = Some(right);
        }
        let start = Instant::now();
        let result = details(last.as_ref().unwrap());
        let details_ns = start.elapsed().as_nanos();
        let full_result = format!(
            "{:?}",
            last.as_ref().unwrap().as_ref().map(|(_, result)| result)
        );
        output.push(format!("{{\"id\":{},\"family\":{},\"input\":{},\"kind\":{},\"samples\":[{}],\"details_ns\":{},\"result\":{},\"result_debug\":{}}}",quote(fields[0]),quote(fields[1]),quote(fields[2]),quote(fields[3]),samples.join(","),details_ns,result,quote(&full_result)));
    }
    println!(
        "{{\"schema_version\":2,\"experiment\":\"cloudleaf-round15-stage-profile-v1\",\"language_accuracy_valid\":false,\"lexicon_entries\":{},\"trie_nodes\":{},\"load_ns\":{},\"iterations_per_case\":{},\"warmup_per_case\":{},\"cases\":[{}]}}",
        lexicon.entries().len(),
        lexicon.trie_nodes(),
        load_ns,
        iterations,
        warmup,
        output.join(",")
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("阶段观测失败：{error}");
        std::process::exit(1);
    }
}
