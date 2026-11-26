//! 固定自编样例的真实词典 Release 回放。命令输出不含输入或候选正文。
use cloudleaf_input::{InputConfig, InputEngine, LanguageSnapshot, SearchSnapshot};
use cloudleaf_search::{Config, Method, Ranker};
use cloudleaf_types::{InputEvent, Key, KeyDisposition};
use std::{error::Error, sync::Arc, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let [runtime, runtime_sha, model, model_sha] = arguments.as_slice() else {
        return Err("用法：mixed_input_trace RUNTIME SHA MODEL SHA".into());
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
        for (index, ch) in typed.chars().enumerate() {
            let event = Key::Char(ch);
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
        let rank = frame
            .candidates
            .iter()
            .position(|candidate| &candidate.text == expected)
            .map_or(0, |i| i + 1);
        let commit = engine
            .handle(InputEvent::Key(Key::Enter))
            .commit
            .ok_or("应有提交")?;
        let exact_commit = &commit.text == expected;
        let commit_is_literal = &commit.text == typed;
        println!("RESULT\t{case}\t{rank}\t{exact_commit}\t{commit_is_literal}");
        assert!(
            engine
                .handle(InputEvent::CommitSucceeded { id: commit.id })
                .frame
                .preedit
                .is_empty()
        );
    }
    Ok(())
}
