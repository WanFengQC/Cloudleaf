//! 无窗口词条格触顶诊断。输入和预期正文只从本地实验材料载入。
use cloudleaf_input::{
    InputConfig, InputEngine, LanguageSnapshot, PinyinScheme, SchemeConfig, SearchSnapshot,
};
use cloudleaf_search::{Budget, Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker};
use cloudleaf_types::{InputEvent, Key};
use std::{error::Error, sync::Arc};

fn events(token: &str) -> Vec<InputEvent> {
    let key = match token {
        "<Blur>" => return vec![InputEvent::FocusLost],
        "<Left>" => Key::Left,
        "<Right>" => Key::Right,
        "<Space>" => Key::Space,
        "<Enter>" => Key::Enter,
        "<Esc>" => Key::Escape,
        "<Backspace>" => Key::Backspace,
        "<Down>" => Key::Down,
        "<PageDown>" => Key::PageDown,
        _ => {
            return token
                .chars()
                .map(|ch| InputEvent::Key(Key::Char(ch)))
                .collect();
        }
    };
    vec![InputEvent::Key(key)]
}

fn viable_edges(graph: &Lattice<'_>) -> usize {
    // 本探针只接收已关闭纠错的简拼图，此处无剩余编辑量维度。
    let mut from_start = vec![false; graph.letters().len() + 1];
    let mut to_end = from_start.clone();
    from_start[0] = true;
    *to_end.last_mut().unwrap() = true;
    for edge in graph.edges() {
        from_start[edge.end] |= from_start[edge.start];
    }
    for edge in graph.edges().iter().rev() {
        to_end[edge.start] |= to_end[edge.end];
    }
    graph
        .edges()
        .iter()
        .filter(|edge| from_start[edge.start] && to_end[edge.end])
        .count()
}

fn target_reachable(graph: &Lattice<'_>, lexicon: &Lexicon, target: &str) -> bool {
    let mut reaches = vec![vec![false; target.len() + 1]; graph.letters().len() + 1];
    reaches[0][0] = true;
    for edge in graph.edges() {
        let text = &lexicon.entries()[edge.entry].text;
        for (offset, _) in target.char_indices() {
            if reaches[edge.start][offset] && target[offset..].starts_with(text) {
                reaches[edge.end][offset + text.len()] = true;
            }
        }
    }
    reaches[graph.letters().len()][target.len()]
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [
        runtime,
        runtime_sha,
        language,
        language_sha,
        fixture,
        labels,
    ] = args.as_slice()
    else {
        return Err(
            "usage: round27_graph_diagnostics RUNTIME RUNTIME_SHA MODEL MODEL_SHA FIXTURE LABELS"
                .into(),
        );
    };
    let runtime = std::fs::read_to_string(runtime)?;
    let language = std::fs::read_to_string(language)?;
    let fixture = std::fs::read_to_string(fixture)?;
    let labels = std::fs::read_to_string(labels)?;
    let mut target_rows = std::collections::BTreeMap::new();
    for line in labels
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let [case, raw, target] = line
            .split('\t')
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "诊断标签须有案例、完整预编辑和正文三列")?;
        assert!(target_rows.insert((case, raw), target).is_none());
    }
    let config = Config {
        method: Method::PositionDag,
        ranker: Ranker::Lexical,
        output_k: 10,
        max_input_bytes: 64,
        max_letters: 64,
        ..Config::default()
    };
    let wider = Config {
        max_edges: config.max_edges * 2,
        ..config.clone()
    };
    let scheme = SchemeConfig {
        scheme: PinyinScheme::AbbreviatedPinyin,
        ..SchemeConfig::default()
    };
    let snapshot = Arc::new(SearchSnapshot::from_verified_tsv(&runtime, runtime_sha)?);
    let engine_model = Arc::new(LanguageSnapshot::from_verified_tsv(
        &language,
        language_sha,
        &snapshot,
    )?);
    let lexicon = Lexicon::from_tsv(&runtime)?;
    let model = LanguageModelSnapshot::from_tsv(&language, &lexicon, runtime_sha)?;
    let mut engine = InputEngine::with_language(
        snapshot,
        config.clone(),
        engine_model,
        None,
        InputConfig::default(),
    )?;
    engine.reconfigure_scheme(scheme.clone())?;
    engine.handle(InputEvent::Activate);
    let mut event_index = 0;
    for line in fixture
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let (case, sequence) = line.split_once('\t').ok_or("缺少案例 ID")?;
        for event in sequence.split_ascii_whitespace().flat_map(events) {
            event_index += 1;
            let response = engine.handle(event);
            let frame = response.frame;
            if let Some(diagnostic) = engine.search_diagnostic().filter(|diag| {
                diag.revision == frame.revision
                    && diag.raw_input == frame.preedit
                    && diag.statistics.budget.is_some()
            }) {
                let (query, _, _) = cloudleaf_search::profile_scheme_query(
                    &lexicon,
                    &frame.preedit,
                    &config,
                    &scheme,
                    Some(model.model()),
                );
                let (graph, result) = query?;
                assert_eq!(result.statistics, diagnostic.statistics);
                if graph.budget() == Some(Budget::GraphEdges) {
                    let (query, _, _) = cloudleaf_search::profile_scheme_query(
                        &lexicon,
                        &frame.preedit,
                        &wider,
                        &scheme,
                        Some(model.model()),
                    );
                    let (expanded, expanded_result) = query?;
                    let counts = graph.scheme_graph_diagnostics().unwrap();
                    let target = target_rows
                        .get(&(case, &*frame.preedit))
                        .or_else(|| target_rows.get(&("*", &*frame.preedit)));
                    let retained_target =
                        target.map(|target| target_reachable(&graph, &lexicon, target));
                    let expanded_target =
                        target.map(|target| target_reachable(&expanded, &lexicon, target));
                    let top_target = target
                        .map(|target| result.candidates.iter().position(|c| c.text == *target));
                    let expanded_top_target = target.map(|target| {
                        expanded_result
                            .candidates
                            .iter()
                            .position(|c| c.text == *target)
                    });
                    let same_top = result
                        .candidates
                        .iter()
                        .map(|c| &c.text)
                        .eq(expanded_result.candidates.iter().map(|c| &c.text));
                    let numeric = [
                        frame.revision as usize,
                        frame.preedit.len(),
                        graph.edges().len(),
                        config.max_edges,
                        counts.published_before_cap,
                        counts.buffered_before_cap,
                        counts.dictionary_rows,
                        counts.duplicate_edges,
                        counts.boundary_rejected,
                        viable_edges(&graph),
                        result.statistics.expansions,
                        result.statistics.resident_high_water,
                        expanded.edges().len(),
                        viable_edges(&expanded),
                        result.candidates.len(),
                        expanded_result.candidates.len(),
                        counts.better_representatives,
                    ]
                    .map(|value| value.to_string())
                    .join("\t");
                    println!(
                        "CAP\t{case}\t{event_index}\t{numeric}\t{:?}\t{same_top}\t{retained_target:?}\t{expanded_target:?}\t{top_target:?}\t{expanded_top_target:?}\t{:?}\t{:?}\t{:?}",
                        counts.first_rejected_start,
                        result.statistics.budget,
                        expanded.budget(),
                        expanded_result.statistics.budget
                    );
                }
            }
            if let Some(commit) = response.commit {
                let receipt = engine.handle(InputEvent::CommitSucceeded { id: commit.id });
                assert!(receipt.error.is_none());
            }
        }
    }
    println!("DONE\t{event_index}");
    Ok(())
}
