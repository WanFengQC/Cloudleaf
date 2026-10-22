//! 显式运行的测试宿主；只在终端展示结果，不保存输入历史。

mod benchmark;

use std::io::{self, BufRead};
use std::process::ExitCode;

use cloudleaf_dictionary::{Dictionary, PROTOTYPE_TSV};
use cloudleaf_input::InputEngine;
use cloudleaf_types::{InputEvent, InputFrame, InputMode, Key};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut query = None;
    let mut replay = None;
    let mut dictionary_path = None;
    let mut benchmark_iterations = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "Cloudleaf M2 测试宿主\n  --query <全拼>\n  --replay \"nihao <Down> <Space>\"\n  --dictionary <UTF-8 TSV>\n  --benchmark <迭代数，1..10000>\n无参数时逐行输入事件，:quit 退出。特殊键：<Space> <Enter> <Backspace> <Esc> <Left> <Right> <Up> <Down> <PageUp> <PageDown> <English> <Chinese> <Blur> <Activate> <Deactivate>。"
                );
                return Ok(());
            }
            "--query" => query = Some(args.next().ok_or("--query 缺少拼音")?),
            "--replay" => replay = Some(args.next().ok_or("--replay 缺少事件序列")?),
            "--dictionary" => dictionary_path = Some(args.next().ok_or("--dictionary 缺少路径")?),
            "--benchmark" => {
                let count = args
                    .next()
                    .ok_or("--benchmark 缺少迭代数")?
                    .parse::<usize>()
                    .map_err(|_| "迭代数必须为整数")?;
                if !(1..=10000).contains(&count) {
                    return Err("迭代数必须在 1..10000".into());
                }
                benchmark_iterations = Some(count);
            }
            _ => return Err("未知参数，使用 --help 查看用法".into()),
        }
    }
    if usize::from(query.is_some())
        + usize::from(replay.is_some())
        + usize::from(benchmark_iterations.is_some())
        > 1
    {
        return Err("--query、--replay 与 --benchmark 只能选择一种".into());
    }
    if let Some(iterations) = benchmark_iterations {
        if dictionary_path.is_some() {
            return Err("固定基准只使用版本化原型词典".into());
        }
        return benchmark::run(iterations);
    }
    let dictionary = if let Some(path) = dictionary_path {
        match std::fs::read_to_string(path) {
            Ok(text) => match Dictionary::from_tsv(&text) {
                Ok(dictionary) => dictionary,
                Err(error) => {
                    eprintln!("词典校验失败：{error}；使用空词典，原文回退仍可用。");
                    Dictionary::empty()
                }
            },
            Err(error) => {
                eprintln!(
                    "词典无法读取（{:?}）；使用空词典，原文回退仍可用。",
                    error.kind()
                );
                Dictionary::empty()
            }
        }
    } else {
        Dictionary::from_tsv(PROTOTYPE_TSV).map_err(|error| error.to_string())?
    };
    let mut engine = InputEngine::new(dictionary);
    engine.handle(InputEvent::Activate);
    if let Some(text) = query {
        for character in text.chars() {
            dispatch(&mut engine, InputEvent::Key(Key::Char(character)))?;
        }
        show_frame(&engine.frame());
    } else if let Some(sequence) = replay {
        replay_sequence(&mut engine, &sequence)?;
        show_frame(&engine.frame());
    } else {
        println!(
            "Cloudleaf 原型词典测试宿主；输入 nihao <Space>，:quit 退出。这里只展示核心行为，尚未接入系统输入法。"
        );
        for line in io::stdin().lock().lines() {
            let line = line.map_err(|_| "无法读取终端输入")?;
            if line.trim() == ":quit" {
                break;
            }
            replay_sequence(&mut engine, &line)?;
            show_frame(&engine.frame());
        }
    }
    Ok(())
}

fn replay_sequence(engine: &mut InputEngine, sequence: &str) -> Result<(), String> {
    // 先验证所有命令，避免错误命令出现前已部分提交。
    let mut events = Vec::new();
    for token in sequence.split_whitespace() {
        let event = match token {
            "<Space>" => Some(InputEvent::Key(Key::Space)),
            "<Enter>" => Some(InputEvent::Key(Key::Enter)),
            "<Backspace>" => Some(InputEvent::Key(Key::Backspace)),
            "<Esc>" => Some(InputEvent::Key(Key::Escape)),
            "<Left>" => Some(InputEvent::Key(Key::Left)),
            "<Right>" => Some(InputEvent::Key(Key::Right)),
            "<Up>" => Some(InputEvent::Key(Key::Up)),
            "<Down>" => Some(InputEvent::Key(Key::Down)),
            "<PageUp>" => Some(InputEvent::Key(Key::PageUp)),
            "<PageDown>" => Some(InputEvent::Key(Key::PageDown)),
            "<English>" => Some(InputEvent::SetMode(InputMode::English)),
            "<Chinese>" => Some(InputEvent::SetMode(InputMode::Chinese)),
            "<Blur>" => Some(InputEvent::FocusLost),
            "<Activate>" => Some(InputEvent::Activate),
            "<Deactivate>" => Some(InputEvent::Deactivate),
            _ if token.starts_with('<') => return Err("未知特殊按键，使用 --help 查看用法".into()),
            _ => None,
        };
        if let Some(event) = event {
            events.push(event);
        } else {
            events.extend(
                token
                    .chars()
                    .map(|character| InputEvent::Key(Key::Char(character))),
            );
        }
    }
    for event in events {
        dispatch(engine, event)?;
    }
    Ok(())
}

fn dispatch(engine: &mut InputEngine, event: InputEvent) -> Result<(), String> {
    let result = engine.handle(event);
    if let Some(error) = result.error {
        return Err(error.to_string());
    }
    if let Some(commit) = result.commit {
        // 终端宿主立即确认；M3 平台必须按系统提交结果确认。
        println!("提交 #{}：{}", commit.id, commit.text);
        let ack = engine.handle(InputEvent::CommitSucceeded { id: commit.id });
        if let Some(error) = ack.error {
            return Err(error.to_string());
        }
    } else if !result.handled {
        println!("交回系统：{event:?}");
    }
    Ok(())
}

fn show_frame(frame: &InputFrame) {
    println!(
        "状态={:?} 模式={:?} 修订={} 组合={} 光标={}",
        frame.state, frame.mode, frame.revision, frame.preedit, frame.cursor
    );
    let end = (frame.page_start + frame.page_size).min(frame.candidates.len());
    for (index, candidate) in frame.candidates[frame.page_start..end].iter().enumerate() {
        let marker = if frame.highlighted == Some(frame.page_start + index) {
            '>'
        } else {
            ' '
        };
        println!(
            "{marker}{} {} [{}] {:?} / {}",
            index + 1,
            candidate.text,
            candidate.annotation.as_deref().unwrap_or(""),
            candidate.match_kind,
            candidate.id
        );
    }
}
