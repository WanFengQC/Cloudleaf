//! 自研全拼分段；仅在最后一个音节允许前缀，不做模糊音或简拼猜测。

use std::cmp::Ordering;
use std::sync::OnceLock;

use cloudleaf_types::{CoreError, ErrorCode};

/// 输入按 UTF-8 字节限制；合法全拼本身只包含 ASCII 字母和分隔符。
pub const MAX_INPUT_LEN: usize = 64;
/// 每个动态规划位置和最终结果都限制分段数量，避免歧义导致无界回溯。
pub const MAX_PARSES: usize = 32;
const MAX_SYLLABLE_LEN: usize = 6;

/// 解析状态不代表词典是否有对应词条。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseStatus {
    Empty,
    Complete,
    Incomplete,
    Invalid,
}

/// incomplete 时最后一项为未完成的音节前缀；尾部分隔符对应空前缀。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinyinParse {
    pub syllables: Vec<String>,
    pub incomplete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult {
    pub normalized: String,
    pub status: ParseStatus,
    pub parses: Vec<PinyinParse>,
}

// 按声母独立整理的常规全拼音节，v 是 n/l 后 ü 的键盘表示。
// 未引入第三方词典、现有输入法数据或其他项目的音节表。
const SYLLABLE_GROUPS: &[&str] = &[
    "a ai an ang ao e ei en eng er o ou",
    "ba bai ban bang bao bei ben beng bi bian biao bie bin bing bo bu",
    "pa pai pan pang pao pei pen peng pi pian piao pie pin ping po pou pu",
    "ma mai man mang mao me mei men meng mi mian miao mie min ming miu mo mou mu",
    "fa fan fang fei fen feng fo fou fu",
    "da dai dan dang dao de dei den deng di dia dian diao die ding diu dong dou du duan dui dun duo",
    "ta tai tan tang tao te teng ti tian tiao tie ting tong tou tu tuan tui tun tuo",
    "na nai nan nang nao ne nei nen neng ni nian niang niao nie nin ning niu nong nou nu nuan nuo nv nve",
    "la lai lan lang lao le lei leng li lia lian liang liao lie lin ling liu lo long lou lu luan lun luo lv lve",
    "ga gai gan gang gao ge gei gen geng gong gou gu gua guai guan guang gui gun guo",
    "ka kai kan kang kao ke kei ken keng kong kou ku kua kuai kuan kuang kui kun kuo",
    "ha hai han hang hao he hei hen heng hong hou hu hua huai huan huang hui hun huo",
    "ji jia jian jiang jiao jie jin jing jiong jiu ju juan jue jun",
    "qi qia qian qiang qiao qie qin qing qiong qiu qu quan que qun",
    "xi xia xian xiang xiao xie xin xing xiong xiu xu xuan xue xun",
    "zha zhai zhan zhang zhao zhe zhei zhen zheng zhi zhong zhou zhu zhua zhuai zhuan zhuang zhui zhun zhuo",
    "cha chai chan chang chao che chen cheng chi chong chou chu chua chuai chuan chuang chui chun chuo",
    "sha shai shan shang shao she shei shen sheng shi shou shu shua shuai shuan shuang shui shun shuo",
    "ran rang rao re ren reng ri rong rou ru ruan rui run ruo",
    "za zai zan zang zao ze zei zen zeng zi zong zou zu zuan zui zun zuo",
    "ca cai can cang cao ce cen ceng ci cong cou cu cuan cui cun cuo",
    "sa sai san sang sao se sen seng si song sou su suan sui sun suo",
    "ya yan yang yao ye yi yin ying yo yong you yu yuan yue yun",
    "wa wai wan wang wei wen weng wo wu",
];

fn syllables() -> &'static [&'static str] {
    static SYLLABLES: OnceLock<Vec<&'static str>> = OnceLock::new();
    SYLLABLES.get_or_init(|| {
        let mut values: Vec<_> = SYLLABLE_GROUPS
            .iter()
            .flat_map(|group| group.split_ascii_whitespace())
            .collect();
        values.sort_unstable();
        values.dedup();
        values
    })
}

/// 检查词典使用的小写规范全拼音节；不接受声调、简拼和空白。
pub fn is_syllable(value: &str) -> bool {
    syllables().binary_search(&value).is_ok()
}

