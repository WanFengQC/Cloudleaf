//! 手写固定语料验证真实模块组合，期望值不由待测实现生成。

use std::collections::BTreeSet;

use cloudleaf_dictionary::{Dictionary, PROTOTYPE_TSV};
use cloudleaf_input::{InputEngine, MAX_CANDIDATES};
use cloudleaf_types::{InputEvent, InputFrame, InputMode, InputState, Key};

const CORPUS: &str = include_str!("../../../tests/fixtures/input-v1.tsv");

fn mode(value: &str) -> InputMode {
    match value {
        "zh" => InputMode::Chinese,
        "en" => InputMode::English,
        _ => panic!("未知语料模式：{value}"),
    }
}

fn state(value: &str) -> InputState {
    match value {
        "Inactive" => InputState::Inactive,
        "Idle" => InputState::Idle,
        "Composing" => InputState::Composing,
        "CandidatesVisible" => InputState::CandidatesVisible,
        "Committing" => InputState::Committing,
        _ => panic!("未知语料状态：{value}"),
    }
}

fn expected_values(value: &str) -> Vec<&str> {
    if value == "-" {
        Vec::new()
    } else {
        value.split('|').collect()
    }
}

fn assert_invariants(frame: &InputFrame, id: &str) {
    assert!(frame.preedit.is_ascii(), "{id}：组合必须为 ASCII");
    assert!(frame.cursor <= frame.preedit.len(), "{id}：光标越界");
    assert!(frame.candidates.len() <= MAX_CANDIDATES, "{id}：候选无界");
    let mut unique = BTreeSet::new();
    for candidate in &frame.candidates {
        assert!(unique.insert(&candidate.text), "{id}：重复候选");
    }
    match frame.highlighted {
        Some(index) => {
            assert!(index < frame.candidates.len(), "{id}：高亮越界");
            assert!(
                (frame.page_start..frame.page_start + frame.page_size).contains(&index),
                "{id}：高亮必须位于当前页"
            );
        }
        None => assert!(frame.candidates.is_empty(), "{id}：有候选却无高亮"),
    }
    if matches!(frame.state, InputState::Inactive | InputState::Idle) {
        assert!(frame.preedit.is_empty(), "{id}：空闲时残留组合");
        assert!(frame.candidates.is_empty(), "{id}：空闲时残留候选");
        assert_eq!(frame.cursor, 0, "{id}：空闲时残留光标");
    }
}

fn dispatch(engine: &mut InputEngine, event: InputEvent, commits: &mut Vec<String>, id: &str) {
    let before = engine.frame();
    let result = engine.handle(event);
    assert!(
        result.error.is_none(),
        "{id}：{event:?} 返回 {:?}",
        result.error
    );
    assert_invariants(&result.frame, id);
    if matches!(event, InputEvent::Key(_))
        && (before.state == InputState::Inactive || before.mode == InputMode::English)
    {
        assert!(!result.handled, "{id}：未激活或英文按键必须透传");
        assert!(result.commit.is_none(), "{id}：透传不能额外提交");
    }
    if let Some(request) = result.commit {
        assert!(result.handled, "{id}：提交事件必须被消费");
        assert_eq!(result.frame.state, InputState::Committing);
        commits.push(request.text);
        // 测试宿主模拟同步成功；失败与迟到确认另由状态机单元测试覆盖。
        let accepted = engine.handle(InputEvent::CommitSucceeded { id: request.id });
        assert!(accepted.error.is_none(), "{id}：确认失败");
        assert!(accepted.commit.is_none(), "{id}：确认不得再输出文本");
        assert_invariants(&accepted.frame, id);
    }
}

