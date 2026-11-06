//! 固定正文可达性诊断；自动导出的读音和来源正文不构成人工gold。
use cloudleaf_search::{
    Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, search_graph,
};
use ring::digest::{SHA256, digest};
use std::{collections::BTreeMap, error::Error, fs};

fn sha(text: &str) -> String {
    digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: coverage_probe RUNTIME SHA MODEL MODEL_SHA TARGET_TSV".into());
    }
    let runtime = fs::read_to_string(&args[0])?;
    let model_text = fs::read_to_string(&args[2])?;
    if sha(&runtime) != args[1] || sha(&model_text) != args[3] {
        return Err("实际资源SHA不符".into());
    }
    let lexicon = Lexicon::from_tsv(&runtime)?;
    let model = LanguageModelSnapshot::from_tsv(&model_text, &lexicon, &args[1])?;
    let ranker = match model.metadata().kind.as_str() {
        "lexical" => Ranker::Lexical,
        "bigram" => Ranker::Bigram,
        "character_bigram" | "character_bigram_witten_bell" => Ranker::CharacterBigram,
        _ => return Err("未支持的研发评分".into()),
    };
    let config = Config {
        method: if ranker == Ranker::Lexical {
            Method::PositionDag
        } else {
            Method::ExactDag
        },
        ranker,
        output_k: 10,
        max_input_bytes: 64,
        max_letters: 64,
        ..Config::default()
    };
    let targets = fs::read_to_string(&args[4])?;
    println!(
        "IDENTITY\t{}\t{}\t{}\t{:?}",
        args[1],
        args[3],
        sha(&targets),
        model.metadata()
    );
    let mut total = 0;
    let mut reachable = 0;
    for line in targets
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 {
            return Err("诊断输入字段错误，未静默忽略记录".into());
        }
        if fields[0] == "case" {
            continue;
        }
        if fields[2].len() > 1024 {
            return Err("诊断正文超限".into());
        }
        total += 1;
        let graph = match Lattice::build(&lexicon, fields[1], &config) {
            Ok(graph) => graph,
            Err(error) => {
                println!(
                    "CASE\t{}\t{:?}\t{:?}\tparser_error={error:?}",
                    fields[0], fields[1], fields[2]
                );
                continue;
            }
        };
        let count = target_paths(&lexicon, &graph, fields[2]);
        reachable += usize::from(count > 0);
        println!(
            "CASE\t{}\t{:?}\t{:?}\ttarget_paths_capped_u64={count}\tgraph_budget={:?}",
            fields[0],
            fields[1],
            fields[2],
            graph.budget()
        );
        let result = search_graph(&lexicon, &graph, &config, Some(model.model()), None)?;
        println!("RESULT\t{}\t{result:?}", fields[0]);
    }
    println!("SUMMARY\ttotal={total}\ttext_reachable={reachable}\tformal_metrics=null");
    Ok(())
}

fn target_paths(lexicon: &Lexicon, graph: &Lattice<'_>, target: &str) -> u64 {
    let mut states = vec![BTreeMap::<usize, u64>::new(); graph.letters().len() + 1];
    states[0].insert(0, 1);
    let mut outgoing = vec![Vec::new(); states.len()];
    for edge in graph.edges() {
        outgoing[edge.start].push(edge);
    }
    for position in 0..graph.letters().len() {
        let current = std::mem::take(&mut states[position]);
        for (offset, count) in current {
            for edge in &outgoing[position] {
                let text = &lexicon.entries()[edge.entry].text;
                if target[offset..].starts_with(text) {
                    let total = states[edge.end].entry(offset + text.len()).or_default();
                    *total = total.saturating_add(count);
                }
            }
        }
    }
    states
        .last()
        .unwrap()
        .get(&target.len())
        .copied()
        .unwrap_or(0)
}
