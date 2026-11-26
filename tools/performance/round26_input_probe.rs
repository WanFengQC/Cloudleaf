//! 单线程无窗口输入诊断。默认工作区禁止 unsafe；此程序在独立目录链接。
use cloudleaf_input::{
    InputConfig, InputEngine, LanguageSnapshot, PinyinScheme, SchemeConfig, SearchSnapshot,
};
use cloudleaf_search::{
    Config, LanguageModelSnapshot, Lexicon, Method, Ranker, profile_scheme_query,
};
use cloudleaf_types::{InputEvent, Key};
use ring::digest::{SHA256, digest};
use std::alloc::{GlobalAlloc, Layout, System};
use std::error::Error;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering::Relaxed},
};
use std::time::Instant;

struct MeasuredAllocator;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static CUMULATIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);

fn allocated(request: usize, growth: usize) {
    let total = LIVE.fetch_add(growth, Relaxed) + growth;
    PEAK.fetch_max(total, Relaxed);
    CUMULATIVE_BYTES.fetch_add(request, Relaxed);
    CALLS.fetch_add(1, Relaxed);
}

// 地址、对齐和生命周期完全沿用 System 的契约；只记录请求大小。
unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, shape: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(shape) };
        if !pointer.is_null() {
            allocated(shape.size(), shape.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, shape: Layout) {
        LIVE.fetch_sub(shape.size(), Relaxed);
        unsafe { System.dealloc(pointer, shape) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, shape: Layout, size: usize) -> *mut u8 {
        let replacement = unsafe { System.realloc(pointer, shape, size) };
        if !replacement.is_null() {
            LIVE.fetch_sub(shape.size(), Relaxed);
            allocated(size, size);
        }
        replacement
    }
}

#[global_allocator]
static ALLOCATOR: MeasuredAllocator = MeasuredAllocator;

fn identity(value: &str) -> String {
    digest(&SHA256, value.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn parse_event(token: &str) -> Vec<InputEvent> {
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

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [
        runtime,
        runtime_sha,
        language,
        language_sha,
        fixture,
        scheme,
        mode,
    ] = args.as_slice()
    else {
        return Err("usage: round26_input_probe RUNTIME RUNTIME_SHA MODEL MODEL_SHA FIXTURE full|abbreviated normal|profile".into());
    };
    let mode_profile = match mode.as_str() {
        "normal" => false,
        "profile" => true,
        _ => return Err("未知观测模式".into()),
    };
    let scheme = match scheme.as_str() {
        "full" => SchemeConfig::default(),
        "abbreviated" => SchemeConfig {
            scheme: PinyinScheme::AbbreviatedPinyin,
            ..SchemeConfig::default()
        },
        _ => return Err("未知输入方案".into()),
    };
    let runtime = std::fs::read_to_string(runtime)?;
    let language = std::fs::read_to_string(language)?;
    let snapshot = Arc::new(SearchSnapshot::from_verified_tsv(&runtime, runtime_sha)?);
    let language_snapshot = Arc::new(LanguageSnapshot::from_verified_tsv(
        &language,
        language_sha,
        &snapshot,
    )?);
    let diagnostics = if mode_profile {
        let lexicon = Box::new(Lexicon::from_tsv(&runtime)?);
        let model = LanguageModelSnapshot::from_tsv(&language, &lexicon, runtime_sha)?;
        Some((lexicon, model))
    } else {
        None
    };
    let config = Config {
        method: Method::PositionDag,
        ranker: Ranker::Lexical,
        output_k: 10,
        max_input_bytes: 64,
        max_letters: 64,
        ..Config::default()
    };
    let mut engine = InputEngine::with_language(
        snapshot,
        config.clone(),
        language_snapshot,
        None,
        InputConfig::default(),
    )?;
    engine.reconfigure_scheme(scheme.clone())?;
    engine.handle(InputEvent::Activate);
    let fixture = std::fs::read_to_string(fixture)?;
    println!(
        "META\t{}\t{}\t{}\t{}\t{mode}",
        runtime_sha,
        language_sha,
        identity(&fixture),
        scheme.version_id()
    );
    let mut index = 0;
    for case in fixture
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let (case, sequence) = case.split_once('\t').ok_or("轨迹缺少事件")?;
        for event in sequence.split_ascii_whitespace().flat_map(parse_event) {
            index += 1;
            let before = LIVE.load(Relaxed);
            let bytes = CUMULATIVE_BYTES.load(Relaxed);
            let calls = CALLS.load(Relaxed);
            PEAK.store(before, Relaxed);
            let start = Instant::now();
            let result = engine.handle(event);
            let elapsed = start.elapsed().as_nanos();
            let end = LIVE.load(Relaxed);
            let peak = PEAK.load(Relaxed);
            let bytes = CUMULATIVE_BYTES.load(Relaxed) - bytes;
            let calls = CALLS.load(Relaxed) - calls;
            let revision = result.frame.revision;
            let (edges, expansions, resident, budget) = engine
                .search_diagnostic()
                .filter(|diag| diag.revision == revision && diag.raw_input == result.frame.preedit)
                .map_or((0, 0, 0, None), |diag| {
                    let stats = &diag.statistics;
                    (
                        stats.graph_edges,
                        stats.expansions,
                        stats.resident_high_water,
                        stats.budget,
                    )
                });
            let output = identity(&format!("{result:?}"));
            let cache = engine.search_cache_statistics();
            println!(
                "EVENT\t{case}\t{index}\t{revision}\t{}\t{elapsed}\t{calls}\t{bytes}\t{before}\t{peak}\t{end}\t{edges}\t{expansions}\t{resident}\t{budget:?}\t{output}\t{cache:?}",
                result.frame.preedit.len()
            );
            if let Some((lexicon, model)) = &diagnostics
                && edges != 0
            {
                let (profiled, profile, interpretation) = profile_scheme_query(
                    lexicon,
                    &result.frame.preedit,
                    &config,
                    &scheme,
                    Some(model.model()),
                );
                let (_, profiled) = profiled?;
                assert_eq!(profiled.statistics.graph_edges, edges);
                assert_eq!(profiled.statistics.expansions, expansions);
                assert_eq!(profiled.statistics.resident_high_water, resident);
                assert_eq!(profiled.statistics.budget, budget);
                for (phase, measured) in profile.measurements() {
                    println!(
                        "PHASE\t{index}\t{}\t{}\t{}",
                        phase.name(),
                        measured.nanos,
                        measured.calls
                    );
                }
                println!(
                    "PROFILE\t{index}\t{}\t{interpretation:?}",
                    profile.clock_reads()
                );
            }
            if let Some(commit) = result.commit {
                let start = Instant::now();
                let reply = engine.handle(InputEvent::CommitSucceeded { id: commit.id });
                let elapsed = start.elapsed().as_nanos();
                println!(
                    "ACK\t{index}\t{elapsed}\t{}",
                    identity(&format!("{reply:?}"))
                );
                assert!(reply.error.is_none());
            }
        }
    }
    println!("DONE\t{index}");
    Ok(())
}
