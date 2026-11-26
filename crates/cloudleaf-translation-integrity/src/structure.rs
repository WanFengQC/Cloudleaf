use crate::tokens::{Token, Value};
use crate::{Direction, FindingKind as Kind, FindingStatus as Status, IntegrityFinding};
use std::ops::Range;

type Text<'a> = (&'a str, &'a [Token]);

pub(crate) fn append_findings(
    findings: &mut Vec<IntegrityFinding>,
    source: Text<'_>,
    output: Text<'_>,
    direction: Direction,
) {
    let chinese = direction == Direction::ChineseToEnglish;
    let left_sentences = sentences(source);
    let right_sentences = sentences(output);
    if left_sentences.len() >= 2 && left_sentences.len() > right_sentences.len() {
        findings.push(uncertain(
            Kind::ClauseCoverage,
            left_sentences[right_sentences.len()].start..source.0.len(),
            None,
        ));
    }
    let (conditions, alternatives, temporal) = markers(chinese);
    let (other_conditions, other_alternatives, other_temporal) = markers(!chinese);
    let condition = first_marker(source.0, conditions);
    if let Some(span) = condition.clone()
        && first_marker(output.0, other_conditions).is_none()
    {
        findings.push(uncertain(Kind::ConditionalMarker, span, None));
    } else if let Some(span) = first_marker(source.0, alternatives)
        && first_marker(output.0, other_alternatives).is_none()
    {
        findings.push(uncertain(Kind::ConditionalMarker, span, None));
    }
    if let Some(span) = first_marker(source.0, temporal)
        && first_marker(output.0, other_temporal).is_none()
    {
        findings.push(uncertain(Kind::TemporalMarker, span, None));
    }

    let labels = [
        named_labels(source.0, chinese),
        named_labels(output.0, !chinese),
    ];
    if condition.is_some()
        && distinct_labels(&labels[0]) >= 2
        && let Some(label) = labels[0]
            .iter()
            .find(|label| !labels[1].iter().any(|other| other.key == label.key))
    {
        findings.push(uncertain(Kind::ConditionSubject, label.span.clone(), None));
    }
    compare_quantities(findings, source, output, &labels, chinese);

    let left_actions = commands(source.0, &left_sentences, chinese);
    let right_actions = commands(output.0, &right_sentences, !chinese);
    if let ([left], [right]) = (left_actions.as_slice(), right_actions.as_slice())
        && left.family == right.family
        && left.forward != right.forward
    {
        findings.push(uncertain(
            Kind::CommandAction,
            left.span.clone(),
            Some(right.span.clone()),
        ));
    }
    if let Some(span) = added_action_after_greeting(source.0, output.0, chinese) {
        findings.push(IntegrityFinding {
            kind: Kind::AddedCommandAction,
            status: Status::Unverifiable,
            source: None,
            output: Some(span),
        });
    }
    let left = switches(source.0, chinese);
    let right = switches(output.0, !chinese);
    if let [setting] = left.as_slice() {
        match right.as_slice() {
            [other] if setting.label == other.label && setting.on != other.on => {
                findings.push(IntegrityFinding {
                    kind: Kind::SwitchSetting,
                    status: Status::Changed,
                    source: Some(setting.span.clone()),
                    output: Some(other.span.clone()),
                });
            }
            [other] if setting.label == other.label => {}
            _ => findings.push(uncertain(Kind::SwitchSetting, setting.span.clone(), None)),
        }
    }
}

fn uncertain(kind: Kind, source: Range<usize>, output: Option<Range<usize>>) -> IntegrityFinding {
    IntegrityFinding {
        kind,
        status: Status::Unverifiable,
        source: Some(source),
        output,
    }
}

