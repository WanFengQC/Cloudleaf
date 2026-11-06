//! 显式研发事件轨迹，调用真实引擎；只在该工具输出自有样本原文。
use cloudleaf_input::{InputConfig, InputEngine, LanguageSnapshot, SearchSnapshot};
use cloudleaf_search::{Config, Method, Ranker};
use cloudleaf_types::{InputEvent, Key};
use std::{io::BufRead, sync::Arc, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !matches!(args.len(), 2 | 4 | 5) {
        return Err("usage: input_trace RUNTIME SHA256 [MODEL MODEL_SHA [structural|lexical|bigram]] < own-events.tsv".into());
    }
    let start = Instant::now();
    let text = std::fs::read_to_string(&args[0])?;
    let snapshot = Arc::new(SearchSnapshot::from_verified_tsv(&text, &args[1])?);
    println!(
        "LOAD\t{}\t{}\t{}\t{}",
        start.elapsed().as_nanos(),
        snapshot.version(),
        snapshot.entries().len(),
        snapshot.estimated_bytes()
    );
    let mut config = Config {
        method: Method::PositionDag,
        ranker: Ranker::Structural,
        output_k: 10,
        max_input_bytes: 64,
        max_letters: 64,
        ..Config::default()
    };
    let mut engine = if args.len() >= 4 {
        let text = std::fs::read_to_string(&args[2])?;
        let language = Arc::new(LanguageSnapshot::from_verified_tsv(
            &text, &args[3], &snapshot,
        )?);
        config.ranker = match args
            .get(4)
            .map(String::as_str)
            .unwrap_or(language.metadata().kind.as_str())
        {
            "structural" => Ranker::Structural,
            "lexical" => Ranker::Lexical,
            "bigram" => {
                config.method = Method::ExactDag;
                Ranker::Bigram
            }
            _ => return Err("unknown research ranker".into()),
        };
        println!(
            "MODEL\t{}\t{}\t{:?}",
            language.version(),
            language.estimated_bytes(),
            language.metadata()
        );
        InputEngine::with_language(snapshot, config, language, None, InputConfig::default())?
    } else {
        InputEngine::with_search(snapshot, config, InputConfig::default())?
    };
    engine.handle(InputEvent::Activate);
    let mut commits = 0;
    let mut event_number = 0;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let (case, sequence) = line.split_once('\t').ok_or("expected case TAB events")?;
        for token in sequence.split_ascii_whitespace() {
            let events = match token {
                "<Space>" => vec![InputEvent::Key(Key::Space)],
                "<Enter>" => vec![InputEvent::Key(Key::Enter)],
                "<Esc>" => vec![InputEvent::Key(Key::Escape)],
                "<Left>" => vec![InputEvent::Key(Key::Left)],
                "<Right>" => vec![InputEvent::Key(Key::Right)],
                "<Backspace>" => vec![InputEvent::Key(Key::Backspace)],
                "<Down>" => vec![InputEvent::Key(Key::Down)],
                "<PageDown>" => vec![InputEvent::Key(Key::PageDown)],
                "<Blur>" => vec![InputEvent::FocusLost],
                _ => token
                    .chars()
                    .map(|ch| InputEvent::Key(Key::Char(ch)))
                    .collect(),
            };
            for event in events {
                event_number += 1;
                let start = Instant::now();
                let result = engine.handle(event);
                println!(
                    "EVENT\t{case}\t{event_number}\t{}\t{event:?}\t{}\t{:?}\t{:?}",
                    start.elapsed().as_nanos(),
                    result.handled,
                    result.error.as_ref().map(|error| error.code),
                    result.frame.preedit
                );
                if let Some(commit) = result.commit {
                    commits += 1;
                    println!(
                        "COMMIT\t{case}\t{}\t{:?}\tSIMULATED_APPLIED",
                        commit.id, commit.text
                    );
                    engine.handle(InputEvent::CommitSucceeded { id: commit.id });
                    let duplicate = engine.handle(InputEvent::CommitSucceeded { id: commit.id });
                    assert_eq!(
                        duplicate.error.unwrap().code,
                        cloudleaf_types::ErrorCode::StaleCommit
                    );
                }
            }
        }
        let frame = engine.frame();
        println!(
            "FRAME\t{case}\t{}\t{:?}\t{:?}\t{:?}\t{}\t{}",
            frame.revision,
            frame.state,
            frame.preedit,
            frame.highlighted,
            frame.page_start,
            frame.candidates.len()
        );
        for (index, candidate) in frame.candidates.iter().enumerate() {
            println!(
                "CANDIDATE\t{case}\t{index}\t{:?}\t{}\t{:?}\t{:?}",
                candidate.text, candidate.id, candidate.frequency, candidate.match_kind
            );
        }
        if let Some(diagnostic) = engine.search_diagnostic() {
            println!(
                "SEARCH\t{case}\t{:?}\t{}\t{:?}\t{:?}",
                diagnostic.method, diagnostic.guaranteed, diagnostic.statistics, diagnostic.error
            );
            for path in &diagnostic.paths {
                println!(
                    "PATH\t{case}\t{}\t{}\t{:?}\t{:?}",
                    path.candidate_id, path.score, path.features, path.edges
                );
            }
        }
    }
    println!(
        "SUMMARY\tevents={event_number}\tconfirmed_simulated_commits={commits}\tformal_metrics=null\tTSF=false"
    );
    Ok(())
}
