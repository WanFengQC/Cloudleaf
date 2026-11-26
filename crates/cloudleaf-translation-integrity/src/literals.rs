use crate::{Direction, FindingKind, FindingStatus, IntegrityFinding, tokens::Value};
use std::ops::Range;

pub(crate) fn technical(text: &str, previous: Option<char>) -> Option<(usize, Value)> {
    if previous.is_some_and(|ch| component(ch) || matches!(ch, '.' | '@' | '/' | '\\')) {
        return None;
    }
    if let Some(len) = url(text) {
        return Some((len, Value::Literal(FindingKind::Url, text[..len].into())));
    }
    let end = text
        .find(|ch| !(component(ch) || ch == '.'))
        .unwrap_or(text.len());
    if text[end..].starts_with(['@', '/', '\\']) {
        return None;
    }
    let token = text[..end].trim_end_matches('.');
    let kind = if version(token) {
        FindingKind::Version
    } else if file_name(token) {
        FindingKind::FileName
    } else if model_code(token) {
        FindingKind::ModelCode
    } else {
        return None;
    };
    Some((token.len(), Value::Literal(kind, token.into())))
}

fn component(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-')
}

fn url(text: &str) -> Option<usize> {
    let tail = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))?;
    let len = tail
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ".-_/~:%?#[]@!$&'*+,;=".contains(ch)))
        .unwrap_or(tail.len());
    let address = tail[..len].trim_end_matches(['.', ',', ';', ':', '!', '?', '\'', ']']);
    let authority = address.split(['/', '?', '#']).next()?;
    let host = if let Some((host, port)) = authority.split_once(':') {
        port.parse::<u16>().ok()?;
        host
    } else {
        authority
    };
    if !host.contains('.')
        || !host.split('.').all(|label| {
            !label.is_empty()
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == b'-')
        })
    {
        return None;
    }
    Some(text.len() - tail.len() + address.len())
}

fn version(text: &str) -> bool {
    if let Some(parts) = text.strip_prefix('v') {
        let mut parts = parts.split('.');
        (0..3).all(|_| parts.next().is_some_and(is_decimal)) && parts.next().is_none()
    } else {
        false
    }
}

fn is_decimal(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn file_name(text: &str) -> bool {
    let Some((base, extension)) = text.rsplit_once('.') else {
        return false;
    };
    base.starts_with(|ch: char| ch.is_ascii_alphanumeric() || ch == '_')
        && [
            "csv", "docx", "ini", "json", "log", "md", "pdf", "toml", "tsv", "txt", "xlsx", "xml",
            "yaml", "yml", "zip",
        ]
        .iter()
        .any(|allowed| extension.eq_ignore_ascii_case(allowed))
}

fn model_code(text: &str) -> bool {
    let Some((prefix, number)) = text.split_once('-') else {
        return false;
    };
    let digits = number.bytes().take_while(u8::is_ascii_digit).count();
    (2..=8).contains(&prefix.len())
        && prefix.bytes().all(|ch| ch.is_ascii_uppercase())
        && (2..=12).contains(&digits)
        && number.len() - digits <= 3
        && number[digits..].bytes().all(|ch| ch.is_ascii_uppercase())
}

pub(crate) fn negation_mismatch(
    source: &str,
    output: &str,
    direction: Direction,
) -> Option<IntegrityFinding> {
    let chinese_source = direction == Direction::ChineseToEnglish;
    let left = leading_negations(source, chinese_source);
    let right = leading_negations(output, !chinese_source);
    // 数量不同时也无法对齐两种语言的句子，只有唯一单边标记可定位。
    (left.len() != right.len()).then(|| IntegrityFinding {
        kind: FindingKind::NegationMarker,
        status: FindingStatus::Unverifiable,
        source: match (left.as_slice(), right.as_slice()) {
            ([span], []) => Some(span.clone()),
            _ => None,
        },
        output: match (left.as_slice(), right.as_slice()) {
            ([], [span]) => Some(span.clone()),
            _ => None,
        },
    })
}

fn leading_negations(text: &str, chinese: bool) -> Vec<Range<usize>> {
    const BOUNDARIES: [char; 9] = ['.', '!', '?', ';', '。', '！', '？', '；', '\n'];
    const ZH: [&str; 4] = ["请不要", "请勿", "禁止", "勿"];
    const EN: [&str; 6] = [
        "please do not",
        "please don't",
        "please don’t",
        "do not",
        "don't",
        "don’t",
    ];
    text.split_inclusive(BOUNDARIES)
        .scan(0, |offset, part| {
            let clause = part.trim_start();
            let start = *offset + part.len() - clause.len();
            *offset += part.len();
            Some((start, clause.trim_end_matches(BOUNDARIES)))
        })
        .filter_map(|(start, clause)| {
            let width = if chinese {
                ZH.into_iter()
                    .find(|prefix| {
                        clause
                            .strip_prefix(prefix)
                            .is_some_and(|suffix| !suffix.trim().is_empty())
                    })?
                    .len()
            } else {
                EN.into_iter()
                    .find(|prefix| {
                        clause
                            .get(..prefix.len())
                            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
                            && clause[prefix.len()..].starts_with(char::is_whitespace)
                            && !clause[prefix.len()..].trim().is_empty()
                    })?
                    .len()
            };
            Some(start..start + width)
        })
        .collect()
}
