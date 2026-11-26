//! 对原文与模型原始输出做关键内容检查。结果仅用于审阅，不修改译文。

mod glossary;
mod literals;
mod spoken_numbers;
mod structure;
mod tokens;

pub use glossary::{
    Direction, Glossary, GlossaryEntry, GlossaryError, GlossaryTarget, LetterCase,
    MAX_GLOSSARY_BYTES,
};

use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use tokens::{Token, Value, tokenize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindingKind {
    Number,
    Percentage,
    Money,
    Date,
    Time,
    Unit,
    Term,
    FileName,
    Url,
    Version,
    ModelCode,
    NegationMarker,
    ClauseCoverage,
    ConditionalMarker,
    ConditionSubject,
    TemporalMarker,
    CommandAction,
    SwitchSetting,
    QuantityBinding,
    Truncated,
}

impl FindingKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Number => "数值",
            Self::Percentage => "百分比",
            Self::Money => "金额",
            Self::Date => "日期",
            Self::Time => "时间",
            Self::Unit => "计量单位",
            Self::Term => "术语",
            Self::FileName => "文件名",
            Self::Url => "网址",
            Self::Version => "版本号",
            Self::ModelCode => "型号",
            Self::NegationMarker => "命令式否定提示",
            Self::ClauseCoverage => "句段覆盖待核对",
            Self::ConditionalMarker => "条件分支待核对",
            Self::ConditionSubject => "条件对象待核对",
            Self::TemporalMarker => "先后条件待核对",
            Self::CommandAction => "命令动作待核对",
            Self::SwitchSetting => "开关设定变化",
            Self::QuantityBinding => "标签数量对应",
            Self::Truncated => "模型输出",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindingStatus {
    ExactPreserved,
    EquivalentRepresentation,
    Missing,
    Changed,
    Ambiguous,
    Unverifiable,
}

impl FindingStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExactPreserved => "原样保留",
            Self::EquivalentRepresentation => "等价表达",
            Self::Missing => "缺失",
            Self::Changed => "已变化",
            Self::Ambiguous => "有歧义",
            Self::Unverifiable => "无法确认",
        }
    }

    pub fn requires_acknowledgement(self) -> bool {
        !matches!(self, Self::ExactPreserved | Self::EquivalentRepresentation)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityFinding {
    pub kind: FindingKind,
    pub status: FindingStatus,
    /// 原文中的 UTF-8 字节范围。输出新增内容时为空。
    pub source: Option<Range<usize>>,
    pub output: Option<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityReport {
    pub glossary_version: u64,
    pub findings: Vec<IntegrityFinding>,
}

impl IntegrityReport {
    pub fn requires_acknowledgement(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.status.requires_acknowledgement())
    }

    /// 仅由收到可信输出截断信号的调用方使用。长度和末尾标点不用于猜测截断。
    pub fn mark_truncated(&mut self) {
        self.findings.push(IntegrityFinding {
            kind: FindingKind::Truncated,
            status: FindingStatus::Unverifiable,
            source: None,
            output: None,
        });
    }
}

pub fn check_translation(
    source: &str,
    raw_translation: &str,
    direction: Direction,
    glossary: &Glossary,
) -> IntegrityReport {
    let (mut findings, protected_source, protected_output) =
        glossary.check(source, raw_translation, direction);
    let source_tokens = tokenize(source, &protected_source);
    let output_tokens = tokenize(raw_translation, &protected_output);
    findings.extend(compare(
        source,
        raw_translation,
        &source_tokens,
        &output_tokens,
    ));
    findings.extend(literals::negation_mismatch(
        source,
        raw_translation,
        direction,
    ));
    structure::append_findings(
        &mut findings,
        (source, &source_tokens),
        (raw_translation, &output_tokens),
        direction,
    );
    findings.sort_by_key(|finding| {
        finding
            .source
            .as_ref()
            .map_or(usize::MAX, |span| span.start)
    });
    IntegrityReport {
        glossary_version: glossary.version(),
        findings,
    }
}

fn compare(source: &str, output: &str, left: &[Token], right: &[Token]) -> Vec<IntegrityFinding> {
    let mut used = vec![false; right.len()];
    let mut pairs = vec![None; left.len()];
    let mut exact: HashMap<_, VecDeque<_>> = HashMap::new();
    let mut equivalent: HashMap<_, VecDeque<_>> = HashMap::new();
    for (index, token) in right.iter().enumerate() {
        exact
            .entry((&token.value, &output[token.span.clone()]))
            .or_default()
            .push_back(index);
        equivalent.entry(&token.value).or_default().push_back(index);
    }
    for (index, token) in left.iter().enumerate() {
        pairs[index] = take(
            exact.get_mut(&(&token.value, &source[token.span.clone()])),
            &mut used,
        );
    }
    for (index, token) in left.iter().enumerate() {
        if pairs[index].is_none() {
            pairs[index] = take(equivalent.get_mut(&token.value), &mut used);
        }
    }

    let mut by_kind: HashMap<_, VecDeque<_>> = HashMap::new();
    let mut plain: HashMap<_, VecDeque<_>> = HashMap::new();
    let mut annotated: HashMap<_, VecDeque<_>> = HashMap::new();
    for (index, token) in right.iter().enumerate().filter(|(index, _)| !used[*index]) {
        let kind = token.value.kind();
        by_kind.entry(kind).or_default().push_back(index);
        if let Some(number) = token.value.number() {
            let by_number = if kind == FindingKind::Number {
                &mut plain
            } else {
                &mut annotated
            };
            by_number.entry(number).or_default().push_back(index);
        }
    }
    for (index, token) in left.iter().enumerate() {
        if pairs[index].is_none()
            && let Some(number) = token.value.number()
        {
            let by_other = if token.value.kind() == FindingKind::Number {
                &mut annotated
            } else {
                &mut plain
            };
            pairs[index] = take(by_other.get_mut(number), &mut used);
        }
    }
    for (index, token) in left.iter().enumerate() {
        if pairs[index].is_none() {
            pairs[index] = take(by_kind.get_mut(&token.value.kind()), &mut used);
        }
    }

    let mut findings = Vec::with_capacity(left.len());
    for (index, token) in left.iter().enumerate() {
        let counterpart = pairs[index].map(|index| &right[index]);
        let status = counterpart.map_or_else(
            || token.value.uncertainty().unwrap_or(FindingStatus::Missing),
            |other| compare_token(token, other, source, output),
        );
        findings.push(IntegrityFinding {
            kind: token.value.kind(),
            status,
            source: Some(token.span.clone()),
            output: counterpart.map(|other| other.span.clone()),
        });
    }
    findings.extend(
        right
            .iter()
            .zip(used)
            .filter(|(_, used)| !used)
            .map(|(token, _)| IntegrityFinding {
                kind: token.value.kind(),
                status: token
                    .value
                    .uncertainty()
                    .unwrap_or(FindingStatus::Unverifiable),
                source: None,
                output: Some(token.span.clone()),
            }),
    );
    findings
}

fn take(queue: Option<&mut VecDeque<usize>>, used: &mut [bool]) -> Option<usize> {
    let queue = queue?;
    while let Some(index) = queue.pop_front() {
        if !used[index] {
            used[index] = true;
            return Some(index);
        }
    }
    None
}

fn compare_token(source: &Token, other: &Token, left: &str, right: &str) -> FindingStatus {
    use FindingStatus::*;
    if let (
        Value::Money(source_amount, source_currency),
        Value::Money(output_amount, output_currency),
    ) = (&source.value, &other.value)
    {
        if source_amount != output_amount {
            return Changed;
        }
        if source_currency.is_ambiguous() {
            return Ambiguous;
        }
        if output_currency.is_ambiguous() {
            return if output_currency.may_be(*source_currency) {
                Ambiguous
            } else {
                Changed
            };
        }
    }
    if let Some(interest) = source
        .value
        .uncertainty()
        .or_else(|| other.value.uncertainty())
    {
        return interest;
    }
    if source.value != other.value {
        return Changed;
    }
    if left[source.span.clone()] == right[other.span.clone()] {
        ExactPreserved
    } else {
        EquivalentRepresentation
    }
}
