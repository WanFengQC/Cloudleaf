//! 仅接收自有事件的无窗口缓存对照工具；实际调用InputEngine，不构造候选或学习确认。
use cloudleaf_input::{
    DoublePinyinLayout, InputConfig, InputEngine, LanguageSnapshot, PinyinScheme, SchemeConfig,
    SearchSnapshot,
};
use cloudleaf_search::{Config, Method, Ranker};
use cloudleaf_types::{InputEvent, Key};
use std::{io::BufRead, sync::Arc, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 6 {
        return Err(
            "incremental_trace RUNTIME SHA MODEL SHA full|abbreviated|xiaohe|fuzzy-nl|correction on|off".into(),
        );
    }
    let source = std::fs::read_to_string(&args[0])?;
    let snapshot = Arc::new(SearchSnapshot::from_verified_tsv(&source, &args[1])?);
    let model = std::fs::read_to_string(&args[2])?;
    let language = Arc::new(LanguageSnapshot::from_verified_tsv(
        &model, &args[3], &snapshot,
    )?);
    let character = language.metadata().kind.starts_with("character_");
    let config = Config {
        method: if character {
            Method::ExactDag
        } else {
            Method::PositionDag
        },
        ranker: if character {
            Ranker::CharacterBigram
        } else {
            Ranker::Lexical
        },
        output_k: 10,
        max_input_bytes: 64,
        max_letters: 64,
        ..Config::default()
    };
    let mut engine =
        InputEngine::with_language(snapshot, config, language, None, InputConfig::default())?;
    engine.set_search_cache_enabled(match args[5].as_str() {
        "on" => true,
        "off" => false,
        _ => return Err("cache must be on|off".into()),
    });
    engine.reconfigure_scheme(SchemeConfig {
        scheme: match args[4].as_str() {
            "full" => PinyinScheme::FullPinyin,
            "abbreviated" => PinyinScheme::AbbreviatedPinyin,
            "xiaohe" => PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
            "fuzzy-nl" | "correction" => PinyinScheme::FullPinyin,
            _ => return Err("unknown scheme".into()),
        },
        fuzzy: if args[4] == "fuzzy-nl" {
            vec![cloudleaf_input::FuzzyRule {
                pair: cloudleaf_input::FuzzyPair::NL,
                direction: cloudleaf_input::FuzzyDirection::Both,
            }]
        } else {
            Vec::new()
        },
        correction: (args[4] == "correction").then(cloudleaf_input::CorrectionConfig::default),
        ..SchemeConfig::default()
    })?;
    engine.handle(InputEvent::Activate);
    let mut sequence = 0;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (case, tokens) = line.split_once('\t').ok_or("case TAB events required")?;
        for token in tokens.split_ascii_whitespace() {
            let events = match token {
                "<Left>" => vec![InputEvent::Key(Key::Left)],
                "<Right>" => vec![InputEvent::Key(Key::Right)],
                "<PageDown>" => vec![InputEvent::Key(Key::PageDown)],
                "<Down>" => vec![InputEvent::Key(Key::Down)],
                "<Backspace>" => vec![InputEvent::Key(Key::Backspace)],
                "<Esc>" => vec![InputEvent::Key(Key::Escape)],
                "<Space>" => vec![InputEvent::Key(Key::Space)],
                "<Enter>" => vec![InputEvent::Key(Key::Enter)],
                "<Blur>" => vec![InputEvent::FocusLost],
                "<Activate>" => vec![InputEvent::Activate],
                _ => token
                    .chars()
                    .map(|c| InputEvent::Key(Key::Char(c)))
                    .collect(),
            };
            for event in events {
                sequence += 1;
                let start = Instant::now();
                let result = engine.handle(event);
                let elapsed = start.elapsed().as_nanos();
                println!(
                    "EVENT\t{case}\t{sequence}\t{elapsed}\t{result:?}\t{:?}",
                    engine.search_diagnostic()
                );
                println!(
                    "CACHE\t{case}\t{sequence}\t{:?}",
                    engine.search_cache_statistics()
                );
                if let Some(commit) = result.commit {
                    engine.handle(InputEvent::CommitSucceeded { id: commit.id });
                    let duplicate = engine.handle(InputEvent::CommitSucceeded { id: commit.id });
                    assert_eq!(
                        duplicate.error.unwrap().code,
                        cloudleaf_types::ErrorCode::StaleCommit
                    );
                }
            }
        }
    }
    println!(
        "SUMMARY\tevents={sequence}\tcapacity={}\tformal_metrics=null\tTSF=false",
        engine.search_cache_capacity_bytes()
    );
    Ok(())
}