fn sentences((text, tokens): Text<'_>) -> Vec<Range<usize>> {
    let mut start = 0;
    let mut ranges = Vec::new();
    let mut facts = tokens.iter().peekable();
    for (offset, character) in text.char_indices() {
        while facts.peek().is_some_and(|fact| fact.span.end <= offset) {
            facts.next();
        }
        if !matches!(character, '.' | '!' | '?' | '。' | '！' | '？' | '\n')
            || facts.peek().is_some_and(|fact| fact.span.contains(&offset))
        {
            continue;
        }
        let end = offset + character.len_utf8();
        if character == '.'
            && (text[end..].starts_with(|next: char| next.is_ascii_alphanumeric() || next == '.')
                || ["mr.", "mrs.", "ms.", "dr.", "etc.", "no.", "e.g.", "i.e."]
                    .iter()
                    .any(|word| {
                        end.checked_sub(word.len())
                            .and_then(|begin| text.get(begin..end))
                            .is_some_and(|head| head.eq_ignore_ascii_case(word))
                    }))
        {
            continue;
        }
        append_sentence(&mut ranges, text, start..end);
        start = end;
    }
    append_sentence(&mut ranges, text, start..text.len());
    ranges
}

fn append_sentence(ranges: &mut Vec<Range<usize>>, text: &str, range: Range<usize>) {
    let head = text[range.clone()].trim_start();
    if head.chars().any(char::is_alphanumeric) {
        ranges.push(range.end - head.len()..range.end);
    }
}

type Markers = (
    &'static [&'static str],
    &'static [&'static str],
    &'static [&'static str],
);

fn markers(chinese: bool) -> Markers {
    if chinese {
        (
            &["如果", "倘若", "除非", "假如", "若"],
            &["否则", "不然", "否則"],
            &[
                "到达后",
                "抵达后",
                "完成后",
                "确认后",
                "之后",
                "以前",
                "以后",
                "之前",
                "之後",
                "確認後",
                "的时候",
                "的時候",
            ],
        )
    } else {
        (
            &["if", "unless", "provided that"],
            &["otherwise"],
            &[
                "when", "before", "after", "until", "once we", "once you", "once I", "upon",
            ],
        )
    }
}

fn first_marker(text: &str, words: &[&str]) -> Option<Range<usize>> {
    text.char_indices().find_map(|(offset, _)| {
        words.iter().find_map(|&word| {
            if word.is_ascii() && text[..offset].ends_with(ascii_word) {
                return None;
            }
            if word == "若"
                && (text[..offset].ends_with(char::is_alphanumeric)
                    || text[offset..].starts_with("若干"))
            {
                return None;
            }
            let length = prefix_len(&text[offset..], word)?;
            Some(offset..offset + length)
        })
    })
}

