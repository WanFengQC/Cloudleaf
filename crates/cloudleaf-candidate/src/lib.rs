//! 无网络的确定性候选生成；注释永远不参与得分。

use cloudleaf_dictionary::{Dictionary, DictionaryEntry, compare_entries};
use cloudleaf_pinyin::{MAX_PARSES, ParseResult, ParseStatus};
use cloudleaf_types::{Candidate, MatchKind};
use std::cmp::Ordering;
use std::collections::BTreeSet;

/// 即使调用者传入更大 limit，也只向输入界面返回此数量的候选。
pub const MAX_CANDIDATES: usize = 128;

/// 完整匹配优先，再按词频、文本字数和稳定 ID 排序并按文本去重。
pub fn generate(parsed: &ParseResult, dictionary: &Dictionary, limit: usize) -> Vec<Candidate> {
    let limit = limit.min(MAX_CANDIDATES);
    if limit == 0 || matches!(parsed.status, ParseStatus::Empty | ParseStatus::Invalid) {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for parse in parsed.parses.iter().take(MAX_PARSES) {
        let match_kind = if parse.incomplete {
            MatchKind::Prefix
        } else {
            MatchKind::Exact
        };
        // 每桶内文本唯一，所以全局前 limit 个文本一定包含在各桶前 limit 项中。
        for entry in dictionary.query_limited(parse, limit) {
            matches.push(MatchedEntry { entry, match_kind });
        }
    }
    matches.sort_unstable_by(compare_matches);

    let mut seen = BTreeSet::new();
    matches
        .into_iter()
        .filter(|matched| seen.insert(matched.entry.text.as_str()))
        .take(limit)
        .map(|matched| Candidate {
            id: matched.entry.id.clone(),
            text: matched.entry.text.clone(),
            annotation: matched.entry.annotation.clone(),
            source: matched.entry.source.clone(),
            frequency: matched.entry.frequency,
            match_kind: matched.match_kind,
        })
        .collect()
}

struct MatchedEntry<'a> {
    entry: &'a DictionaryEntry,
    match_kind: MatchKind,
}

fn compare_matches(left: &MatchedEntry<'_>, right: &MatchedEntry<'_>) -> Ordering {
    rank(left.match_kind)
        .cmp(&rank(right.match_kind))
        .then_with(|| compare_entries(left.entry, right.entry))
}

