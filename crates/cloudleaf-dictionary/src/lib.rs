//! 本地原型词典在载入时校验数据并建立索引，按键查询不访问文件。

use cloudleaf_pinyin::{MAX_INPUT_LEN, PinyinParse, is_syllable};
use cloudleaf_types::{CoreError, ErrorCode};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// 项目独立编写的微型测试词典，不代表正式输入法词库。
pub const PROTOTYPE_TSV: &str = include_str!("../../../data/dictionaries/prototype.tsv");
pub const MAX_DICTIONARY_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DICTIONARY_ENTRIES: usize = 100_000;
pub const MAX_ENTRY_SYLLABLES: usize = 16;

const VERSION: &str = "# cloudleaf-dictionary-v1";
const HEADER: &str = "id\tpinyin\ttext\tfrequency\tannotation\tsource";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryEntry {
    pub id: String,
    pub pinyin: Vec<String>,
    pub text: String,
    pub frequency: u32,
    pub annotation: Option<String>,
    pub source: String,
}

/// 索引中的每个桶已按排序规则保留同文本的最佳词条。
#[derive(Debug, Clone, Default)]
pub struct Dictionary {
    entries: Vec<DictionaryEntry>,
    exact: BTreeMap<String, Vec<usize>>,
    prefix: BTreeMap<String, Vec<usize>>,
}

impl Dictionary {
    pub fn empty() -> Self {
        Self::default()
    }

    /// 严格校验整个文件；不跳过坏行，也不回传包含输入内容的错误。
    pub fn from_tsv(data: &str) -> Result<Self, CoreError> {
        if data.len() > MAX_DICTIONARY_BYTES {
            return Err(CoreError::new(
                ErrorCode::InvalidDictionary,
                "词典文件超过大小限制",
            ));
        }
        let mut lines = data.lines();
        match lines.next() {
            Some(VERSION) => {}
            Some(value) if value.starts_with("# cloudleaf-dictionary-") => {
                return Err(
                    CoreError::new(ErrorCode::UnsupportedVersion, "不支持此词典版本").at_line(1),
                );
            }
            _ => return Err(invalid("缺少有效的词典版本标识", 1)),
        }
        if lines.next() != Some(HEADER) {
            return Err(invalid("词典表头不符合版本规范", 2));
        }

        let mut entries = Vec::new();
        let mut ids = BTreeSet::new();
        for (offset, line) in lines.enumerate() {
            let line_number = offset + 3;
            if entries.len() >= MAX_DICTIONARY_ENTRIES {
                return Err(invalid("词典条目超过数量限制", line_number));
            }
            let fields: Vec<&str> = line.splitn(7, '\t').collect();
            if fields.len() != 6 {
                return Err(invalid("词典行必须包含六个字段", line_number));
            }
            let [id, pinyin, text, frequency, annotation, source] =
                <[&str; 6]>::try_from(fields).expect("已检查词典字段数量");
            if !valid_identifier(id) || !ids.insert(id.to_owned()) {
                return Err(invalid("词条标识无效或重复", line_number));
            }
            if !valid_identifier(source) {
                return Err(invalid("词条来源标识无效", line_number));
            }
            if !valid_text(text, 64, false) || !valid_text(annotation, 256, true) {
                return Err(invalid("词条文本或注释无效或过长", line_number));
            }
            if pinyin.len() > MAX_INPUT_LEN {
                return Err(invalid("词条拼音超过长度限制", line_number));
            }
            let syllables: Vec<&str> = pinyin.split(' ').collect();
            if syllables.len() > MAX_ENTRY_SYLLABLES
                || syllables.iter().any(|syllable| !is_syllable(syllable))
            {
                return Err(invalid(
                    "词条拼音必须为小写标准音节并以单空格分隔",
                    line_number,
                ));
            }
            if frequency.is_empty() || !frequency.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(invalid("词频必须为非负十进制整数", line_number));
            }
            let frequency = frequency
                .parse::<u32>()
                .map_err(|_| invalid("词频超出有效范围", line_number))?;
            entries.push(DictionaryEntry {
                id: id.to_owned(),
                pinyin: syllables.into_iter().map(str::to_owned).collect(),
                text: text.to_owned(),
                frequency,
                annotation: (!annotation.is_empty()).then(|| annotation.to_owned()),
                source: source.to_owned(),
            });
        }

