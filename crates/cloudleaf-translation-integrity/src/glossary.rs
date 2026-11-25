use crate::{FindingKind, FindingStatus, IntegrityFinding};
use std::collections::VecDeque;
use std::{fmt, ops::Range};

pub const MAX_GLOSSARY_BYTES: usize = 32 * 1024;
const MAX_GLOSSARY_ENTRIES: usize = 128;
const MAX_TERM_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    ChineseToEnglish,
    EnglishToChinese,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LetterCase {
    Sensitive,
    AsciiInsensitive,
}

#[derive(Clone, PartialEq, Eq)]
pub enum GlossaryTarget {
    Preserve,
    Translate(String),
}

#[derive(Clone, PartialEq, Eq)]
pub struct GlossaryEntry {
    pub direction: Direction,
    pub source: String,
    pub target: GlossaryTarget,
    pub case: LetterCase,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlossaryError {
    InvalidHeader,
    InvalidRow(usize),
    InvalidEntry(usize),
    ConflictingEntries(usize, usize),
    TooLarge,
}

impl fmt::Display for GlossaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => f.write_str("术语表版本头无效"),
            Self::InvalidRow(line) => write!(f, "术语表第 {line} 行格式无效"),
            Self::InvalidEntry(entry) => write!(f, "术语表第 {entry} 项无效"),
            Self::ConflictingEntries(left, right) => {
                write!(f, "术语表第 {left} 项与第 {right} 项冲突")
            }
            Self::TooLarge => f.write_str("术语表超过大小上限"),
        }
    }
}

impl std::error::Error for GlossaryError {}

#[derive(Default, Clone, PartialEq, Eq)]
pub struct Glossary {
    version: u64,
    entries: Vec<GlossaryEntry>,
}

type Span = Range<usize>;
type GlossaryFindings = (Vec<IntegrityFinding>, Vec<Span>, Vec<Span>);

impl Glossary {
    pub fn new(version: u64, entries: Vec<GlossaryEntry>) -> Result<Self, GlossaryError> {
        if entries.len() > MAX_GLOSSARY_ENTRIES {
            return Err(GlossaryError::TooLarge);
        }
        for (index, entry) in entries.iter().enumerate() {
            if !valid_term(&entry.source) || !valid_term(entry.expected()) {
                return Err(GlossaryError::InvalidEntry(index + 1));
            }
            if !entry.enabled {
                continue;
            }
            for (other_index, other) in entries[..index].iter().enumerate() {
                if other.enabled
                    && other.direction == entry.direction
                    && (other.source == entry.source
                        || (other.case == LetterCase::AsciiInsensitive
                            || entry.case == LetterCase::AsciiInsensitive)
                            && other.source.eq_ignore_ascii_case(&entry.source))
                {
                    return Err(GlossaryError::ConflictingEntries(
                        other_index + 1,
                        index + 1,
                    ));
                }
            }
        }
        Ok(Self { version, entries })
    }

