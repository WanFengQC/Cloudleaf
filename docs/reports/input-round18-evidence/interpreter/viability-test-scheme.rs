//! 原始按键到有界音节假设图；不读取词典、不改写原文、不枚举完整句子。
use crate::{CoreError, ErrorCode, legal_syllables};
use std::collections::{BTreeSet, VecDeque};

#[path = "xiaohe.rs"]
mod xiaohe;
pub use xiaohe::XIAOHE_LAYOUT_VERSION;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum PinyinScheme {
    #[default]
    FullPinyin,
    AbbreviatedPinyin,
    DoublePinyin(DoublePinyinLayout),
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DoublePinyinLayout {
    Xiaohe,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FuzzyPair {
    ZhZ,
    ChC,
    ShS,
    NL,
    InIng,
    EnEng,
    AnAng,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FuzzyDirection {
    Forward,
    Reverse,
    Both,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FuzzyRule {
    pub pair: FuzzyPair,
    pub direction: FuzzyDirection,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorrectionConfig {
    pub omission: bool,
    pub extra: bool,
    pub adjacent_key: bool,
    pub transpose: bool,
    pub max_edit_cost: u16,
    pub max_input_bytes: usize,
    pub max_candidates: usize,
}
impl Default for CorrectionConfig {
    fn default() -> Self {
        Self {
            omission: true,
            extra: true,
            adjacent_key: true,
            transpose: true,
            max_edit_cost: 1,
            max_input_bytes: 32,
            max_candidates: 128,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterpretationLimits {
    pub max_input_bytes: usize,
    pub max_arcs: usize,
    pub max_work: usize,
    pub max_fuzzy_steps: usize,
}
impl Default for InterpretationLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 64,
            max_arcs: 4096,
            max_work: 200_000,
            max_fuzzy_steps: 2,
        }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchemeConfig {
    pub scheme: PinyinScheme,
    pub fuzzy: Vec<FuzzyRule>,
    pub correction: Option<CorrectionConfig>,
    pub limits: InterpretationLimits,
}
impl SchemeConfig {
    pub fn validate(&self) -> Result<(), CoreError> {
        let invalid = || CoreError::new(ErrorCode::InvalidConfig, "输入方案或解释预算无效");
        if !(1..=64).contains(&self.limits.max_input_bytes)
            || !(1..=16384).contains(&self.limits.max_arcs)
            || !(1..=1_000_000).contains(&self.limits.max_work)
            || !(1..=2).contains(&self.limits.max_fuzzy_steps)
            || self.fuzzy.len() > 7
        {
            return Err(invalid());
        }
        let mut pairs = BTreeSet::new();
        if self.fuzzy.iter().any(|rule| !pairs.insert(rule.pair)) {
            return Err(invalid());
        }
        if let Some(c) = &self.correction
            && (matches!(self.scheme, PinyinScheme::DoublePinyin(_))
                || c.max_edit_cost != 1
                || !(1..=32).contains(&c.max_input_bytes)
                || !(1..=4096).contains(&c.max_candidates)
                || !(c.omission || c.extra || c.adjacent_key || c.transpose))
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
    pub fn accepts_key(&self, key: char) -> bool {
        key.is_ascii_alphabetic() || matches!(key, '\'' | 'ü' | 'Ü')
    }
    pub fn version_id(&self) -> String {
        let scheme = match self.scheme {
            PinyinScheme::FullPinyin => "full-v1",
            PinyinScheme::AbbreviatedPinyin => "abbreviated-v1",
            PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe) => XIAOHE_LAYOUT_VERSION,
        };
        let mut fuzzy = self.fuzzy.clone();
        fuzzy.sort_unstable();
        format!(
            "scheme-v1:{scheme}:{fuzzy:?}:{:?}:{:?}",
            self.correction, self.limits
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BaseMatch {
    Exact,
    Prefix,
    Abbreviation,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PhoneticMatch {
    Exact,
    Prefix,
    Abbreviation,
    Fuzzy,
    Correction,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ChangeKind {
    Fuzzy,
    Omission,
    Extra,
    AdjacentKey,
    Transpose,
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PhoneticChange {
    pub kind: ChangeKind,
    pub raw_start: usize,
    pub raw_end: usize,
    pub from: String,
    pub to: String,
    pub rule_id: &'static str,
    pub cost: u16,
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PhoneticArc {
    pub start: usize,
    pub end: usize,
    pub raw_start: usize,
    pub raw_end: usize,
    pub syllable: String,
    pub kind: PhoneticMatch,
    pub base: BaseMatch,
    pub changes: Vec<PhoneticChange>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterpretationBudget {
    Arcs,
    Work,
    FuzzyAlternatives,
    CorrectionInput,
    CorrectionCandidates,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhoneticHypotheses {
    pub raw: String,
    pub keys: String,
    pub raw_offsets: Vec<usize>,
    pub waiting: bool,
    pub arcs: Vec<PhoneticArc>,
    pub budget: Option<InterpretationBudget>,
    pub work: usize,
}

pub fn interpret(raw: &str, config: &SchemeConfig) -> Result<PhoneticHypotheses, CoreError> {
    config.validate()?;
    let (mut graph, starts, separators) = normalize(raw, config)?;
    if graph.waiting || graph.keys.is_empty() {
        return Ok(graph);
    }
    base_arcs(&mut graph, &starts, &separators, config);
    if graph.budget.is_some() {
        return Ok(graph);
    }
    let complete_exact = complete_exact(&graph);
    if config.scheme == PinyinScheme::AbbreviatedPinyin {
        abbreviation_arcs(&mut graph, &starts, &separators, config);
    }
    if graph.budget.is_none() && !config.fuzzy.is_empty() {
        fuzzy_arcs(&mut graph, config);
    }
    if graph.budget.is_none()
        && !complete_exact
        && let Some(correction) = &config.correction
    {
        correction_arcs(&mut graph, &starts, &separators, config, correction);
    }
    graph.arcs.sort_unstable();
    graph.arcs.dedup();
    Ok(graph)
}

fn normalize(
    raw: &str,
    config: &SchemeConfig,
) -> Result<(PhoneticHypotheses, Vec<usize>, BTreeSet<usize>), CoreError> {
    let invalid = || CoreError::new(ErrorCode::InvalidInput, "按键字符、分隔或长度无效");
    if raw.len() > config.limits.max_input_bytes {
        return Err(CoreError::new(
            ErrorCode::InputTooLong,
            "输入超过方案字节预算",
        ));
    }
    let mut graph = PhoneticHypotheses {
        raw: raw.into(),
        keys: String::new(),
        raw_offsets: vec![0],
        waiting: false,
        arcs: Vec::new(),
        budget: None,
        work: 0,
    };
    let mut starts = Vec::new();
    let mut separators = BTreeSet::new();
    for (offset, ch) in raw.char_indices() {
        match ch {
            '\'' => {
                if graph.keys.is_empty() || graph.waiting {
                    return Err(invalid());
                }
                separators.insert(graph.keys.len());
                graph.waiting = true;
            }
            'a'..='z' | 'A'..='Z' | 'ü' | 'Ü' => {
                graph.keys.push(if matches!(ch, 'ü' | 'Ü') {
                    'v'
                } else {
                    ch.to_ascii_lowercase()
                });
                starts.push(offset);
                graph.raw_offsets.push(offset + ch.len_utf8());
                graph.waiting = false;
            }
            _ => return Err(invalid()),
        }
    }
    Ok((graph, starts, separators))
}

fn spend(graph: &mut PhoneticHypotheses, config: &SchemeConfig) -> bool {
    if graph.work == config.limits.max_work {
        graph.budget = Some(InterpretationBudget::Work);
        false
    } else {
        graph.work += 1;
        true
    }
}
fn add(graph: &mut PhoneticHypotheses, arc: PhoneticArc, config: &SchemeConfig) -> bool {
    if graph.arcs.len() == config.limits.max_arcs {
        graph.budget = Some(InterpretationBudget::Arcs);
        false
    } else {
        graph.arcs.push(arc);
        true
    }
}
fn arc(
    graph: &PhoneticHypotheses,
    starts: &[usize],
    start: usize,
    end: usize,
    syllable: &str,
    base: BaseMatch,
) -> PhoneticArc {
    PhoneticArc {
        start,
        end,
        raw_start: starts[start],
        raw_end: graph.raw_offsets[end],
        syllable: syllable.into(),
        kind: match base {
            BaseMatch::Exact => PhoneticMatch::Exact,
            BaseMatch::Prefix => PhoneticMatch::Prefix,
            BaseMatch::Abbreviation => PhoneticMatch::Abbreviation,
        },
        base,
        changes: Vec::new(),
    }
}
fn end_of_part(start: usize, len: usize, separators: &BTreeSet<usize>) -> usize {
    separators.range(start + 1..).next().copied().unwrap_or(len)
}

fn base_arcs(
    graph: &mut PhoneticHypotheses,
    starts: &[usize],
    separators: &BTreeSet<usize>,
    config: &SchemeConfig,
) {
    for start in 0..graph.keys.len() {
        let stop = end_of_part(start, graph.keys.len(), separators);
        for syllable in legal_syllables() {
            if !spend(graph, config) {
                return;
            }
            let code;
            let spelling = if matches!(config.scheme, PinyinScheme::DoublePinyin(_)) {
                code = xiaohe::xiaohe_code(syllable).unwrap();
                std::str::from_utf8(&code).unwrap()
            } else {
                *syllable
            };
            let available = &graph.keys[start..stop];
            if available.starts_with(spelling) {
                let next = arc(
                    graph,
                    starts,
                    start,
                    start + spelling.len(),
                    syllable,
                    BaseMatch::Exact,
                );
                if !add(graph, next, config) {
                    return;
                }
            } else if stop == graph.keys.len() && spelling.starts_with(available) {
                let next = arc(graph, starts, start, stop, syllable, BaseMatch::Prefix);
                if !add(graph, next, config) {
                    return;
                }
            }
        }
    }
}
fn complete_exact(graph: &PhoneticHypotheses) -> bool {
    let mut positions = vec![false; graph.keys.len() + 1];
    positions[0] = true;
    for start in 0..graph.keys.len() {
        if positions[start] {
            for arc in graph
                .arcs
                .iter()
                .filter(|arc| arc.start == start && arc.base == BaseMatch::Exact)
            {
                positions[arc.end] = true;
            }
        }
    }
    positions[graph.keys.len()]
}
fn abbreviation_arcs(
    graph: &mut PhoneticHypotheses,
    starts: &[usize],
    separators: &BTreeSet<usize>,
    config: &SchemeConfig,
) {
    for start in 0..graph.keys.len() {
        let stop = end_of_part(start, graph.keys.len(), separators);
        for syllable in legal_syllables() {
            if !spend(graph, config) {
                return;
            }
            let initial = ["zh", "ch", "sh"]
                .into_iter()
                .find(|initial| syllable.starts_with(initial));
            for length in [1, initial.map_or(0, str::len)] {
                if length == 0 || length >= syllable.len() || start + length > stop {
                    continue;
                }
                if graph.keys[start..start + length] == syllable[..length] {
                    let next = arc(
                        graph,
                        starts,
                        start,
                        start + length,
                        syllable,
                        BaseMatch::Abbreviation,
                    );
                    if !add(graph, next, config) {
                        return;
                    }
                }
            }
        }
    }
}

fn fuzzy_parts(pair: FuzzyPair) -> (&'static str, &'static str, &'static str, bool) {
    match pair {
        FuzzyPair::ZhZ => ("zh", "z", "fuzzy-zh-z-v1", true),
        FuzzyPair::ChC => ("ch", "c", "fuzzy-ch-c-v1", true),
        FuzzyPair::ShS => ("sh", "s", "fuzzy-sh-s-v1", true),
        FuzzyPair::NL => ("n", "l", "fuzzy-n-l-v1", true),
        FuzzyPair::InIng => ("in", "ing", "fuzzy-in-ing-v1", false),
        FuzzyPair::EnEng => ("en", "eng", "fuzzy-en-eng-v1", false),
        FuzzyPair::AnAng => ("an", "ang", "fuzzy-an-ang-v1", false),
    }
}
fn fuzzy_arcs(graph: &mut PhoneticHypotheses, config: &SchemeConfig) {
    let mut rules = config.fuzzy.clone();
    rules.sort_unstable();
    let originals = graph.arcs.clone();
    for original in originals
        .into_iter()
        .filter(|arc| arc.base == BaseMatch::Exact)
    {
        let mut pending = VecDeque::from([(original.clone(), 0)]);
        let mut seen = BTreeSet::from([original.syllable.clone()]);
        while let Some((current, depth)) = pending.pop_front() {
            if depth == config.limits.max_fuzzy_steps {
                continue;
            }
            for rule in &rules {
                let (a, b, id, initial) = fuzzy_parts(rule.pair);
                for (from, to) in [(a, b), (b, a)] {
                    if (rule.direction == FuzzyDirection::Forward && from != a)
                        || (rule.direction == FuzzyDirection::Reverse && from != b)
                    {
                        continue;
                    }
                    if !spend(graph, config) {
                        return;
                    }
                    let target = if initial {
                        // 单字母平舌声母不能错误匹配zh/ch/sh，n/l亦按整个声母替换。
                        if from.len() == 1 && current.syllable.starts_with(&format!("{from}h")) {
                            None
                        } else {
                            current
                                .syllable
                                .strip_prefix(from)
                                .map(|rest| format!("{to}{rest}"))
                        }
                    } else {
                        current
                            .syllable
                            .strip_suffix(from)
                            .map(|rest| format!("{rest}{to}"))
                    };
                    let Some(target) = target.filter(|target| crate::is_syllable(target)) else {
                        continue;
                    };
                    if !seen.insert(target.clone()) {
                        continue;
                    }
                    if seen.len() > 32 {
                        graph.budget = Some(InterpretationBudget::FuzzyAlternatives);
                        return;
                    }
                    let mut next = current.clone();
                    next.syllable = target;
                    next.kind = PhoneticMatch::Fuzzy;
                    next.changes.push(PhoneticChange {
                        kind: ChangeKind::Fuzzy,
                        raw_start: current.raw_start,
                        raw_end: current.raw_end,
                        from: from.into(),
                        to: to.into(),
                        rule_id: id,
                        cost: 0,
                    });
                    if !add(graph, next.clone(), config) {
                        return;
                    }
                    pending.push_back((next, depth + 1));
                }
            }
        }
    }
}

fn correction_arcs(
    graph: &mut PhoneticHypotheses,
    starts: &[usize],
    separators: &BTreeSet<usize>,
    config: &SchemeConfig,
    correction: &CorrectionConfig,
) {
    if graph.raw.len() > correction.max_input_bytes {
        graph.budget = Some(InterpretationBudget::CorrectionInput);
        return;
    }
    let mut count = 0;
    for start in 0..graph.keys.len() {
        let stop = end_of_part(start, graph.keys.len(), separators);
        for end in start + 1..=stop.min(start + 7) {
            for syllable in legal_syllables() {
                if !spend(graph, config) {
                    return;
                }
                let code;
                let target = if matches!(config.scheme, PinyinScheme::DoublePinyin(_)) {
                    code = xiaohe::xiaohe_code(syllable).unwrap();
                    std::str::from_utf8(&code).unwrap()
                } else {
                    *syllable
                };
                let raw = &graph.keys[start..end];
                let Some((kind, position, removed, added, id)) = one_edit(raw, target, correction)
                else {
                    continue;
                };
                if count == correction.max_candidates {
                    graph.budget = Some(InterpretationBudget::CorrectionCandidates);
                    return;
                }
                let mut next = arc(graph, starts, start, end, syllable, BaseMatch::Exact);
                next.kind = PhoneticMatch::Correction;
                let change_start = if position == end - start {
                    graph.raw_offsets[end]
                } else {
                    starts[start + position]
                };
                let change_end = if removed.is_empty() {
                    change_start
                } else {
                    graph.raw_offsets[start + position + removed.len()]
                };
                next.changes.push(PhoneticChange {
                    kind,
                    raw_start: change_start,
                    raw_end: change_end,
                    from: removed,
                    to: added,
                    rule_id: id,
                    cost: 1,
                });
                if !add(graph, next, config) {
                    return;
                }
                count += 1;
            }
        }
    }
}
fn one_edit(
    raw: &str,
    target: &str,
    config: &CorrectionConfig,
) -> Option<(ChangeKind, usize, String, String, &'static str)> {
    let a = raw.as_bytes();
    let b = target.as_bytes();
    if a == b || a.len().abs_diff(b.len()) > 1 {
        return None;
    }
    let at = a
        .iter()
        .zip(b)
        .position(|(a, b)| a != b)
        .unwrap_or(a.len().min(b.len()));
    if config.omission && b.len() == a.len() + 1 && a[at..] == b[at + 1..] {
        return Some((
            ChangeKind::Omission,
            at,
            String::new(),
            target[at..at + 1].into(),
            "correction-omission-v1",
        ));
    }
    if config.extra && a.len() == b.len() + 1 && a[at + 1..] == b[at..] {
        return Some((
            ChangeKind::Extra,
            at,
            raw[at..at + 1].into(),
            String::new(),
            "correction-extra-v1",
        ));
    }
    if a.len() != b.len() {
        return None;
    }
    if config.adjacent_key && at < a.len() && a[at + 1..] == b[at + 1..] && adjacent(a[at], b[at]) {
        return Some((
            ChangeKind::AdjacentKey,
            at,
            raw[at..at + 1].into(),
            target[at..at + 1].into(),
            "correction-qwerty-neighbor-v1",
        ));
    }
    if config.transpose
        && at + 1 < a.len()
        && a[at] == b[at + 1]
        && a[at + 1] == b[at]
        && a[at + 2..] == b[at + 2..]
    {
        return Some((
            ChangeKind::Transpose,
            at,
            raw[at..at + 2].into(),
            target[at..at + 2].into(),
            "correction-transpose-v1",
        ));
    }
    None
}
fn adjacent(a: u8, b: u8) -> bool {
    let position = |key| {
        [
            b"qwertyuiop".as_slice(),
            b"asdfghjkl".as_slice(),
            b"zxcvbnm".as_slice(),
        ]
        .into_iter()
        .enumerate()
        .find_map(|(row, keys)| {
            keys.iter()
                .position(|value| *value == key)
                .map(|col| (row as i32, col as i32 * 4 + [0, 1, 3][row]))
        })
    };
    match (position(a), position(b)) {
        (Some((ar, ac)), Some((br, bc))) => (ar - br).abs() <= 1 && (ac - bc).abs() <= 4,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn has(
        graph: &PhoneticHypotheses,
        start: usize,
        end: usize,
        syllable: &str,
        kind: PhoneticMatch,
    ) -> bool {
        graph
            .arcs
            .iter()
            .any(|a| a.start == start && a.end == end && a.syllable == syllable && a.kind == kind)
    }
    #[test]
    fn raw_unicode_ranges_case_and_separator_are_not_syllable_indexes() {
        let graph = interpret("NÜ'hao", &SchemeConfig::default()).unwrap();
        assert_eq!(graph.raw, "NÜ'hao");
        assert_eq!(graph.keys, "nvhao");
        let nv = graph
            .arcs
            .iter()
            .find(|a| a.syllable == "nv" && a.start == 0)
            .unwrap();
        assert_eq!((nv.raw_start, nv.raw_end), (0, 3));
        let hao = graph
            .arcs
            .iter()
            .find(|a| a.syllable == "hao" && a.start == 2)
            .unwrap();
        assert_eq!((hao.raw_start, hao.raw_end), (4, 7));
        assert!(!graph.arcs.iter().any(|a| a.start < 2 && a.end > 2));
        assert!(interpret("ni'", &SchemeConfig::default()).unwrap().waiting);
        for bad in ["'ni", "ni''hao", "ni hao", "ni2", "☁"] {
            assert!(interpret(bad, &SchemeConfig::default()).is_err(), "{bad}");
        }
    }
    #[test]
    fn abbreviation_and_mixed_arcs_keep_exact_syllables_and_raw_positions() {
        let config = SchemeConfig {
            scheme: PinyinScheme::AbbreviatedPinyin,
            ..SchemeConfig::default()
        };
        for input in ["zgr", "bjdx", "zg", "zhg", "zhonggr", "wzgs", "nihao", "nh"] {
            let graph = interpret(input, &config).unwrap();
            assert!(graph.budget.is_none(), "{input}");
            assert!(
                graph
                    .arcs
                    .iter()
                    .all(|a| a.start < a.end && a.raw_end <= input.len())
            );
        }
        let graph = interpret("zhonggr", &config).unwrap();
        assert!(has(&graph, 0, 5, "zhong", PhoneticMatch::Exact));
        assert!(has(&graph, 5, 6, "guo", PhoneticMatch::Abbreviation));
        assert!(has(&graph, 6, 7, "ren", PhoneticMatch::Abbreviation));
        let graph = interpret("nihao", &config).unwrap();
        assert!(has(&graph, 0, 2, "ni", PhoneticMatch::Exact));
        assert!(has(&graph, 2, 5, "hao", PhoneticMatch::Exact));
    }
    #[test]
    fn xiaohe_pairs_zero_initials_collisions_and_unfinished_keys() {
        let config = SchemeConfig {
            scheme: PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
            ..SchemeConfig::default()
        };
        for syllable in legal_syllables() {
            assert!(xiaohe::xiaohe_code(syllable).is_some(), "{syllable}");
        }
        let graph = interpret("NIHC", &config).unwrap();
        assert!(has(&graph, 0, 2, "ni", PhoneticMatch::Exact));
        assert!(has(&graph, 2, 4, "hao", PhoneticMatch::Exact));
        let graph = interpret("lo", &config).unwrap();
        assert!(has(&graph, 0, 2, "lo", PhoneticMatch::Exact));
        assert!(has(&graph, 0, 2, "luo", PhoneticMatch::Exact));
        let graph = interpret("n", &config).unwrap();
        assert!(graph.arcs.iter().all(|a| a.kind == PhoneticMatch::Prefix));
        assert!(has(&graph, 0, 1, "ni", PhoneticMatch::Prefix));
        assert!(
            !interpret("aa'ah", &config)
                .unwrap()
                .arcs
                .iter()
                .any(|a| a.start < 2 && a.end > 2)
        );
    }
    #[test]
    fn fuzzy_is_directional_composable_and_does_not_rewrite_source() {
        let mut config = SchemeConfig {
            fuzzy: vec![
                FuzzyRule {
                    pair: FuzzyPair::NL,
                    direction: FuzzyDirection::Forward,
                },
                FuzzyRule {
                    pair: FuzzyPair::AnAng,
                    direction: FuzzyDirection::Forward,
                },
            ],
            ..SchemeConfig::default()
        };
        let graph = interpret("nan", &config).unwrap();
        assert_eq!(graph.raw, "nan");
        assert!(has(&graph, 0, 3, "nan", PhoneticMatch::Exact));
        assert!(has(&graph, 0, 3, "lang", PhoneticMatch::Fuzzy));
        assert!(!has(
            &interpret("lan", &config).unwrap(),
            0,
            3,
            "nan",
            PhoneticMatch::Fuzzy
        ));
        config.fuzzy[0].direction = FuzzyDirection::Both;
        assert!(has(
            &interpret("lan", &config).unwrap(),
            0,
            3,
            "nan",
            PhoneticMatch::Fuzzy
        ));
        config.fuzzy.clear();
        assert!(
            !interpret("nan", &config)
                .unwrap()
                .arcs
                .iter()
                .any(|a| a.kind == PhoneticMatch::Fuzzy)
        );
        let config = SchemeConfig {
            fuzzy: vec![FuzzyRule {
                pair: FuzzyPair::ZhZ,
                direction: FuzzyDirection::Reverse,
            }],
            ..SchemeConfig::default()
        };
        assert!(
            !interpret("zhang", &config)
                .unwrap()
                .arcs
                .iter()
                .any(|a| a.syllable == "zhhang")
        );
    }
    #[test]
    fn correction_records_actual_edit_and_never_treats_legal_oov_as_a_typo() {
        let config = SchemeConfig {
            correction: Some(CorrectionConfig::default()),
            ..SchemeConfig::default()
        };
        for (raw, syllable, kind) in [
            ("zhnog", "zhong", ChangeKind::Transpose),
            ("zongg", "zong", ChangeKind::Extra),
            ("zhng", "zhong", ChangeKind::Omission),
            ("zhpng", "zhong", ChangeKind::AdjacentKey),
        ] {
            let graph = interpret(raw, &config).unwrap();
            assert!(
                graph.arcs.iter().any(|a| a.start == 0
                    && a.end == raw.len()
                    && a.syllable == syllable
                    && a.changes.iter().any(|c| c.kind == kind)),
                "{raw}: {:?}",
                graph.budget
            );
        }
        let graph = interpret("nihao", &config).unwrap();
        assert!(
            !graph
                .arcs
                .iter()
                .any(|a| a.kind == PhoneticMatch::Correction)
        );
        assert!(graph.arcs.iter().all(|a| crate::is_syllable(&a.syllable)));
    }
    #[test]
    fn correction_budget_ignores_arcs_that_cannot_join_a_complete_one_edit_path() {
        let config = SchemeConfig {
            correction: Some(CorrectionConfig::default()),
            ..SchemeConfig::default()
        };
        let graph = interpret("jintiantiqnqihenhao", &config).unwrap();
        assert!(graph.budget.is_none(), "{:?}", graph.budget);
        assert!(
            graph
                .arcs
                .iter()
                .any(|arc| arc.kind == PhoneticMatch::Correction
                    && arc.syllable == "tian"
                    && &graph.raw[arc.raw_start..arc.raw_end] == "tiqn")
        );
        assert!(
            graph
                .arcs
                .iter()
                .filter(|arc| arc.kind == PhoneticMatch::Correction)
                .count()
                < 128
        );
    }
    #[test]
    fn budgets_reject_invalid_config_and_report_deterministic_exhaustion() {
        let mut config = SchemeConfig::default();
        config.limits.max_work = 1;
        let graph = interpret("zhongguo", &config).unwrap();
        assert_eq!(graph.budget, Some(InterpretationBudget::Work));
        assert_eq!(graph.work, 1);
        config.limits.max_work = 200000;
        config.limits.max_arcs = 1;
        assert_eq!(
            interpret("n", &config).unwrap().budget,
            Some(InterpretationBudget::Arcs)
        );
        config.fuzzy = vec![
            FuzzyRule {
                pair: FuzzyPair::NL,
                direction: FuzzyDirection::Both
            };
            2
        ];
        assert!(config.validate().is_err());
        config = SchemeConfig {
            correction: Some(CorrectionConfig {
                max_edit_cost: 2,
                ..CorrectionConfig::default()
            }),
            ..SchemeConfig::default()
        };
        assert!(config.validate().is_err());
        config = SchemeConfig {
            scheme: PinyinScheme::AbbreviatedPinyin,
            ..SchemeConfig::default()
        };
        let raw = "z".repeat(64);
        assert_eq!(
            interpret(&raw, &config).unwrap(),
            interpret(&raw, &config).unwrap()
        );
    }
    #[test]
    fn configuration_identity_is_order_independent_but_rules_and_layouts_are_bound() {
        let a = FuzzyRule {
            pair: FuzzyPair::NL,
            direction: FuzzyDirection::Both,
        };
        let b = FuzzyRule {
            pair: FuzzyPair::InIng,
            direction: FuzzyDirection::Forward,
        };
        let mut c = SchemeConfig {
            fuzzy: vec![a, b],
            ..SchemeConfig::default()
        };
        let version = c.version_id();
        let first_graph = interpret("nin", &c).unwrap();
        c.fuzzy.reverse();
        assert_eq!(version, c.version_id());
        assert_eq!(first_graph, interpret("nin", &c).unwrap());
        c.fuzzy[0].direction = FuzzyDirection::Both;
        assert_ne!(version, c.version_id());
        assert!(!c.is_default());
        assert!(SchemeConfig::default().is_default());
        assert!(c.version_id().is_ascii());
    }
}