fn ascii_word(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn added_action_after_greeting(source: &str, output: &str, chinese: bool) -> Option<Range<usize>> {
    const GREETINGS: &[(&[&str], &[&str])] = &[
        (&["hello", "hi"], &["你好", "您好"]),
        (&["good morning"], &["早上好", "早安"]),
        (&["good afternoon"], &["下午好", "午安"]),
        (&["good evening"], &["晚上好", "晚安"]),
    ];
    let terminal = |ch: char| matches!(ch, '.' | '!' | '?' | '。' | '！' | '？');
    let separators = |ch: char| terminal(ch) || matches!(ch, ',' | '，' | ';' | '；' | '\n');
    let head = source
        .trim()
        .trim_end_matches(|ch: char| terminal(ch) || ch.is_whitespace());
    let targets = GREETINGS.iter().find_map(|&(en, zh)| {
        let (from, to) = if chinese { (zh, en) } else { (en, zh) };
        from.iter()
            .any(|greeting| head.eq_ignore_ascii_case(greeting))
            .then_some(to)
    })?;
    let tail = targets.iter().find_map(|greeting| {
        let output = output.trim_start();
        let width = prefix_len(output, greeting)?;
        let rest = output[width..].trim_start_matches(|ch: char| ch != '\n' && ch.is_whitespace());
        rest.starts_with(separators)
            .then(|| rest.trim_start_matches(|ch: char| separators(ch) || ch.is_whitespace()))
    })?;
    let action = tail
        .split_once(separators)
        .map_or(tail, |(first, _)| first)
        .trim_end();
    let start = output.len() - tail.len();
    let span = start..start + action.len();
    let (polite, invite) = if chinese {
        (&["please"][..], &["come in", "step inside"][..])
    } else {
        (&["请", "請"][..], &["进", "進", "进来", "進來"][..])
    };
    let command = polite.iter().find_map(|word| prefix_len(action, word))?;
    let command = action[command..].trim_start();
    // 只告警已经有明确信号的指令。礼貌套话和没有已知动作的句段交给人工质量评估。
    (invite
        .iter()
        .any(|word| prefix_len(command, word).is_some())
        || !commands(output, std::slice::from_ref(&span), !chinese).is_empty()
        || (!chinese
            && ["开门", "開門", "关门", "關門"]
                .iter()
                .any(|word| prefix_len(command, word).is_some())))
    .then_some(span)
}

fn prefix_len(text: &str, word: &str) -> Option<usize> {
    let (head, tail) = text.split_at_checked(word.len())?;
    (head.eq_ignore_ascii_case(word) && (!word.is_ascii() || !tail.starts_with(ascii_word)))
        .then_some(word.len())
}

struct Label {
    key: u8,
    span: Range<usize>,
}

fn named_labels(text: &str, chinese: bool) -> Vec<Label> {
    let mut result = Vec::new();
    for (offset, character) in text.char_indices() {
        let part = &text[offset..];
        if chinese {
            let key = "甲乙丙丁"
                .chars()
                .position(|value| value == character)
                .map(|value| value as u8)
                .or_else(|| stable_letter(character));
            let suffix = part[character.len_utf8()..].trim_start();
            if let Some(key) = key
                && !text[..offset].ends_with(ascii_word)
                && suffix.starts_with(|character| "组組箱袋桶批".contains(character))
            {
                result.push(Label {
                    key,
                    span: offset
                        ..text.len() - suffix.len() + suffix.chars().next().unwrap().len_utf8(),
                });
            }
        } else if !text[..offset].ends_with(ascii_word)
            && let Some(used) = [
                "case",
                "box",
                "group",
                "batch",
                "bucket",
                "bag",
                "crate",
                "container",
            ]
            .iter()
            .find_map(|noun| prefix_len(part, noun))
        {
            let tail = part[used..].trim_start();
            if let Some(key) = tail.chars().next().and_then(stable_letter)
                && !tail[1..].starts_with(ascii_word)
            {
                result.push(Label {
                    key,
                    span: offset..text.len() - tail.len() + 1,
                });
            }
        }
    }
    result
}

fn stable_letter(character: char) -> Option<u8> {
    matches!(character, 'A'..='D').then(|| character as u8 - b'A')
}

fn distinct_labels(labels: &[Label]) -> u32 {
    labels
        .iter()
        .fold(0u8, |bits, label| bits | (1 << label.key))
        .count_ones()
}

fn measure_after<'a>((text, tokens): Text<'a>, label: &Label, chinese: bool) -> Option<&'a Token> {
    let tail = text[label.span.end..].trim_start();
    let predicates: &[&str] = if chinese {
        &["装有", "裝有", "载有", "載有", "含有", "有", "重"]
    } else {
        &["contains", "carries", "weighs", "holds", "has"]
    };
    let used = predicates.iter().find_map(|verb| prefix_len(tail, verb))?;
    let start = text.len() - tail[used..].trim_start().len();
    let index = tokens
        .binary_search_by_key(&start, |token| token.span.start)
        .ok()?;
    let token = &tokens[index];
    (matches!(token.value, Value::Number(_) | Value::Unit(..))
        && token.value.uncertainty().is_none())
    .then_some(token)
}

fn compare_quantities(
    findings: &mut Vec<IntegrityFinding>,
    source: Text<'_>,
    output: Text<'_>,
    labels: &[Vec<Label>; 2],
    chinese: bool,
) {
    if labels[0].len() < 2
        || labels[0].len() != labels[1].len()
        || distinct_labels(&labels[0]) as usize != labels[0].len()
        || distinct_labels(&labels[1]) as usize != labels[1].len()
    {
        return;
    }
    let mut pairs = Vec::with_capacity(labels[0].len());
    for label in &labels[0] {
        let other = labels[1].iter().find(|label2| label2.key == label.key);
        match (
            measure_after(source, label, chinese),
            other.and_then(|other| measure_after(output, other, !chinese)),
        ) {
            (Some(left), Some(right)) => pairs.push((left, right)),
            _ => return,
        }
    }
    if pairs.iter().any(|(left, _)| {
        pairs
            .iter()
            .filter(|(other, _)| other.value == left.value)
            .count()
            != pairs
                .iter()
                .filter(|(_, other)| other.value == left.value)
                .count()
    }) {
        return;
    }
    findings.extend(
        pairs
            .into_iter()
            .filter(|(left, right)| left.value != right.value)
            .map(|(left, right)| IntegrityFinding {
                kind: Kind::QuantityBinding,
                status: Status::Changed,
                source: Some(left.span.clone()),
                output: Some(right.span.clone()),
            }),
    );
}

