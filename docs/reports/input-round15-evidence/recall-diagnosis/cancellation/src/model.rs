use crate::{ErrorKind, SearchError};
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
}

impl BigramModel {
    pub fn train(sequences: &[Vec<String>]) -> Result<Self, SearchError> {
        if sequences.len() > 100_000
            || sequences.iter().map(Vec::len).sum::<usize>() > 1_000_000
            || sequences.iter().flatten().any(|s| {
                s.is_empty()
                    || s.len() > 128
                    || s.starts_with('<')
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
}

/// 以1024为单位的整数二进制对数；避免浮点平台舍入进入候选平局。
fn log2_fixed(value: u64) -> i64 {
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
