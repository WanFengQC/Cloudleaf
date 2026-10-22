use crate::{ErrorKind, SearchError};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrequencyKind {
    Unknown,
    Manual,
    Count,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub id: String,
    pub pinyin: String,
    pub text: String,
    pub frequency: Option<u32>,
    pub frequency_kind: FrequencyKind,
    pub annotation: String,
    pub source: String,
    pub source_record_id: String,
    pub license_id: String,
    pub(crate) letters: Vec<u8>,
    pub(crate) boundaries: BTreeSet<usize>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TrieNode {
    pub next: BTreeMap<u8, usize>,
    pub exact: Vec<usize>,
    pub prefix: Vec<usize>,
}

/// 只读词条与读音索引，装载后可在多个查询之间复用。
#[derive(Clone, Debug)]
pub struct Lexicon {
    pub(crate) entries: Vec<Entry>,
    pub(crate) trie: Vec<TrieNode>,
}

/// 载入预算由宿主设定；v2默认容量不继承旧原型十万条上限。
#[derive(Clone, Debug)]
pub struct LoadLimits {
    pub max_bytes: usize,
    pub max_entries: usize,
    pub max_trie_nodes: usize,
}

impl Default for LoadLimits {
    fn default() -> Self {
        Self {
            max_bytes: 128 * 1024 * 1024,
            max_entries: 1_000_000,
            max_trie_nodes: 4_000_000,
        }
    }
}

fn clean(value: &str, max: usize, empty: bool) -> bool {
    (empty || !value.is_empty())
        && value.chars().count() <= max
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}

impl Lexicon {
    pub fn from_tsv(text: &str) -> Result<Self, SearchError> {
        Self::from_tsv_with_limits(text, &LoadLimits::default())
    }

    pub fn from_tsv_with_limits(text: &str, limits: &LoadLimits) -> Result<Self, SearchError> {
        let invalid = || SearchError::new(ErrorKind::InvalidLexicon, "词典格式或字段无效");
        if !(1..=512 * 1024 * 1024).contains(&limits.max_bytes)
            || !(1..=5_000_000).contains(&limits.max_entries)
            || !(1..=20_000_000).contains(&limits.max_trie_nodes)
        {
            return Err(SearchError::new(
                ErrorKind::InvalidConfig,
                "词典装载预算无效",
            ));
        }
        if text.len() > limits.max_bytes || text.starts_with('\u{feff}') {
            return Err(invalid());
        }
        let mut lines = text.lines();
        let v2 = match lines.next() {
            Some("# cloudleaf-lexicon-v2") => true,
            Some("# cloudleaf-dictionary-v1") => false,
            _ => return Err(invalid()),
        };
        if v2 && text.contains('\r') {
            return Err(invalid());
        }
        let expected = if v2 {
            "id\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id"
        } else {
            "id\tpinyin\ttext\tfrequency\tannotation\tsource"
        };
        if lines.next() != Some(expected) {
            return Err(invalid());
        }
        if !v2 && text.len() > 16 * 1024 * 1024 {
            return Err(invalid());
        }
        let mut entries = Vec::new();
        let mut ids = BTreeSet::new();
        for (index, line) in lines.enumerate() {
            let result = (|| {
                let fields: Vec<_> = line.split('\t').collect();
                if fields.len() != if v2 { 9 } else { 6 }
                    || entries.len() >= limits.max_entries
                    || (!v2 && entries.len() >= 100_000)
                {
                    return Err(invalid());
                }
                let id = fields[0];
                let pinyin = fields[1];
                let words: Vec<_> = pinyin.split(' ').collect();
                if !identifier(id)
                    || !ids.insert(id.to_owned())
                    || !clean(fields[2], 64, false)
                    || words.len() > 16
                    || pinyin.len() > 80
                    || words.iter().any(|w| {
                        !w.bytes().all(|b| b.is_ascii_lowercase())
                            || !cloudleaf_pinyin::is_syllable(w)
                    })
                {
                    return Err(invalid());
                }
                let kind = if v2 {
                    match fields[4] {
                        "unknown" => FrequencyKind::Unknown,
                        "manual" => FrequencyKind::Manual,
                        "count" => FrequencyKind::Count,
                        _ => return Err(invalid()),
                    }
                } else {
                    FrequencyKind::Manual
                };
                let frequency = if kind == FrequencyKind::Unknown {
                    if !fields[3].is_empty() {
                        return Err(invalid());
                    }
                    None
                } else {
                    if fields[3].is_empty() || !fields[3].bytes().all(|b| b.is_ascii_digit()) {
                        return Err(invalid());
                    }
                    Some(fields[3].parse().map_err(|_| invalid())?)
                };
                let (annotation, source, source_record_id, license_id) = if v2 {
                    (fields[5], fields[6], fields[7], fields[8])
                } else {
                    (fields[4], fields[5], id, "MIT")
                };
                if !clean(annotation, 256, true)
                    || !identifier(source)
                    || !identifier(source_record_id)
                    || !identifier(license_id)
                {
                    return Err(invalid());
                }
                let mut letters = Vec::new();
                let mut boundaries = BTreeSet::new();
                for word in words {
                    if !letters.is_empty() {
                        boundaries.insert(letters.len());
                    }
                    letters.extend_from_slice(word.as_bytes());
                }
                Ok(Entry {
                    id: id.into(),
                    pinyin: pinyin.into(),
                    text: fields[2].into(),
                    frequency,
                    frequency_kind: kind,
                    annotation: annotation.into(),
                    source: source.into(),
                    source_record_id: source_record_id.into(),
                    license_id: license_id.into(),
                    letters,
                    boundaries,
                })
            })();
            entries.push(result.map_err(|e: SearchError| e.at_line(index + 3))?);
        }
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        let mut trie = vec![TrieNode::default()];
        for (entry_index, entry) in entries.iter().enumerate() {
            let last_start = entry.boundaries.last().copied().unwrap_or(0);
            let mut node = 0;
            for (offset, byte) in entry.letters.iter().enumerate() {
                node = if let Some(next) = trie[node].next.get(byte) {
                    *next
                } else {
                    let next = trie.len();
                    if next >= limits.max_trie_nodes {
                        return Err(SearchError::new(
                            ErrorKind::InvalidLexicon,
                            "词典读音索引超出装载预算",
                        ));
                    }
                    trie.push(TrieNode::default());
                    trie[node].next.insert(*byte, next);
                    next
                };
                let consumed = offset + 1;
                if consumed > last_start && consumed < entry.letters.len() {
                    trie[node].prefix.push(entry_index);
                }
            }
            trie[node].exact.push(entry_index);
        }
        Ok(Self { entries, trie })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn trie_nodes(&self) -> usize {
        self.trie.len()
    }
}