fn rank(kind: MatchKind) -> u8 {
    match kind {
        MatchKind::Exact => 0,
        MatchKind::Prefix => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudleaf_dictionary::PROTOTYPE_TSV;
    use cloudleaf_pinyin::{PinyinParse, parse};

    fn dictionary(rows: &str) -> Dictionary {
        Dictionary::from_tsv(&format!(
            "# cloudleaf-dictionary-v1\nid\tpinyin\ttext\tfrequency\tannotation\tsource\n{rows}"
        ))
        .unwrap()
    }

    fn texts(candidates: &[Candidate]) -> Vec<&str> {
        candidates
            .iter()
            .map(|candidate| candidate.text.as_str())
            .collect()
    }

    #[test]
    fn prototype_generates_words_and_keeps_annotations_separate() {
        let dictionary = Dictionary::from_tsv(PROTOTYPE_TSV).unwrap();
        for (input, expected) in [
            ("nihao", "你好"),
            ("zhongguo", "中国"),
            ("zhongwen", "中文"),
            ("shijie", "世界"),
            ("cankao", "参考"),
            ("lv", "绿"),
            ("nv", "女"),
        ] {
            let candidates = generate(&parse(input).unwrap(), &dictionary, 9);
            assert_eq!(candidates[0].text, expected);
            assert!(candidates[0].annotation.is_some());
            assert_eq!(candidates[0].source, "cloudleaf:prototype-v1");
        }
    }

    #[test]
    fn exact_matches_precede_higher_frequency_prefix_matches() {
        let dictionary = dictionary("a\tni\t你\t1\t\ttest\nb\tning\t宁\t999\t\ttest\n");
        let candidates = generate(&parse("ni").unwrap(), &dictionary, 9);
        assert_eq!(texts(&candidates), ["你", "宁"]);
        assert_eq!(candidates[0].match_kind, MatchKind::Exact);
        assert_eq!(candidates[1].match_kind, MatchKind::Prefix);
    }

    #[test]
    fn ordering_is_stable_when_rows_are_reordered() {
        let a = "c\tni\t泥\t10\t\ttest\na\tni\t你\t10\t\ttest\nb\tni\t尼\t10\t\ttest\n";
        let b = "b\tni\t尼\t10\t\ttest\nc\tni\t泥\t10\t\ttest\na\tni\t你\t10\t\ttest\n";
        let parsed = parse("ni").unwrap();
        let first = generate(&parsed, &dictionary(a), 9);
        assert_eq!(texts(&first), ["你", "尼", "泥"]);
        assert_eq!(first, generate(&parsed, &dictionary(b), 9));
    }

    #[test]
    fn tied_frequency_prefers_longer_text_then_id() {
        let dictionary = dictionary("a\tni\t你\t10\t\ttest\nz\tni\t你好\t10\t\ttest\n");
        assert_eq!(
            texts(&generate(&parse("ni").unwrap(), &dictionary, 9)),
            ["你好", "你"]
        );
    }

    #[test]
    fn duplicate_text_across_pronunciations_uses_the_best_match() {
        let dictionary = dictionary(
            "a\txian\t先\t10\t\ttest\nb\txi an\t先\t20\t\ttest\nc\txi an\t西安\t5\t\ttest\n",
        );
        let parsed = ParseResult {
            normalized: "xian".to_owned(),
            status: ParseStatus::Complete,
            parses: vec![
                PinyinParse {
                    syllables: vec!["xian".to_owned()],
                    incomplete: false,
                },
                PinyinParse {
                    syllables: vec!["xi".to_owned(), "an".to_owned()],
                    incomplete: false,
                },
            ],
        };
        let candidates = generate(&parsed, &dictionary, 9);
        assert_eq!(texts(&candidates), ["先", "西安"]);
        assert_eq!(candidates[0].id, "b");
    }

    #[test]
    fn annotations_do_not_change_ranking() {
        let plain = dictionary("a\tni\t你\t10\t\ttest\nb\tni\t尼\t5\t\ttest\n");
        let annotated =
            dictionary("a\tni\t你\t10\tzero\ttest\nb\tni\t尼\t5\tbest translation\ttest\n");
        let parsed = parse("ni").unwrap();
        assert_eq!(
            texts(&generate(&parsed, &plain, 9)),
            texts(&generate(&parsed, &annotated, 9))
        );
    }

    #[test]
    fn unknown_invalid_empty_inputs_and_waiting_boundaries_have_no_candidates() {
        let dictionary = Dictionary::from_tsv(PROTOTYPE_TSV).unwrap();
        for input in ["", "biao", "🙂", "xi'"] {
            assert!(generate(&parse(input).unwrap(), &dictionary, 9).is_empty());
        }
        assert!(generate(&parse("ni").unwrap(), &Dictionary::empty(), 9).is_empty());
    }

    #[test]
    fn limits_bound_output_and_keep_best_unique_candidates() {
        let dictionary = Dictionary::from_tsv(PROTOTYPE_TSV).unwrap();
        let parsed = parse("ni").unwrap();
        assert!(generate(&parsed, &dictionary, 0).is_empty());
        let all = generate(&parsed, &dictionary, usize::MAX);
        let limited = generate(&parsed, &dictionary, 3);
        assert_eq!(limited, all[..3]);
        assert!(all.len() <= MAX_CANDIDATES);
    }

    #[test]
    fn large_matching_bucket_cannot_exceed_the_public_candidate_limit() {
        let mut rows = String::new();
        for number in 0..200 {
            rows.push_str(&format!(
                "entry-{number:03}\tni\t词{number:03}\t{number}\t\ttest\n"
            ));
        }
        let dictionary = dictionary(&rows);
        let candidates = generate(&parse("ni").unwrap(), &dictionary, usize::MAX);
        assert_eq!(candidates.len(), MAX_CANDIDATES);
        assert_eq!(candidates[0].id, "entry-199");
        assert_eq!(candidates[MAX_CANDIDATES - 1].id, "entry-072");
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.match_kind == MatchKind::Exact)
        );
    }

    #[test]
    fn bucket_limits_preserve_global_top_k_despite_cross_bucket_duplicates() {
        let dictionary = dictionary(
            "a\txian\t先\t30\t\ttest\nb\txian\t西安\t20\t\ttest\nc\txi an\t西安\t40\t\ttest\nd\txi an\t现\t10\t\ttest\n",
        );
        let parsed = parse("xian").unwrap();
        let all = generate(&parsed, &dictionary, MAX_CANDIDATES);
        assert_eq!(texts(&all), ["西安", "先", "现"]);
        for limit in 1..=3 {
            assert_eq!(generate(&parsed, &dictionary, limit), all[..limit]);
        }
    }
}