struct Action {
    family: usize,
    forward: bool,
    span: Range<usize>,
}

fn commands(text: &str, bounds: &[Range<usize>], chinese: bool) -> Vec<Action> {
    type Variants = (&'static [&'static str], &'static [&'static str]);
    const EN_FAMILIES: &[Variants] = &[
        (&["reconnect", "connect"], &["disconnect", "unplug"]),
        (
            &["turn on", "switch on", "enable"],
            &["turn off", "switch off", "disable"],
        ),
        (&["open"], &["close", "shut"]),
    ];
    const ZH_FAMILIES: &[Variants] = &[
        (
            &["重新连接", "重新連接", "连接", "連接", "接通"],
            &["断开", "斷開", "切断", "切斷", "拔掉"],
        ),
        (&["打开", "打開", "启用", "啟用"], &["关闭", "關閉", "禁用"]),
    ];
    let families = if chinese { ZH_FAMILIES } else { EN_FAMILIES };
    let introductions: &[&str] = if chinese {
        &[
            "请不要",
            "請不要",
            "请勿",
            "請勿",
            "不要",
            "禁止",
            "切勿",
            "勿",
            "请",
            "請",
        ]
    } else {
        &[
            "please do not",
            "please don't",
            "please don’t",
            "do not",
            "don't",
            "don’t",
            "please",
        ]
    };
    bounds
        .iter()
        .filter_map(|span| {
            let phrase = &text[span.clone()];
            let command = introductions
                .iter()
                .find_map(|word| prefix_len(phrase, word))
                .map_or(phrase, |used| phrase[used..].trim_start());
            let offset = span.end - command.len();
            for (family, (forward, backward)) in families.iter().enumerate() {
                for (direction, words) in [(true, forward), (false, backward)] {
                    if let Some(width) = words.iter().find_map(|word| prefix_len(command, word)) {
                        return Some(Action {
                            family,
                            forward: direction,
                            span: offset..offset + width,
                        });
                    }
                }
            }
            None
        })
        .collect()
}

struct Setting<'a> {
    label: &'a str,
    on: bool,
    span: Range<usize>,
}

fn switches(text: &str, chinese: bool) -> Vec<Setting<'_>> {
    let introductions: &[&str] = if chinese {
        &["将", "將", "把"]
    } else {
        &["set"]
    };
    let predicates: &[&str] = if chinese {
        &["设置为", "設置為", "设为", "設為"]
    } else {
        &["to"]
    };
    let states: &[(&str, bool)] = if chinese {
        &[
            ("开启", true),
            ("開啟", true),
            ("打开", true),
            ("关闭", false),
            ("關閉", false),
        ]
    } else {
        &[("on", true), ("off", false)]
    };
    text.char_indices()
        .filter_map(|(offset, _)| {
            if !chinese && text[..offset].ends_with(ascii_word) {
                return None;
            }
            let prefix = introductions
                .iter()
                .find_map(|word| prefix_len(&text[offset..], word))?;
            let target = text[offset + prefix..].trim_start();
            if !target.starts_with(|character: char| character.is_ascii_alphabetic()) {
                return None;
            }
            let len = target.find(|character: char| !ascii_word(character) && character != '-')?;
            let (label, tail) = target.split_at(len);
            let tail = tail.trim_start();
            let predicate = predicates.iter().find_map(|word| prefix_len(tail, word))?;
            let state = tail[predicate..].trim_start();
            let (on, width) = states
                .iter()
                .find_map(|(word, on)| Some((*on, prefix_len(state, word)?)))?;
            if state[width..].starts_with('-') {
                return None;
            }
            Some(Setting {
                label,
                on,
                span: offset..text.len() - state.len() + width,
            })
        })
        .collect()
}
