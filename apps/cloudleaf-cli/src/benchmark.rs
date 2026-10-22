//! 固定原型词典上的独立核心基准，不包含 UI、平台提交和网络。

use std::hint::black_box;
use std::time::Instant;

use cloudleaf_dictionary::{Dictionary, PROTOTYPE_TSV};
use cloudleaf_input::InputEngine;
use cloudleaf_pinyin::parse;
use cloudleaf_types::{InputEvent, Key};

const QUERIES: &[&str] = &[
    "nihao", "zhongguo", "xian", "xi'an", "zhongwen", "shijie", "cankao", "nv", "lve", "zhongg",
    "zzzz",
];

pub fn run(iterations: usize) -> Result<(), String> {
    let cold_start = Instant::now();
    let dictionary = Dictionary::from_tsv(PROTOTYPE_TSV).map_err(|error| error.to_string())?;
    let entries = dictionary.len();
    let mut engine = InputEngine::new(dictionary);
    engine.handle(InputEvent::Activate);
    for character in "nihao".chars() {
        black_box(engine.handle(InputEvent::Key(Key::Char(character))));
    }
    let cold_ns = cold_start.elapsed().as_nanos();
    engine.handle(InputEvent::Key(Key::Escape));
    for _ in 0..20 {
        sample_events(&mut engine, &mut Vec::new());
    }

    let mut event_times = Vec::with_capacity(iterations * 100);
    for _ in 0..iterations {
        sample_events(&mut engine, &mut event_times);
    }

    // 在计时区间外解析，单独计量内存索引查询，而非解析或排序。
    let dictionary = Dictionary::from_tsv(PROTOTYPE_TSV).map_err(|error| error.to_string())?;
    let parsed: Vec<_> = QUERIES.iter().map(|query| parse(query).unwrap()).collect();
    let mut query_times = Vec::with_capacity(iterations * QUERIES.len());
    for _ in 0..20 {
        for result in &parsed {
            for path in &result.parses {
                black_box(dictionary.query(path));
            }
        }
    }
    for _ in 0..iterations {
        for result in &parsed {
            let start = Instant::now();
            for path in &result.parses {
                black_box(dictionary.query(path));
            }
            query_times.push(start.elapsed().as_nanos());
        }
    }
    print!(
        "{{\"schema\":1,\"iterations\":{iterations},\"dictionary_entries\":{entries},\"dictionary_bytes\":{},\"candidate_limit\":{},\"cold_load_and_nihao_sequence_ns\":{cold_ns},\"event\":",
        PROTOTYPE_TSV.len(),
        cloudleaf_input::MAX_CANDIDATES
    );
    print_stats(&mut event_times);
    print!(",\"dictionary_query\":");
    print_stats(&mut query_times);
    println!("}}");
    Ok(())
}

fn sample_events(engine: &mut InputEngine, times: &mut Vec<u128>) {
    for query in QUERIES {
        engine.handle(InputEvent::Key(Key::Escape));
        for character in query.chars() {
            measure(engine, InputEvent::Key(Key::Char(character)), times);
        }
        measure(engine, InputEvent::Key(Key::Backspace), times);
        measure(engine, InputEvent::Key(Key::Escape), times);
    }
}

fn measure(engine: &mut InputEngine, event: InputEvent, times: &mut Vec<u128>) {
    let start = Instant::now();
    let result = engine.handle(event);
    let elapsed = start.elapsed().as_nanos();
    assert!(result.error.is_none(), "固定基准事件不能返回错误");
    black_box(result);
    times.push(elapsed);
}

fn print_stats(times: &mut [u128]) {
    times.sort_unstable();
    let percentile = |value: usize| times[(times.len() * value).div_ceil(100).saturating_sub(1)];
    print!(
        "{{\"samples\":{},\"p50_ns\":{},\"p95_ns\":{},\"p99_ns\":{},\"max_ns\":{}}}",
        times.len(),
        percentile(50),
        percentile(95),
        percentile(99),
        times[times.len() - 1]
    );
}