        let mut dictionary = Self {
            entries,
            ..Self::empty()
        };
        dictionary.build_indexes();
        Ok(dictionary)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 返回同音节数的匹配，每个提交文本仅保留排序最佳的词条。
    pub fn query(&self, parse: &PinyinParse) -> Vec<&DictionaryEntry> {
        self.query_limited(parse, usize::MAX)
    }

    /// 按键路径使用此接口；读取已预排序的桶，不扫描或重排整份词典。
    pub fn query_limited(&self, parse: &PinyinParse, limit: usize) -> Vec<&DictionaryEntry> {
        if limit == 0
            || parse.syllables.is_empty()
            || parse.syllables.len() > MAX_ENTRY_SYLLABLES
            || parse
                .syllables
                .iter()
                .any(|syllable| syllable.len() > 6 || !syllable.is_ascii())
            || parse.syllables.last().is_some_and(String::is_empty)
        {
            return Vec::new();
        }
        let key = parse.syllables.join(" ");
        let index = if parse.incomplete {
            &self.prefix
        } else {
            &self.exact
        };
        index
            .get(&key)
            .into_iter()
            .flatten()
            .take(limit)
            .map(|position| &self.entries[*position])
            .collect()
    }

    fn build_indexes(&mut self) {
        for (position, entry) in self.entries.iter().enumerate() {
            let key = entry.pinyin.join(" ");
            self.exact.entry(key).or_default().push(position);
            // 拼音已校验为 ASCII；只索引最后音节的非空前缀。
            let last = entry.pinyin.last().expect("已检查拼音音节");
            let stem = &entry.pinyin[..entry.pinyin.len() - 1];
            let stem = if stem.is_empty() {
                String::new()
            } else {
                format!("{} ", stem.join(" "))
            };
            for end in 1..=last.len() {
                self.prefix
                    .entry(format!("{}{}", stem, &last[..end]))
                    .or_default()
                    .push(position);
            }
        }
        prepare_buckets(&self.entries, &mut self.exact);
        prepare_buckets(&self.entries, &mut self.prefix);
    }
}

fn prepare_buckets(entries: &[DictionaryEntry], index: &mut BTreeMap<String, Vec<usize>>) {
    for bucket in index.values_mut() {
        bucket.sort_unstable_by(|left, right| compare_entries(&entries[*left], &entries[*right]));
        let mut seen = BTreeSet::new();
        bucket.retain(|position| seen.insert(entries[*position].text.as_str()));
    }
}

/// 同匹配类型的排序：词频降序、提交文本字数降序、稳定 ID 升序。
pub fn compare_entries(left: &DictionaryEntry, right: &DictionaryEntry) -> Ordering {
    right
        .frequency
        .cmp(&left.frequency)
        .then_with(|| right.text.chars().count().cmp(&left.text.chars().count()))
        .then_with(|| left.id.cmp(&right.id))
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
}

fn valid_text(value: &str, max_characters: usize, allow_empty: bool) -> bool {
    (allow_empty || !value.is_empty())
        && value.chars().count() <= max_characters
        && !value.chars().any(char::is_control)
        && value.trim() == value
}

