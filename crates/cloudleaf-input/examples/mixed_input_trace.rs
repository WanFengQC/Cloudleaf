//! 固定自编样例的真实词典 Release 回放。命令输出不含输入或候选正文。
use cloudleaf_input::{InputConfig, InputEngine, LanguageSnapshot, SearchSnapshot};
use cloudleaf_search::{Config, Method, Ranker};
use cloudleaf_types::{ErrorCode, InputEvent, Key, KeyDisposition};
use ring::digest::{SHA256, digest};
use std::{error::Error, sync::Arc, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let (fixture, runtime, runtime_sha, model, model_sha) = match arguments.as_slice() {
        [flag, fixture, runtime, runtime_sha, model, model_sha] if flag == "--round28" => {
            (Some(fixture), runtime, runtime_sha, model, model_sha)
        }
        [runtime, runtime_sha, model, model_sha] => (None, runtime, runtime_sha, model, model_sha),
        _ => {
            return Err("用法：mixed_input_trace [--round28 FIXTURE] RUNTIME SHA MODEL SHA".into());
        }
    };
    let dictionary = std::fs::read_to_string(runtime)?;
    let text = std::fs::read_to_string(model)?;
    let snapshot = Arc::new(SearchSnapshot::from_verified_tsv(&dictionary, runtime_sha)?);
    let language = Arc::new(LanguageSnapshot::from_verified_tsv(
        &text, model_sha, &snapshot,
    )?);
    let config = Config {
        method: Method::PositionDag,
        ranker: Ranker::Lexical,
        output_k: 10,
        max_input_bytes: 64,
        max_letters: 64,
        ..Config::default()
    };
    let mut engine =
        InputEngine::with_language(snapshot, config, language, None, InputConfig::default())?;
    engine.reconfigure_mixed_input(true)?;
    engine.handle(InputEvent::Activate);
    println!("META\t{runtime_sha}\t{model_sha}");
    for record in include_str!("../tests/fixtures/mixed-input.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .skip(1)
    {
        let fields = record.split('\t').collect::<Vec<_>>();
        let [case, typed, expected] = fields.as_slice() else {
            return Err("冻结混排轨迹列数无效".into());
        };
        replay(
            &mut engine,
            case,
            "round27",
            typed,
            typed,
            expected,
            fixture.is_some(),
        )?;
    }
    if let Some(fixture) = fixture {
        let records = std::fs::read_to_string(fixture)?;
        let sha = digest(&SHA256, records.as_bytes())
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        println!("FIXTURE\t{sha}");
        for record in records
            .lines()
            .filter(|line| !line.starts_with('#'))
            .skip(1)
        {
            let fields = record.split('\t').collect::<Vec<_>>();
            let [case, group, steps, original, expected] = fields.as_slice() else {
                return Err("Round 28 冻结混排轨迹列数无效".into());
            };
            replay(&mut engine, case, group, steps, original, expected, true)?;
        }
    }
    Ok(())
}

fn replay(
    engine: &mut InputEngine,
    case: &str,
    group: &str,
    steps: &str,
    original: &str,
    expected: &str,
    extend: bool,
) -> Result<(), Box<dyn Error>> {
    let mut pre_edit = None;
    let events = steps.split(' ').flat_map(|step| match step {
        "^L" => vec![Key::Left],
        "^R" => vec![Key::Right],
        "^B" => vec![Key::Backspace],
        _ => step.chars().map(Key::Char).collect(),
    });
    for (index, event) in events.enumerate() {
        if pre_edit.is_none() && !matches!(event, Key::Char(_)) {
            let frame = engine.frame();
            pre_edit = frame
                .candidates
                .first()
                .map(|c| (frame.revision, c.id.clone()));
        }
        assert_eq!(
            engine.key_disposition(event),
            KeyDisposition::Handle,
            "{case}/{index}"
        );
        let start = Instant::now();
        let result = engine.handle(InputEvent::Key(event));
        let wall = start.elapsed().as_nanos();
        assert!(
            result.handled && result.error.is_none() && result.commit.is_none(),
            "{case}/{index}"
        );
        let (edges, expansions, paths, budget) =
            engine
                .search_diagnostic()
                .map_or((0, 0, 0, None), |diagnostic| {
                    let stats = &diagnostic.statistics;
                    (
                        stats.graph_edges,
                        stats.expansions,
                        stats.resident_high_water,
                        stats.budget,
                    )
                });
        println!(
            "EVENT\t{case}\t{index}\t{}\t{wall}\t{edges}\t{expansions}\t{paths}\t{budget:?}\t{}",
            result.frame.revision,
            result.frame.candidates.len()
        );
    }
    let frame = engine.frame();
    assert_eq!(frame.preedit, original, "{case}");
    let stale_rejected = pre_edit.map(|(revision, id)| {
        let error = engine
            .select_candidate(revision, &id)
            .error
            .map(|err| err.code);
        assert_eq!(error, Some(ErrorCode::StaleCandidate), "{case}");
        assert_eq!(engine.frame(), frame, "{case}");
        true
    });
    let rank = frame
        .candidates
        .iter()
        .position(|candidate| candidate.text == expected)
        .map_or(0, |i| i + 1);
    if extend {
        let original_available = frame.candidates.iter().any(|c| c.text == original);
        for (index, candidate) in frame.candidates.iter().enumerate() {
            let path = engine.search_diagnostic().and_then(|diagnostic| {
                diagnostic
                    .paths
                    .iter()
                    .find(|p| p.candidate_id == candidate.id)
            });
            let spans = path.map_or_else(Vec::new, |path| {
                path.edges
                    .iter()
                    .map(|edge| (edge.raw_start, edge.raw_end))
                    .collect()
            });
            let mut literal_spans = Vec::new();
            let mut cursor = 0;
            for &(start, end) in &spans {
                if cursor < start {
                    literal_spans.push((cursor, start));
                }
                cursor = end;
            }
            if cursor < original.len() {
                literal_spans.push((cursor, original.len()));
            }
            println!(
                "RANK\t{case}\t{}\t{}\t{}\t{spans:?}\t{literal_spans:?}",
                index + 1,
                candidate.source,
                candidate.id,
            );
        }
        println!(
            "FRAME\t{case}\t{group}\t{}\t{}\t{original_available}\t{stale_rejected:?}",
            frame.revision, frame.cursor
        );
    }
    let commit = engine
        .handle(InputEvent::Key(Key::Enter))
        .commit
        .ok_or("应有提交")?;
    let exact_commit = commit.text == expected;
    let commit_is_literal = commit.text == original;
    println!("RESULT\t{case}\t{rank}\t{exact_commit}\t{commit_is_literal}");
    if extend {
        if let Some(evidence) = engine.pending_selection() {
            assert_eq!(evidence.commit_id, commit.id, "{case}");
            assert_eq!(evidence.revision, frame.revision, "{case}");
            assert_eq!(evidence.candidate_id, frame.candidates[0].id, "{case}");
            println!(
                "LEARNING\t{case}\t{}\t{}",
                commit.id,
                evidence.entries.len()
            );
        } else {
            println!("LEARNING\t{case}\t{}\t0", commit.id);
        }
        assert!(
            !engine
                .handle(InputEvent::CommitFailed { id: commit.id })
                .frame
                .preedit
                .is_empty(),
            "{case}"
        );
        assert_eq!(engine.frame().preedit, original, "{case}");
    }
    let receipt = if extend {
        let retry = engine
            .handle(InputEvent::RetryCommit)
            .commit
            .ok_or("应有重试")?;
        assert_ne!(retry.id, commit.id, "{case}");
        assert_eq!(retry.text, commit.text, "{case}");
        println!("RECOVERY\t{case}\t{}\t{}", commit.id, retry.id);
        retry.id
    } else {
        commit.id
    };
    assert!(
        engine
            .handle(InputEvent::CommitSucceeded { id: receipt })
            .frame
            .preedit
            .is_empty(),
        "{case}"
    );
    assert!(engine.pending_selection().is_none(), "{case}");
    if extend {
        for character in original.chars() {
            let result = engine.handle(InputEvent::Key(Key::Char(character)));
            assert!(result.handled && result.error.is_none() && result.commit.is_none());
        }
        let revision = engine.frame().revision;
        let literal = engine
            .confirm_literal(revision, "")
            .commit
            .ok_or("应有原文提交")?;
        assert_eq!(literal.text, original, "{case}");
        assert!(engine.pending_selection().is_none(), "{case}");
        println!("LITERAL\t{case}\t{revision}\t{}\ttrue", literal.id);
        assert!(
            engine
                .handle(InputEvent::CommitSucceeded { id: literal.id })
                .frame
                .preedit
                .is_empty(),
            "{case}"
        );
    }
    Ok(())
}
