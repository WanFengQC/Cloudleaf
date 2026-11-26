//! 对当前原文寻找有精确词条支撑的中文区间；其他字节照原样保留。
use crate::{SearchDiagnostic, SearchPath};
use cloudleaf_search::Lexicon;
use cloudleaf_types::{Candidate, MatchKind};
use ring::digest::{Context, SHA256};
use std::ops::Range;

const INPUT_BYTES: usize = cloudleaf_pinyin::MAX_INPUT_LEN;
const LITERAL_SOURCE: &str = "cloudleaf:mixed-literal";
const MIXED_SOURCE: &str = "cloudleaf:mixed";
const MAX_CHINESE_SPANS: usize = 3;
const MIN_UNMARKED_CHINESE_BYTES: usize = 5;

pub(crate) struct Output {
    pub(crate) candidates: Vec<Candidate>,
    pub(crate) diagnostic: Option<SearchDiagnostic>,
    pub(crate) has_literal: bool,
    pub(crate) trailing_literal: bool,
}

impl Output {
    pub(crate) fn literal(raw: &str, diagnostic: Option<SearchDiagnostic>) -> Self {
        Self {
            candidates: vec![raw_candidate(raw)],
            diagnostic,
            has_literal: true,
            trailing_literal: true,
        }
    }
}

pub(crate) fn body_and_punctuation(raw: &str) -> (&str, Option<char>) {
    let mark = match raw.as_bytes().last() {
        Some(b'.') => '。',
        Some(b':') => '：',
        _ => return (raw, None),
    };
    let body = &raw[..raw.len() - 1];
    if !body.is_empty()
        && body
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
    {
        (body, Some(mark))
    } else {
        (raw, None)
    }
}

pub(crate) fn is_literal(candidate: &Candidate) -> bool {
    candidate.source == LITERAL_SOURCE
}

pub(crate) fn raw_candidate(raw: &str) -> Candidate {
    Candidate {
        id: candidate_id(raw, &[]),
        text: raw.into(),
        annotation: None,
        source: LITERAL_SOURCE.into(),
        frequency: None,
        match_kind: MatchKind::Exact,
    }
}

pub(crate) fn protected(raw: &str) -> bool {
    raw.starts_with("www.")
        || raw.bytes().any(|byte| {
            matches!(
                byte,
                b'@' | b'/'
                    | b'\\'
                    | b'_'
                    | b'='
                    | b'+'
                    | b'&'
                    | b'#'
                    | b'$'
                    | b'%'
                    | b'('
                    | b')'
                    | b'['
                    | b']'
                    | b'{'
                    | b'}'
                    | b'`'
                    | b'<'
                    | b'>'
                    | b'|'
            )
        })
        || raw.contains("::")
        || raw.contains('"')
}

#[derive(Clone, Copy)]
struct Step {
    score: i16,
    start: usize,
    previous_chinese: bool,
    spans: usize,
}

pub(crate) enum Plan {
    Chinese,
    Literal,
    Mixed(Vec<Range<usize>>),
}

