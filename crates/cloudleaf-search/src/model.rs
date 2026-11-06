use crate::{ErrorKind, Lexicon, SearchError};
use std::collections::{BTreeMap, BTreeSet};

const BOS: &str = "<bos>";
const EOS: &str = "<eos>";
const UNK: &str = "<unk>";

/// 独立训练快照的一阶模型；只存计数，不保存原始语句。
#[derive(Clone, Debug)]
pub struct BigramModel {
    vocabulary: BTreeSet<String>,
    pairs: BTreeMap<(String, String), u64>,
    totals: BTreeMap<String, u64>,
    lexical: BTreeMap<String, u64>,
    pub(crate) character: Option<crate::character::CharacterModel>,
}

impl BigramModel {
    pub fn train(sequences: &[Vec<String>]) -> Result<Self, SearchError> {
        Self::train_tokens(sequences, false)
    }

    fn train_tokens(sequences: &[Vec<String>], characters: bool) -> Result<Self, SearchError> {
        if sequences.len() > 100_000
            || sequences.iter().map(Vec::len).sum::<usize>() > 1_000_000
            || sequences.iter().flatten().any(|s| {
                s.is_empty()
                    || s.len() > 128
                    || if characters {
                        s.chars().count() != 1
                    } else {
                        s.starts_with('<')
                    }
                    || s.chars().any(char::is_whitespace)
            })
        {
            return Err(SearchError::new(
                ErrorKind::InvalidModel,
                "训练快照无效或超限",
            ));
        }
        let mut model = Self {
            vocabulary: BTreeSet::from([EOS.into(), UNK.into()]),
            pairs: BTreeMap::new(),
            totals: BTreeMap::new(),
            lexical: BTreeMap::new(),
            character: None,
        };
        for sequence in sequences {
            let mut previous = BOS;
            for token in sequence
                .iter()
                .map(String::as_str)
                .chain(std::iter::once(EOS))
            {
                model.vocabulary.insert(token.into());
                *model
                    .pairs
                    .entry((previous.into(), token.into()))
                    .or_default() += 1;
                *model.totals.entry(previous.into()).or_default() += 1;
                previous = token;
            }
        }
        Ok(model)
    }

    fn known<'a>(&self, token: &'a str) -> &'a str {
        if token == BOS || self.vocabulary.contains(token) {
            token
        } else {
            UNK
        }
    }

    pub fn transition(&self, previous: Option<&str>, next: Option<&str>) -> i64 {
        let previous = self.known(previous.unwrap_or(BOS));
        let next = self.known(next.unwrap_or(EOS));
        let numerator = self
            .pairs
            .get(&(previous.into(), next.into()))
            .copied()
            .unwrap_or(0)
            + 1;
        let denominator =
            self.totals.get(previous).copied().unwrap_or(0) + self.vocabulary.len() as u64;
        log2_fixed(numerator) - log2_fixed(denominator)
    }

    pub fn lexical_units(&self, id: &str) -> Option<i64> {
        self.lexical.get(id).map(|count| log2_fixed(count + 1))
    }

    pub fn has_transitions(&self) -> bool {
        !self.pairs.is_empty()
    }

    pub fn has_lexical_signal(&self) -> bool {
        !self.lexical.is_empty()
    }

    pub fn has_character_transitions(&self) -> bool {
        self.character.is_some()
    }

    /// 字符缓存按不可变词条索引编译；新词典实例须在装载阶段重新绑定。
    pub fn is_compatible_lexicon(&self, lexicon: &Lexicon) -> bool {
        self.character
            .as_ref()
            .is_none_or(|model| std::sync::Arc::ptr_eq(&model.lexicon_identity, &lexicon.identity))
    }

    /// 固定Rust1.90、BTree节点至少五项的布局估计，不是allocator硬上界。
    pub fn estimated_bytes(&self) -> usize {
        let node = |len: usize, item: usize| (len.div_ceil(5) + 1) * (32 + 12 * 8 + 11 * item);
        std::mem::size_of::<Self>()
            + self.vocabulary.iter().map(String::capacity).sum::<usize>()
            + self
                .pairs
                .keys()
                .map(|(a, b)| a.capacity() + b.capacity())
                .sum::<usize>()
            + self.totals.keys().map(String::capacity).sum::<usize>()
            + self.lexical.keys().map(String::capacity).sum::<usize>()
            + node(self.vocabulary.len(), std::mem::size_of::<String>())
            + node(
                self.pairs.len(),
                std::mem::size_of::<((String, String), u64)>(),
            )
            + node(self.totals.len(), std::mem::size_of::<(String, u64)>())
            + node(self.lexical.len(), std::mem::size_of::<(String, u64)>())
            + self
                .character
                .as_ref()
                .map_or(0, |model| model.estimated_bytes())
    }
}

/// 固定来源声明不构成生产批准；本格式只接收显式研发材料。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelMetadata {
    pub lexicon_sha256: String,
    pub source_sha256: String,
    pub source_id: String,
    pub license_id: String,
    pub tokenization: String,
    pub usage: String,
    pub kind: String,
    pub training_split: String,
}

