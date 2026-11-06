use crate::Lexicon;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const BOS: u32 = 0x110000;
const EOS: u32 = 0x110001;
const UNK: u32 = 0x110002;

#[derive(Clone, Debug)]
struct Row {
    key: u64,
    count: u64,
    units: i64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EntryScore {
    pub first: u32,
    pub last: u32,
    pub internal: i64,
    pub characters: usize,
    pub unknown: usize,
}

/// 数字键与词条内部转移只在模型载入时生成；输入查询不分配临时token或计算对数。
#[derive(Clone, Debug)]
pub(crate) struct CharacterModel {
    pub(crate) lexicon_identity: std::sync::Arc<()>,
    rows: Vec<Row>,
    vocabulary: Vec<u32>,
    denominators: Vec<i64>,
    starts: Vec<usize>,
    entries: Vec<EntryScore>,
}

fn key(previous: u32, next: u32) -> u64 {
    (u64::from(previous) << 32) | u64::from(next)
}

fn token(text: &str) -> u32 {
    match text {
        "<bos>" => BOS,
        "<eos>" => EOS,
        "<unk>" => UNK,
        _ => text.chars().next().unwrap() as u32,
    }
}

fn text(token: u32) -> String {
    match token {
        BOS => "<bos>".into(),
        EOS => "<eos>".into(),
        UNK => "<unk>".into(),
        _ => char::from_u32(token).unwrap().to_string(),
    }
}

impl CharacterModel {
    pub(crate) fn from_counts(
        vocabulary: &BTreeSet<String>,
        pairs: &BTreeMap<(String, String), u64>,
        totals: &BTreeMap<String, u64>,
        lexicon: &Lexicon,
    ) -> Self {
        let mut vocabulary: Vec<_> = vocabulary.iter().map(|value| token(value)).collect();
        vocabulary.sort_unstable();
        let size = vocabulary.len() as u64;
        let mapped = |value: &str| {
            if value == "<bos>" {
                vocabulary.len()
            } else {
                vocabulary.binary_search(&token(value)).unwrap()
            }
        };
        let denominators: Vec<_> = vocabulary
            .iter()
            .copied()
            .chain(std::iter::once(BOS))
            .map(|value| {
                crate::model::log2_fixed(totals.get(&text(value)).copied().unwrap_or(0) + size)
            })
            .collect();
        let mut rows: Vec<_> = pairs
            .iter()
            .map(|((previous, next), count)| {
                let previous = mapped(previous);
                let denominator = denominators[previous];
                Row {
                    key: key(previous as u32, mapped(next) as u32),
                    count: *count,
                    units: crate::model::log2_fixed(count + 1) - denominator,
                }
            })
            .collect();
        rows.sort_unstable_by_key(|row| row.key);
        let starts = (0..=vocabulary.len() + 1)
            .map(|index| rows.partition_point(|row| (row.key >> 32) < index as u64))
            .collect();
        let mut model = Self {
            lexicon_identity: lexicon.identity.clone(),
            rows,
            vocabulary,
            denominators,
            starts,
            entries: Vec::with_capacity(lexicon.entries().len()),
        };
        for entry in lexicon.entries() {
            let mut characters = entry.text.chars();
            let first = model.known(characters.next().unwrap());
            let mut score = EntryScore {
                first,
                last: first,
                internal: 0,
                characters: 1,
                unknown: usize::from(first as usize == model.vocabulary.len() - 1),
            };
            for character in characters {
                let next = model.known(character);
                score.internal += model.transition(score.last, next);
                score.last = next;
                score.characters += 1;
                score.unknown += usize::from(next as usize == model.vocabulary.len() - 1);
            }
            model.entries.push(score);
        }
        model
    }

    fn known(&self, value: char) -> u32 {
        let value = value as u32;
        self.vocabulary
            .binary_search(&value)
            .unwrap_or(self.vocabulary.len() - 1) as u32
    }

    pub(crate) fn entry(&self, index: usize) -> EntryScore {
        self.entries[index]
    }

    pub(crate) fn transition(&self, previous: u32, next: u32) -> i64 {
        let previous = if previous == BOS {
            self.vocabulary.len()
        } else {
            previous as usize
        };
        let rows = &self.rows[self.starts[previous]..self.starts[previous + 1]];
        if let Ok(index) = rows.binary_search_by_key(&next, |row| row.key as u32) {
            rows[index].units
        } else {
            -self.denominators[previous]
        }
    }

    pub(crate) fn finish(&self, previous: u32) -> i64 {
        self.transition(previous, (self.vocabulary.len() - 2) as u32)
    }

    pub(crate) fn write_tsv(&self, output: &mut String) {
        for row in &self.rows {
            let previous = (row.key >> 32) as usize;
            let previous = if previous == self.vocabulary.len() {
                BOS
            } else {
                self.vocabulary[previous]
            };
            output.push_str(&format!(
                "{}\t{}\t{}\n",
                text(previous),
                text(self.vocabulary[row.key as u32 as usize]),
                row.count
            ));
        }
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.rows.capacity() * std::mem::size_of::<Row>()
            + self.vocabulary.capacity() * std::mem::size_of::<u32>()
            + self.denominators.capacity() * std::mem::size_of::<i64>()
            + self.starts.capacity() * std::mem::size_of::<usize>()
            + self.entries.capacity() * std::mem::size_of::<EntryScore>()
    }
}