/// 只有完整词条端点进入可达位图；同音词条数量不影响解释计算量。
pub(crate) fn plan(raw: &str, lexicon: &Lexicon) -> Plan {
    if raw.bytes().any(|byte| !(byte.is_ascii_alphanumeric())) {
        return if raw
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
        {
            Plan::Chinese
        } else {
            Plan::Literal
        };
    }
    let len = raw.len();
    let mut reach = [0u64; INPUT_BYTES + 1];
    for start in (0..len).rev() {
        for end in lexicon.exact_pinyin_ends(raw.as_bytes(), start) {
            reach[start] |= (1 << (end - 1)) | reach[end];
        }
    }

    let complete_chinese = len > 0 && reach[0] & (1 << (len - 1)) != 0;
    let mut literal = [None; INPUT_BYTES + 1];
    let mut chinese = [None; INPUT_BYTES + 1];
    literal[0] = Some(Step {
        score: 0,
        start: 0,
        previous_chinese: false,
        spans: 0,
    });
    for start in 0..len {
        if let Some(step) = literal[start]
            && step.spans < MAX_CHINESE_SPANS
        {
            let mut ends = reach[start];
            while ends != 0 {
                let end = ends.trailing_zeros() as usize + 1;
                ends &= ends - 1;
                if usable_chinese_span(raw.as_bytes(), start, end) {
                    retain(
                        &mut chinese[end],
                        Step {
                            score: step.score + ((end - start) as i16) * 4 - 6,
                            start,
                            previous_chinese: false,
                            spans: step.spans + 1,
                        },
                    );
                }
            }
        }

        // 原文可从开头开始，或在一个完整中文区间之后继续。相邻字面段不需要分开。
        let Some(step) = (if start == 0 {
            literal[0]
        } else {
            chinese[start]
        }) else {
            continue;
        };
        for end in start + 1..=len {
            let token = &raw[start..end];
            let marked = token
                .bytes()
                .any(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());
            if !marked && token.len() < 4 && !(start == 0 && end == len) {
                continue;
            }
            let right_anchor = (u64::BITS - reach[end].leading_zeros()) as usize;
            let token_has_exact_pinyin = reach[start] & (1 << (end - 1)) != 0;
            let anchored = (start != 0 || end != len)
                && (end == len || right_anchor >= end + MIN_UNMARKED_CHINESE_BYTES);
            let english_score = english_spelling_evidence(token).max(
                if anchored && !complete_chinese && !token_has_exact_pinyin {
                    anchored_spelling_evidence(token, end != len)
                } else {
                    0
                },
            );
            retain(
                &mut literal[end],
                Step {
                    score: step.score + english_score,
                    start,
                    previous_chinese: start != 0,
                    spans: step.spans,
                },
            );
        }
    }

    let mut selected_chinese = match (chinese[len], literal[len]) {
        (Some(ch), Some(lit)) => ch.score >= lit.score,
        (Some(_), None) => true,
        _ => false,
    };
    if selected_chinese && chinese[len].is_some_and(|step| step.start == 0) {
        return Plan::Chinese;
    }
    // 没有完整覆盖时仍显示通常的未完成拼音候选，只要没有可靠的字面边界证据。
    if !selected_chinese
        && literal[len].is_none_or(|step| step.spans == 0 && step.score == 0)
        && raw.bytes().all(|byte| byte.is_ascii_lowercase())
        && cloudleaf_pinyin::parse(raw).is_ok_and(|parsed| {
            matches!(
                parsed.status,
                cloudleaf_pinyin::ParseStatus::Complete | cloudleaf_pinyin::ParseStatus::Incomplete
            )
        })
    {
        return Plan::Chinese;
    }
    let mut cursor = len;
    let mut spans = Vec::new();
    while cursor > 0 {
        let step = if selected_chinese {
            chinese[cursor]
        } else {
            literal[cursor]
        }
        .expect("解释前驱已按正向写入");
        if selected_chinese {
            spans.push(step.start..cursor);
        }
        cursor = step.start;
        selected_chinese = step.previous_chinese;
    }
    if spans.is_empty() {
        Plan::Literal
    } else {
        spans.reverse();
        Plan::Mixed(spans)
    }
}

fn retain(slot: &mut Option<Step>, candidate: Step) {
    if slot.is_none_or(|current| {
        (candidate.score, std::cmp::Reverse(candidate.spans))
            > (current.score, std::cmp::Reverse(current.spans))
    }) {
        *slot = Some(candidate);
    }
}

fn usable_chinese_span(raw: &[u8], start: usize, end: usize) -> bool {
    if start == 0 && end == raw.len() {
        return true;
    }
    let marked = |byte: u8| byte.is_ascii_uppercase() || byte.is_ascii_digit();
    // 单个大写首字母也可能属于专名，后续短片段需要比紧邻缩写或数字更长的拼音证据。
    let marked_left = start > 0
        && (raw[start - 1].is_ascii_digit()
            || (start > 1 && raw[start - 1].is_ascii_uppercase() && marked(raw[start - 2])));
    if end - start >= MIN_UNMARKED_CHINESE_BYTES {
        return true;
    }
    let inside_title = raw[..start]
        .iter()
        .rposition(|byte| marked(*byte))
        .is_some_and(|index| {
            raw[index].is_ascii_uppercase() && (index == 0 || !marked(raw[index - 1]))
        });
    // 连续大写或数字可以直接邻接短拼音；驼峰词内部没有这样的边界证据。
    end - start >= 2
        && (marked_left || (!inside_title && raw.get(end).copied().is_some_and(marked)))
}

fn english_spelling_evidence(token: &str) -> i16 {
    let bytes = token.as_bytes();
    // 以字形作为证据仍有歧义，拼音片段恰好拼成英文单词时用户可保留原文。
    if (6..=12).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && !matches!(bytes[0], b'a' | b'e' | b'i' | b'o' | b'u')
        && token.ends_with("ing")
        && bytes[1..bytes.len() - 3]
            .windows(2)
            .any(|pair| matches!(pair, b"ee" | b"oo"))
    {
        39
    } else {
        0
    }
}