fn execute(engine: &mut InputEngine, token: &str, commits: &mut Vec<String>, id: &str) {
    if let Some(text) = token.strip_prefix("text:") {
        assert!(!text.is_empty(), "{id}：文本事件不能为空");
        for character in text.chars() {
            dispatch(engine, InputEvent::Key(Key::Char(character)), commits, id);
        }
        return;
    }
    if let Some(value) = token.strip_prefix("char:") {
        let mut characters = value.chars();
        let character = characters.next().expect("字符事件不能为空");
        assert!(
            characters.next().is_none(),
            "{id}：字符事件必须恰好一个字符"
        );
        dispatch(engine, InputEvent::Key(Key::Char(character)), commits, id);
        return;
    }
    let event = match token {
        "space" => InputEvent::Key(Key::Space),
        "enter" => InputEvent::Key(Key::Enter),
        "escape" => InputEvent::Key(Key::Escape),
        "backspace" => InputEvent::Key(Key::Backspace),
        "left" => InputEvent::Key(Key::Left),
        "right" => InputEvent::Key(Key::Right),
        "up" => InputEvent::Key(Key::Up),
        "down" => InputEvent::Key(Key::Down),
        "pageup" => InputEvent::Key(Key::PageUp),
        "pagedown" => InputEvent::Key(Key::PageDown),
        "focus_lost" => InputEvent::FocusLost,
        "deactivate" => InputEvent::Deactivate,
        "mode:en" => InputEvent::SetMode(InputMode::English),
        "mode:zh" => InputEvent::SetMode(InputMode::Chinese),
        _ => panic!("{id}：未知语料事件 {token}"),
    };
    dispatch(engine, event, commits, id);
}

#[test]
fn fixed_input_corpus_and_candidate_ranking() {
    let mut lines = CORPUS.lines();
    assert_eq!(lines.next(), Some("# cloudleaf-input-corpus-v1"));
    assert_eq!(
        lines.next(),
        Some("id\tinitial_mode\tevents\tpreedit\tcandidates\thighlighted\tcommits\tstate")
    );
    let mut ids = BTreeSet::new();
    let mut count = 0;
    println!("id\tpreedit\tcandidates\thighlighted\tcommits\tstate");
    for line in lines {
        assert!(!line.is_empty(), "语料中不能用空行跳过用例");
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 8, "语料必须包含八个字段：{line}");
        let id = fields[0];
        assert!(!id.is_empty() && ids.insert(id), "语料 ID 必须唯一");
        let mut engine = InputEngine::new(Dictionary::from_tsv(PROTOTYPE_TSV).unwrap());
        let mut commits = Vec::new();
        dispatch(&mut engine, InputEvent::Activate, &mut commits, id);
        dispatch(
            &mut engine,
            InputEvent::SetMode(mode(fields[1])),
            &mut commits,
            id,
        );
        assert!(!fields[2].is_empty(), "{id}：事件串不能为空");
        for token in fields[2].split_ascii_whitespace() {
            execute(&mut engine, token, &mut commits, id);
        }
        let frame = engine.frame();
        assert_eq!(
            frame.preedit,
            if fields[3] == "-" { "" } else { fields[3] },
            "{id}：组合文本"
        );
        let candidates: Vec<_> = frame
            .candidates
            .iter()
            .map(|candidate| candidate.text.as_str())
            .collect();
        assert_eq!(candidates, expected_values(fields[4]), "{id}：候选顺序");
        let highlighted = if fields[5] == "-" {
            None
        } else {
            Some(fields[5].parse::<usize>().unwrap())
        };
        assert_eq!(frame.highlighted, highlighted, "{id}：高亮");
        assert_eq!(commits, expected_values(fields[6]), "{id}：提交文本");
        assert_eq!(frame.state, state(fields[7]), "{id}：最终状态");
        println!(
            "{}\t{}\t{}\t{}\t{}\t{:?}",
            id,
            fields[3],
            candidates.join("|"),
            fields[5],
            commits.join("|"),
            frame.state
        );
        count += 1;
    }
    assert!(count >= 30, "固定语料数量不足");
    println!("通过固定输入语料：{count} 条");
}