#[derive(Clone, Debug)]
pub struct LanguageModelSnapshot {
    metadata: ModelMetadata,
    model: BigramModel,
}

const META_KEYS: [&str; 8] = [
    "lexicon_sha256",
    "source_sha256",
    "source_id",
    "license_id",
    "tokenization",
    "usage",
    "kind",
    "training_split",
];

impl LanguageModelSnapshot {
    pub fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    pub fn estimated_bytes(&self) -> usize {
        let m = &self.metadata;
        self.model.estimated_bytes()
            + std::mem::size_of::<ModelMetadata>()
            + [
                &m.lexicon_sha256,
                &m.source_sha256,
                &m.source_id,
                &m.license_id,
                &m.tokenization,
                &m.usage,
                &m.kind,
                &m.training_split,
            ]
            .iter()
            .map(|value| value.capacity())
            .sum::<usize>()
    }
    pub fn model(&self) -> &BigramModel {
        &self.model
    }

    pub fn from_tsv(
        text: &str,
        lexicon: &Lexicon,
        expected_lexicon: &str,
    ) -> Result<Self, SearchError> {
        let invalid =
            || SearchError::new(ErrorKind::InvalidModel, "固定语言模型格式、引用或预算无效");
        if text.len() > 16 * 1024 * 1024 || text.contains('\r') || !text.ends_with('\n') {
            return Err(invalid());
        }
        let mut lines = text.lines();
        if lines.next() != Some("# cloudleaf-language-v1") {
            return Err(invalid());
        }
        let mut values = Vec::new();
        for key in META_KEYS {
            let (actual, value) = lines
                .next()
                .and_then(|line| line.split_once('\t'))
                .ok_or_else(invalid)?;
            if actual != key
                || value.is_empty()
                || value.len() > 256
                || value.chars().any(char::is_control)
            {
                return Err(invalid());
            }
            values.push(value.to_owned());
        }
        if lines.next() != Some("previous\tnext\tcount") {
            return Err(invalid());
        }
        let metadata = ModelMetadata {
            lexicon_sha256: values[0].clone(),
            source_sha256: values[1].clone(),
            source_id: values[2].clone(),
            license_id: values[3].clone(),
            tokenization: values[4].clone(),
            usage: values[5].clone(),
            kind: values[6].clone(),
            training_split: values[7].clone(),
        };
        let sha = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        if !sha(&metadata.lexicon_sha256)
            || !sha(&metadata.source_sha256)
            || metadata.lexicon_sha256 != expected_lexicon
            || metadata.training_split != "train"
            || !matches!(metadata.usage.as_str(), "mechanical" | "research_only")
            || !matches!(
                metadata.kind.as_str(),
                "lexical" | "bigram" | "character_bigram"
            )
            || (metadata.kind == "character_bigram" && metadata.tokenization != "unicode-scalar-v1")
        {
            return Err(invalid());
        }
        let ids: BTreeSet<&str> = lexicon
            .entries()
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        let mut model = BigramModel {
            vocabulary: BTreeSet::from([EOS.into(), UNK.into()]),
            pairs: BTreeMap::new(),
            totals: BTreeMap::new(),
            lexical: BTreeMap::new(),
            character: None,
        };
        let mut rows = 0;
        let mut incoming = BTreeMap::<String, u64>::new();
        for line in lines {
            rows += 1;
            if rows > 200_000 {
                return Err(invalid());
            }
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() != 3 || !fields[2].bytes().all(|b| b.is_ascii_digit()) {
                return Err(invalid());
            }
            let count = fields[2].parse::<u64>().map_err(|_| invalid())?;
            if count == 0 || count > 1_000_000_000_000 {
                return Err(invalid());
            }
            let (previous, next) = (fields[0], fields[1]);
            if metadata.kind == "lexical" {
                if previous != "<lexical>"
                    || !ids.contains(next)
                    || model.lexical.insert(next.into(), count).is_some()
                {
                    return Err(invalid());
                }
            } else {
                let known = |value: &str| {
                    if metadata.kind == "character_bigram" {
                        value.chars().count() == 1
                            && !value.chars().any(|c| c.is_whitespace() || c.is_control())
                    } else {
                        ids.contains(value)
                    }
                };
                if (previous != BOS && !known(previous))
                    || (next != EOS && !known(next))
                    || model
                        .pairs
                        .insert((previous.into(), next.into()), count)
                        .is_some()
                {
                    return Err(invalid());
                }
                model.vocabulary.insert(next.into());
                if previous != BOS {
                    model.vocabulary.insert(previous.into());
                }
                *model.totals.entry(previous.into()).or_default() += count;
                *incoming.entry(next.into()).or_default() += count;
            }
        }
        if rows == 0 {
            return Err(invalid());
        }
        if matches!(metadata.kind.as_str(), "bigram" | "character_bigram") {
            let bos = model.totals.get(BOS).copied().unwrap_or(0);
            if bos == 0
                || bos > 100_000
                || incoming.get(EOS).copied().unwrap_or(0) != bos
                || model.totals.values().sum::<u64>() > 1_100_000
                || model
                    .vocabulary
                    .iter()
                    .filter(|id| id.as_str() != EOS && id.as_str() != UNK)
                    .any(|id| {
                        model.totals.get(id).copied().unwrap_or(0)
                            != incoming.get(id).copied().unwrap_or(0)
                    })
            {
                return Err(invalid());
            }
            let from_bos = reachable(&model.pairs, BOS, false);
            let to_eos = reachable(&model.pairs, EOS, true);
            if model.pairs.keys().any(|(previous, next)| {
                !from_bos.contains(previous.as_str()) || !to_eos.contains(next.as_str())
            }) {
                return Err(invalid());
            }
        }
        if metadata.kind == "character_bigram" {
            model.character = Some(crate::character::CharacterModel::from_counts(
                &model.vocabulary,
                &model.pairs,
                &model.totals,
                lexicon,
            ));
            model.pairs.clear();
            model.totals.clear();
            model.vocabulary = BTreeSet::from([EOS.into(), UNK.into()]);
        }
        Ok(Self { metadata, model })
    }

