//! 无窗口词条格触顶诊断。输入和预期正文只从本地实验材料载入。
use cloudleaf_input::{
    InputConfig, InputEngine, LanguageSnapshot, PinyinScheme, SchemeConfig, SearchSnapshot,
};
use cloudleaf_search::{
    Budget, Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, SchemeAdmission,
    profile_scheme_query_with_admission,
};
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
        return Err("需要 RUNTIME RUNTIME_SHA MODEL MODEL_SHA FIXTURE LABELS".into());
    };
    let runtime = std::fs::read_to_string(runtime)?;
    let language = std::fs::read_to_string(language)?;
    let fixture = std::fs::read_to_string(fixture)?;
    let labels = std::fs::read_to_string(labels)?;
    let target_rows: std::collections::BTreeMap<_, _> = labels
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('\t');
            let case = fields.next().unwrap();
            let raw = fields.next().unwrap();
            let target = fields.next().unwrap();
            assert!(fields.next().is_none());
            ((case, raw), target)
        })
        .collect();
    let config = Config {
        method: Method::PositionDag,
        ranker: Ranker::Lexical,
        output_k: 10,
        max_input_bytes: 64,
        max_letters: 64,
        ..Config::default()
    };
    assert_eq!(
        (
            config.max_edges,
            config.max_expansions,
            config.max_resident_paths
        ),
        (16_384, 200_000, 50_000)
    );
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
            if let Some(diagnostic) = engine
                .search_diagnostic()
                .filter(|diag| diag.revision == frame.revision && diag.raw_input == frame.preedit)
            {
                let search =
                    |config: &Config, admission| -> Result<_, cloudleaf_search::SearchError> {
                        profile_scheme_query_with_admission(
                            &lexicon,
                            &frame.preedit,
                            config,
                            &scheme,
                            Some(model.model()),
                            admission,
                        )
                        .0
                    };
                let (forward, old) = search(&config, SchemeAdmission::Forward)?;
                let (reachable, new) = search(&config, SchemeAdmission::ReachableSuffix)?;
                assert_eq!(new.statistics, diagnostic.statistics);
                if forward.budget().is_none() && reachable.budget().is_none() {
                    assert_eq!(forward.edges(), reachable.edges());
                    for index in 0..forward.edges().len() {
                        assert_eq!(
                            forward.interpretation(index),
                            reachable.interpretation(index)
                        );
                    }
                    assert_eq!(old, new);
                    println!("EQUAL\t{case}\t{event_index}\t{}", forward.edges().len());
                } else if forward.budget() == Some(Budget::GraphEdges) {
                    let (balanced, alternate) = search(&config, SchemeAdmission::Balanced)?;
                    let wide = Config {
                        max_edges: 32768,
                        ..config.clone()
                    };
                    let (diagnosis, wider) = search(&wide, SchemeAdmission::Forward)?;
                    let target = target_rows
                        .get(&(case, &*frame.preedit))
                        .or_else(|| target_rows.get(&("*", &*frame.preedit)));
                    if let Some(target) = target {
                        let frame_rank = frame
                            .candidates
                            .iter()
                            .position(|candidate| candidate.text == *target)
                            .map(|index| index + 1);
                        println!("TARGET_FRAME\t{case}\t{event_index}\t{frame_rank:?}");
                        if let Some(commit) = response.commit.as_ref() {
                            println!(
                                "TARGET_COMMIT\t{case}\t{event_index}\t{}",
                                commit.text == *target
                            );
                        }
                    }
                    for (admission, graph, result) in [
                        ("forward", &forward, &old),
                        ("balanced", &balanced, &alternate),
                        ("reachable", &reachable, &new),
                        ("diagnostic", &diagnosis, &wider),
                    ] {
                        let reaches =
                            target.map(|target| target_reachable(graph, &lexicon, target));
                        let rank = target.and_then(|target| {
                            result
                                .candidates
                                .iter()
                                .position(|c| c.text == *target)
                                .map(|row| row + 1)
                        });
                        let cap = graph
                            .scheme_graph_diagnostics()
                            .and_then(|diagnostic| diagnostic.first_rejected_start);
                        println!(
                            "CAP\t{case}\t{event_index}\t{admission}\t{}\t{}\t{:?}\t{:?}\t{}\t{}\t{}\t{reaches:?}\t{rank:?}\t{cap:?}",
                            graph.edges().len(),
                            viable_edges(graph),
                            graph.budget(),
                            result.statistics.budget,
                            result.statistics.expansions,
                            result.statistics.resident_high_water,
                            result.candidates.len(),
                        );
                    }
                } else {
                    println!(
                        "OTHER\t{case}\t{event_index}\t{:?}\t{:?}",
                        forward.budget(),
                        reachable.budget()
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