fn is_strict_prefix(value: &str) -> bool {
    if value.is_empty() || value.len() >= MAX_SYLLABLE_LEN {
        return false;
    }
    let index = syllables().partition_point(|syllable| *syllable < value);
    syllables()[index..]
        .iter()
        .take_while(|syllable| syllable.starts_with(value))
        .any(|syllable| syllable.len() > value.len())
}

fn compare_paths(left: &[&str], right: &[&str]) -> Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

fn add_path<'a>(paths: &mut Vec<Vec<&'a str>>, path: Vec<&'a str>) {
    // 每次插入都维持相同的全序，截断与输入遍历顺序无关。
    match paths.binary_search_by(|existing| compare_paths(existing, &path)) {
        Ok(_) => {}
        Err(index) if index < MAX_PARSES => {
            paths.insert(index, path);
            paths.truncate(MAX_PARSES);
        }
        Err(_) => {}
    }
}

/// 将全拼分为最多 32 条路径，完整分段优先，再按音节数和字典序排序。
///
/// 字母大小写在副本中归一；非法输入完整保留在 normalized 中并返回 Invalid。
/// 超长输入返回错误，不截断原始文本。空末项仅表示显式分隔符后的待输入状态。
pub fn parse(input: &str) -> Result<ParseResult, CoreError> {
    if input.len() > MAX_INPUT_LEN {
        return Err(CoreError::new(
            ErrorCode::InputTooLong,
            "pinyin input exceeds the byte limit",
        ));
    }
    let normalized = input.to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok(ParseResult {
            normalized,
            status: ParseStatus::Empty,
            parses: Vec::new(),
        });
    }
    if !normalized
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
        || normalized.starts_with('\'')
        || normalized.contains("''")
    {
        return Ok(ParseResult {
            normalized,
            status: ParseStatus::Invalid,
            parses: Vec::new(),
        });
    }

    let len = normalized.len();
    let mut positions: Vec<Vec<Vec<&str>>> = vec![Vec::new(); len + 1];
    positions[0].push(Vec::new());
    for start in 0..len {
        if positions[start].is_empty() {
            continue;
        }
        let paths = positions[start].clone();
        if normalized.as_bytes()[start] == b'\'' {
            // 只有完整音节才可到达分隔符，不能把未完成音节带入下一个区段。
            for path in paths {
                add_path(&mut positions[start + 1], path);
            }
            continue;
        }
        let boundary = normalized[start..]
            .find('\'')
            .map_or(len, |offset| start + offset);
        for end in start + 1..=boundary.min(start + MAX_SYLLABLE_LEN) {
            let syllable = &normalized[start..end];
            if is_syllable(syllable) {
                for path in &paths {
                    let mut extended = path.clone();
                    extended.push(syllable);
                    add_path(&mut positions[end], extended);
                }
            }
        }
    }

    let mut parses = Vec::new();
    let trailing_separator = normalized.ends_with('\'');
    for path in &positions[len] {
        let mut syllables: Vec<_> = path.iter().map(|value| (*value).to_owned()).collect();
        if trailing_separator {
            syllables.push(String::new());
        }
        parses.push(PinyinParse {
            syllables,
            incomplete: trailing_separator,
        });
    }
    if !trailing_separator {
        for (start, paths) in positions.iter().enumerate().take(len) {
            let suffix = &normalized[start..];
            if !suffix.contains('\'') && is_strict_prefix(suffix) {
                for path in paths {
                    let mut syllables: Vec<_> =
                        path.iter().map(|value| (*value).to_owned()).collect();
                    syllables.push(suffix.to_owned());
                    parses.push(PinyinParse {
                        syllables,
                        incomplete: true,
                    });
                }
            }
        }
    }
    parses.sort_by(|left, right| {
        left.incomplete
            .cmp(&right.incomplete)
            .then_with(|| left.syllables.len().cmp(&right.syllables.len()))
            .then_with(|| left.syllables.cmp(&right.syllables))
    });
    parses.dedup();
    parses.truncate(MAX_PARSES);
    let status = if parses.iter().any(|path| !path.incomplete) {
        ParseStatus::Complete
    } else if !parses.is_empty() {
        ParseStatus::Incomplete
    } else {
        ParseStatus::Invalid
    };
    Ok(ParseResult {
        normalized,
        status,
        parses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_path(result: &ParseResult, syllables: &[&str], incomplete: bool) -> bool {
        result
            .parses
            .iter()
            .any(|path| path.syllables == syllables && path.incomplete == incomplete)
    }

    #[test]
    fn continuous_pinyin_and_uppercase_are_supported() {
        let result = parse("ZHONGguo").unwrap();
        assert_eq!(result.normalized, "zhongguo");
        assert_eq!(result.status, ParseStatus::Complete);
        assert!(has_path(&result, &["zhong", "guo"], false));
        assert!(has_path(&parse("nihao").unwrap(), &["ni", "hao"], false));
    }

    #[test]
    fn ambiguous_paths_have_a_stable_order() {
        let first = parse("xian").unwrap();
        assert_eq!(first.parses[0].syllables, ["xian"]);
        assert!(has_path(&first, &["xi", "an"], false));
        for _ in 0..50 {
            assert_eq!(parse("xian").unwrap(), first);
        }
    }

    #[test]
    fn explicit_separator_prevents_syllable_merging() {
        let result = parse("xi'an").unwrap();
        assert_eq!(result.status, ParseStatus::Complete);
        assert!(has_path(&result, &["xi", "an"], false));
        assert!(!has_path(&result, &["xian"], false));
        let waiting = parse("xi'").unwrap();
        assert_eq!(waiting.status, ParseStatus::Incomplete);
        assert!(has_path(&waiting, &["xi", ""], true));
        assert_eq!(parse("zh'an").unwrap().status, ParseStatus::Invalid);
    }

    #[test]
    fn trailing_partial_syllable_is_preserved() {
        let result = parse("niha").unwrap();
        assert!(has_path(&result, &["ni", "ha"], false));
        assert!(has_path(&result, &["ni", "ha"], true));
        let unfinished = parse("zhongg").unwrap();
        assert_eq!(unfinished.status, ParseStatus::Incomplete);
        assert!(has_path(&unfinished, &["zhong", "g"], true));
        assert_eq!(parse("z").unwrap().status, ParseStatus::Incomplete);
    }

    #[test]
    fn keyboard_umlaut_spelling_is_supported() {
        assert!(has_path(&parse("nv").unwrap(), &["nv"], false));
        assert!(has_path(&parse("lve").unwrap(), &["lve"], false));
        assert!(has_path(&parse("nve").unwrap(), &["nve"], false));
        assert!(is_syllable("lv"));
        assert!(!is_syllable("LV"));
        assert!(!is_syllable("nü"));
    }

    #[test]
    fn six_letter_syllables_and_standard_ju_qu_xu_spelling_work() {
        for (input, expected) in [
            ("shuangshou", vec!["shuang", "shou"]),
            ("chuangkou", vec!["chuang", "kou"]),
            ("zhuangtai", vec!["zhuang", "tai"]),
            ("quxuexi", vec!["qu", "xue", "xi"]),
            ("juzi", vec!["ju", "zi"]),
        ] {
            assert!(
                has_path(&parse(input).unwrap(), &expected, false),
                "{input}"
            );
        }
    }

    #[test]
    fn invalid_input_is_never_silently_truncated() {
        for input in ["ni2", "ni hao", "'ni", "ni''hao", "☁", "zzz", "q'a"] {
            let result = parse(input).unwrap();
            assert_eq!(result.normalized, input);
            assert_eq!(result.status, ParseStatus::Invalid, "{input}");
            assert!(result.parses.is_empty());
        }
        assert_eq!(parse("").unwrap().status, ParseStatus::Empty);
        assert!(parse(&"a".repeat(MAX_INPUT_LEN + 1)).is_err());
    }

    #[test]
    fn deleting_a_prefix_recomputes_the_segmentation() {
        let full = parse("zhongguo").unwrap();
        let deleted = parse("zhonggu").unwrap();
        assert!(has_path(&full, &["zhong", "guo"], false));
        assert!(has_path(&deleted, &["zhong", "gu"], true));
        assert!(!has_path(&deleted, &["zhong", "guo"], false));
    }

    #[test]
    fn ambiguous_long_input_is_bounded_and_covers_every_letter() {
        let input = "xian".repeat(MAX_INPUT_LEN / 4);
        let result = parse(&input).unwrap();
        assert!(result.parses.len() <= MAX_PARSES);
        assert!(!result.parses.is_empty());
        for path in &result.parses {
            assert_eq!(path.syllables.concat(), input);
        }
        assert_eq!(parse(&input).unwrap(), result);
    }
}