    /// 离线调用，元数据和序列只在边界校验，输入查询不调用训练器。
    pub fn train(
        metadata_header: &str,
        lexicon: &Lexicon,
        expected_lexicon: &str,
        sequences: &[Vec<String>],
    ) -> Result<Self, SearchError> {
        if metadata_header.lines().count() != 10
            || !metadata_header.ends_with("previous\tnext\tcount\n")
        {
            return Err(SearchError::new(
                ErrorKind::InvalidModel,
                "训练元数据不能携带计数",
            ));
        }
        let characters = metadata_header
            .lines()
            .any(|line| line == "kind\tcharacter_bigram");
        let trained = BigramModel::train_tokens(sequences, characters)?;
        let mut payload = metadata_header.to_owned();
        for ((previous, next), count) in &trained.pairs {
            payload.push_str(&format!("{previous}\t{next}\t{count}\n"));
        }
        let snapshot = Self::from_tsv(&payload, lexicon, expected_lexicon)?;
        if !matches!(
            snapshot.metadata.kind.as_str(),
            "bigram" | "character_bigram"
        ) {
            return Err(SearchError::new(
                ErrorKind::InvalidModel,
                "训练入口只接受独立序列bigram",
            ));
        }
        Ok(snapshot)
    }

    pub fn to_tsv(&self) -> String {
        let m = &self.metadata;
        let values = [
            &m.lexicon_sha256,
            &m.source_sha256,
            &m.source_id,
            &m.license_id,
            &m.tokenization,
            &m.usage,
            &m.kind,
            &m.training_split,
        ];
        let mut output = String::from("# cloudleaf-language-v1\n");
        for (key, value) in META_KEYS.iter().zip(values) {
            output.push_str(&format!("{key}\t{value}\n"));
        }
        output.push_str("previous\tnext\tcount\n");
        if m.kind == "lexical" {
            for (id, count) in &self.model.lexical {
                output.push_str(&format!("<lexical>\t{id}\t{count}\n"));
            }
        } else if let Some(model) = &self.model.character {
            model.write_tsv(&mut output);
        } else {
            for ((previous, next), count) in &self.model.pairs {
                output.push_str(&format!("{previous}\t{next}\t{count}\n"));
            }
        }
        output
    }
}

fn reachable<'a>(
    pairs: &'a BTreeMap<(String, String), u64>,
    start: &'a str,
    reverse: bool,
) -> BTreeSet<&'a str> {
    let mut outgoing = BTreeMap::<&str, Vec<&str>>::new();
    for (previous, next) in pairs.keys() {
        let (from, to) = if reverse {
            (next.as_str(), previous.as_str())
        } else {
            (previous.as_str(), next.as_str())
        };
        outgoing.entry(from).or_default().push(to);
    }
    let mut visited = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(node) = pending.pop() {
        if visited.insert(node) {
            pending.extend(outgoing.get(node).into_iter().flatten().copied());
        }
    }
    visited
}

/// 以1024为单位的整数二进制对数；避免浮点平台舍入进入候选平局。
pub(crate) fn log2_fixed(value: u64) -> i64 {
    let exponent = 63 - value.leading_zeros();
    let mut normalized = (u128::from(value) << 32) >> exponent;
    let mut fraction = 0_i64;
    for bit in (0..10).rev() {
        normalized = (normalized * normalized) >> 32;
        if normalized >= 1_u128 << 33 {
            normalized >>= 1;
            fraction |= 1 << bit;
        }
    }
    i64::from(exponent) * 1024 + fraction
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_log_is_monotone_and_exact_at_powers() {
        assert_eq!(log2_fixed(1), 0);
        assert_eq!(log2_fixed(8), 3072);
        for n in 1..1000 {
            assert!(log2_fixed(n) <= log2_fixed(n + 1));
        }
    }
}