fn anchored_spelling_evidence(token: &str, has_right_chinese: bool) -> i16 {
    let bytes = token.as_bytes();
    if !(4..=16).contains(&bytes.len()) {
        return 0;
    }
    let tail = &bytes[bytes.len() - 3..];
    let vowel = |byte: u8| matches!(byte, b'a' | b'e' | b'i' | b'o' | b'u');
    // 英语辅音尾缀的紧邻元音可避免从右侧中文借字母来凑最短字面段。
    if vowel(tail[0]) && matches!(&tail[1..], b"ck" | b"ft" | b"mp" | b"nd" | b"st") {
        return 32;
    }
    if has_right_chinese
        && let Some(title) = bytes.iter().rposition(u8::is_ascii_uppercase)
        && title + 3 < bytes.len()
        && (title == 0 || !bytes[title - 1].is_ascii_uppercase())
        && (matches!(&tail[1..], b"io" | b"ee" | b"oo")
            || (vowel(tail[0])
                && tail[1].is_ascii_lowercase()
                && !vowel(tail[1])
                && tail[2] == b'e'))
    {
        return 32;
    }
    0
}

pub(crate) fn with_punctuation(
    raw: &str,
    mark: char,
    candidates: &mut [Candidate],
    paths: &mut [SearchPath],
) {
    for candidate in candidates {
        let id = candidate_id(raw, &[&candidate.id]);
        if let Some(path) = paths
            .iter_mut()
            .find(|path| path.candidate_id == candidate.id)
        {
            path.candidate_id.clone_from(&id);
        }
        candidate.id = id;
        candidate.text.push(mark);
    }
}

pub(crate) fn merge(
    raw: &str,
    runs: &[(Range<usize>, Vec<Candidate>, Vec<SearchPath>)],
    limit: usize,
) -> (Vec<Candidate>, Vec<SearchPath>) {
    // 每段的候选序保留；跨段先按页内排名之和，再按字节顺序的分段排名。
    let mut ranks = vec![[0; MAX_CHINESE_SPANS]];
    for (index, (_, candidates, _)) in runs.iter().enumerate() {
        ranks = ranks
            .into_iter()
            .flat_map(|previous| {
                (0..candidates.len()).map(move |rank| {
                    let mut next = previous;
                    next[index] = rank;
                    next
                })
            })
            .collect();
        ranks.sort_unstable_by_key(|ranks| (ranks.iter().sum::<usize>(), *ranks));
        ranks.truncate(limit);
    }
    let mut candidates = Vec::with_capacity(limit);
    let mut paths = Vec::with_capacity(limit);
    for ranks in ranks {
        let mut text = String::new();
        let mut cursor = 0;
        let mut chosen = Vec::with_capacity(runs.len());
        let mut edges = Vec::new();
        let mut kind = MatchKind::Exact;
        for ((range, candidates, paths), rank) in runs.iter().zip(ranks) {
            let candidate = &candidates[rank];
            text.push_str(&raw[cursor..range.start]);
            text.push_str(&candidate.text);
            cursor = range.end;
            chosen.push(candidate.id.as_str());
            kind = kind.max(candidate.match_kind);
            if let Some(path) = paths.iter().find(|path| path.candidate_id == candidate.id) {
                edges.extend(path.edges.iter().cloned().map(|mut edge| {
                    edge.start += range.start;
                    edge.end += range.start;
                    edge.raw_start += range.start;
                    edge.raw_end += range.start;
                    if let Some(interpretation) = &mut edge.interpretation {
                        for arc in &mut interpretation.arcs {
                            arc.start += range.start;
                            arc.end += range.start;
                            arc.raw_start += range.start;
                            arc.raw_end += range.start;
                            for change in &mut arc.changes {
                                change.raw_start += range.start;
                                change.raw_end += range.start;
                            }
                        }
                    }
                    edge
                }));
            }
        }
        text.push_str(&raw[cursor..]);
        if candidates
            .iter()
            .any(|existing: &Candidate| existing.text == text)
        {
            continue;
        }
        let id = candidate_id(raw, &chosen);
        paths.push(SearchPath {
            candidate_id: id.clone(),
            score: -(ranks.iter().sum::<usize>() as i64),
            features: Default::default(),
            edges,
        });
        candidates.push(Candidate {
            id,
            text,
            annotation: None,
            source: MIXED_SOURCE.into(),
            frequency: None,
            match_kind: kind,
        });
    }
    (candidates, paths)
}

fn candidate_id(raw: &str, parts: &[&str]) -> String {
    let mut hash = Context::new(&SHA256);
    hash.update(b"cloudleaf-mixed-v1");
    hash.update(&(raw.len() as u64).to_le_bytes());
    hash.update(raw.as_bytes());
    for part in parts {
        hash.update(&(part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    hash.finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