fn invalid(message: &'static str, line: usize) -> CoreError {
    CoreError::new(ErrorCode::InvalidDictionary, message).at_line(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dictionary(rows: &str) -> Result<Dictionary, CoreError> {
        Dictionary::from_tsv(&format!("{VERSION}\n{HEADER}\n{rows}"))
    }

    fn parsed(syllables: &[&str], incomplete: bool) -> PinyinParse {
        PinyinParse {
            syllables: syllables.iter().map(|value| (*value).to_owned()).collect(),
            incomplete,
        }
    }

    #[test]
    fn prototype_is_valid_and_queries_use_the_whole_pronunciation() {
        let dictionary = Dictionary::from_tsv(PROTOTYPE_TSV).unwrap();
        assert_eq!(dictionary.len(), 64);
        let ni = dictionary.query(&parsed(&["ni"], false));
        assert_eq!(ni.len(), 12);
        assert_eq!(ni[0].text, "你");
        assert!(ni.iter().all(|entry| entry.pinyin.len() == 1));
        assert!(dictionary.query(&parsed(&["ni", "ha"], false)).is_empty());
        let nihao = dictionary.query(&parsed(&["ni", "hao"], false));
        assert_eq!(nihao.len(), 1);
        assert_eq!(nihao[0].id, "nihao-001");
        assert_eq!(nihao[0].annotation.as_deref(), Some("hello"));
    }

    #[test]
    fn only_the_last_syllable_may_be_a_prefix() {
        let dictionary = Dictionary::from_tsv(PROTOTYPE_TSV).unwrap();
        let nihao = dictionary.query(&parsed(&["ni", "h"], true));
        assert_eq!(nihao.len(), 1);
        assert_eq!(nihao[0].text, "你好");
        assert!(dictionary.query(&parsed(&["n", "hao"], true)).is_empty());
        assert!(dictionary.query(&parsed(&["xi", ""], true)).is_empty());
        assert!(dictionary.query(&parsed(&[], true)).is_empty());
        assert!(dictionary.query(&parsed(&["世"], true)).is_empty());
    }

    #[test]
    fn limited_queries_preserve_best_unique_texts() {
        let dictionary =
            dictionary("z\tni\t你\t10\t\ttest\na\tni\t你\t10\t\ttest\nb\tni\t尼\t5\t\ttest\n")
                .unwrap();
        let entries = dictionary.query_limited(&parsed(&["ni"], false), 2);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert!(
            dictionary
                .query_limited(&parsed(&["ni"], false), 0)
                .is_empty()
        );
    }

    #[test]
    fn missing_or_unknown_versions_and_invalid_header_are_rejected() {
        assert!(Dictionary::from_tsv("").is_err());
        assert!(Dictionary::from_tsv("# cloudleaf-dictionary-v2\n").is_err());
        assert!(Dictionary::from_tsv(&format!("{VERSION}\nwrong\n")).is_err());
    }

    #[test]
    fn corrupt_rows_are_never_silently_skipped() {
        let invalid_rows = [
            "id\tni\t你\t1\t\ttest\nid\tni\t尼\t1\t\ttest\n",
            "id\tnothing\t你\t1\t\ttest\n",
            "id\tNI\t你\t1\t\ttest\n",
            "id\tni  hao\t你好\t1\t\ttest\n",
            "id\tni\t\t1\t\ttest\n",
            "id\tni\t你\t4294967296\t\ttest\n",
            "id\tni\t你\t-1\t\ttest\n",
            "id\tni\t你\t+1\t\ttest\n",
            "id\tni\t你\t1\t\t\n",
            "id\tni\t你\t1\t\ttest\textra\n",
            "id\tni\t你\t1\tbad\u{001b}\ttest\n",
            "id\tni\t 你\t1\t\ttest\n",
            "\n",
        ];
        for rows in invalid_rows {
            assert!(dictionary(rows).is_err(), "应拒绝无效词典行");
        }
    }

    #[test]
    fn size_and_field_limits_are_enforced() {
        assert!(Dictionary::from_tsv(&" ".repeat(MAX_DICTIONARY_BYTES + 1)).is_err());
        let long_text = "你".repeat(65);
        assert!(dictionary(&format!("id\tni\t{long_text}\t1\t\ttest\n")).is_err());
        let long_pinyin = vec!["ni"; MAX_ENTRY_SYLLABLES + 1].join(" ");
        assert!(dictionary(&format!("id\t{long_pinyin}\t你\t1\t\ttest\n")).is_err());
    }

    #[test]
    fn empty_dictionary_and_crlf_files_are_supported() {
        assert!(Dictionary::empty().is_empty());
        assert!(
            Dictionary::empty()
                .query(&parsed(&["ni"], false))
                .is_empty()
        );
        assert!(dictionary("").unwrap().is_empty());
        let data = format!("{VERSION}\r\n{HEADER}\r\nid\tni\t你\t0\t\ttest\r\n");
        assert_eq!(Dictionary::from_tsv(&data).unwrap().len(), 1);
    }
}