    pub fn parse_tsv(text: &str) -> Result<Self, GlossaryError> {
        if text.len() > MAX_GLOSSARY_BYTES {
            return Err(GlossaryError::TooLarge);
        }
        let mut lines = text.lines();
        let header = lines.next().ok_or(GlossaryError::InvalidHeader)?;
        let version = header
            .strip_prefix("cloudleaf-glossary\t1\t")
            .filter(|number| !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|number| number.parse().ok())
            .ok_or(GlossaryError::InvalidHeader)?;
        let mut entries = Vec::new();
        for (row, line) in lines.enumerate() {
            if entries.len() == MAX_GLOSSARY_ENTRIES {
                return Err(GlossaryError::TooLarge);
            }
            entries.push(parse_entry(line).ok_or(GlossaryError::InvalidRow(row + 2))?);
        }
        Self::new(version, entries)
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub(crate) fn check(
        &self,
        source: &str,
        output: &str,
        direction: Direction,
    ) -> GlossaryFindings {
        let active: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.enabled && entry.direction == direction)
            .collect();
        let mut source_hits = Vec::new();
        let mut left_spans = Vec::new();
        let mut cursor = 0;
        while cursor < source.len() && !active.is_empty() {
            let matched = active
                .iter()
                .enumerate()
                .filter(|(_, entry)| matches_at(source, cursor, &entry.source, entry.case))
                .max_by_key(|(_, entry)| entry.source.len());
            if let Some((index, entry)) = matched {
                let span = cursor..cursor + entry.source.len();
                cursor = span.end;
                left_spans.push(span.clone());
                source_hits.push((index, span));
            } else {
                cursor += source[cursor..]
                    .chars()
                    .next()
                    .expect("非空术语输入")
                    .len_utf8();
            }
        }

        // 目标词可能共用前缀。先保护较长目标词，并且每个输出片段只能匹配一次。
        source_hits.sort_by_key(|(index, span)| {
            (
                std::cmp::Reverse(active[*index].expected().len()),
                span.start,
            )
        });
        let mut candidates = vec![None; active.len()];
        let mut occupied = vec![false; output.len()];
        let mut right_spans = Vec::new();
        let mut findings = Vec::new();
        for (index, span) in source_hits {
            let entry = active[index];
            let expected = entry.expected();
            let ranges = candidates[index]
                .get_or_insert_with(|| matching_ranges(output, expected, entry.case));
            let target = ranges
                .iter()
                .position(|range| !occupied[range.clone()].iter().any(|used| *used))
                .and_then(|position| ranges.drain(..=position).next_back());
            let status = if let Some(target) = &target {
                occupied[target.clone()].fill(true);
                right_spans.push(target.clone());
                if source[span.clone()] == output[target.clone()] {
                    FindingStatus::ExactPreserved
                } else {
                    FindingStatus::EquivalentRepresentation
                }
            } else {
                FindingStatus::Missing
            };
            findings.push(IntegrityFinding {
                kind: FindingKind::Term,
                status,
                source: Some(span),
                output: target,
            });
        }
        right_spans.sort_by_key(|span| span.start);
        (findings, left_spans, right_spans)
    }
}

impl GlossaryEntry {
    fn expected(&self) -> &str {
        match &self.target {
            GlossaryTarget::Preserve => &self.source,
            GlossaryTarget::Translate(text) => text,
        }
    }
}

fn parse_entry(line: &str) -> Option<GlossaryEntry> {
    let fields: Vec<_> = line.split('\t').collect();
    let &[direction, enabled, case, source, action, text] = fields.as_slice() else {
        return None;
    };
    let direction = match direction {
        "zh-en" => Direction::ChineseToEnglish,
        "en-zh" => Direction::EnglishToChinese,
        _ => return None,
    };
    let enabled = match enabled {
        "true" => true,
        "false" => false,
        _ => return None,
    };
    let case = match case {
        "sensitive" => LetterCase::Sensitive,
        "ascii-insensitive" => LetterCase::AsciiInsensitive,
        _ => return None,
    };
    let target = match action {
        "preserve" if text.is_empty() => GlossaryTarget::Preserve,
        "translate" if !text.is_empty() => GlossaryTarget::Translate(text.into()),
        _ => return None,
    };
    Some(GlossaryEntry {
        direction,
        source: source.into(),
        target,
        case,
        enabled,
    })
}

fn valid_term(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_TERM_BYTES
        && text.trim() == text
        && !text.chars().any(char::is_control)
}

fn matching_ranges(text: &str, term: &str, case: LetterCase) -> VecDeque<Span> {
    text.char_indices()
        .filter_map(|(offset, _)| {
            matches_at(text, offset, term, case).then_some(offset..offset + term.len())
        })
        .collect()
}

fn matches_at(text: &str, offset: usize, term: &str, case: LetterCase) -> bool {
    let end = offset + term.len();
    let Some(slice) = text.get(offset..end) else {
        return false;
    };
    let equal = match case {
        LetterCase::Sensitive => slice == term,
        LetterCase::AsciiInsensitive => slice.eq_ignore_ascii_case(term),
    };
    equal
        && !(term.as_bytes()[0].is_ascii()
            && text[..offset]
                .chars()
                .next_back()
                .is_some_and(is_identifier))
        && !(term.as_bytes()[term.len() - 1].is_ascii()
            && text[end..].chars().next().is_some_and(is_identifier))
}

fn is_identifier(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}
