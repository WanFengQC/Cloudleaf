//! 小鹤纯双拼事实键位的独立实现；不包含原输入法核心、形码或词库。

pub const XIAOHE_LAYOUT_VERSION: &str = "xiaohe-flypy-up-2026-09-27-v1";

pub const XIAOHE_INITIALS: &[(&str, u8)] = &[("zh", b'v'), ("ch", b'i'), ("sh", b'u')];
pub const XIAOHE_FINALS: &[(&str, u8)] = &[
    ("a", b'a'), ("ai", b'd'), ("an", b'j'), ("ang", b'h'), ("ao", b'c'),
    ("e", b'e'), ("ei", b'w'), ("en", b'f'), ("eng", b'g'),
    ("i", b'i'), ("ia", b'x'), ("ian", b'm'), ("iang", b'l'), ("iao", b'n'),
    ("ie", b'p'), ("in", b'b'), ("ing", b'k'), ("iong", b's'), ("iu", b'q'),
    ("o", b'o'), ("ong", b's'), ("ou", b'z'),
    ("u", b'u'), ("ua", b'x'), ("uai", b'k'), ("uan", b'r'), ("uang", b'l'),
    ("ue", b't'), ("ui", b'v'), ("un", b'y'), ("uo", b'o'),
    ("v", b'v'), ("ve", b't'),
];

/// 参数为入口已验证的小写规范音节，调用者负责合法音节集合和大小写/ü归一。
pub fn xiaohe_code(syllable: &str) -> Option<[u8; 2]> {
    let first = *syllable.as_bytes().first()?;
    if matches!(first, b'a' | b'e' | b'o') {
        return match syllable.as_bytes() {
            [a] => Some([*a, *a]),
            [a, b] => Some([*a, *b]),
            _ => XIAOHE_FINALS.iter().find(|(final_, _)| *final_ == syllable)
                .map(|(_, key)| [first, *key]),
        };
    }
    let (initial_key, final_) = XIAOHE_INITIALS.iter()
        .find_map(|(initial, key)| syllable.strip_prefix(initial).map(|rest| (*key, rest)))
        .unwrap_or((first, &syllable[1..]));
    XIAOHE_FINALS.iter().find(|(value, _)| *value == final_)
        .map(|(_, key)| [initial_key, *key])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_rules_and_independently_worked_examples() {
        for (syllable, expected) in [
            ("a", "aa"), ("e", "ee"), ("o", "oo"), ("ai", "ai"),
            ("an", "an"), ("ao", "ao"), ("ei", "ei"), ("en", "en"),
            ("er", "er"), ("ou", "ou"), ("ang", "ah"), ("eng", "eg"),
            ("zhong", "vs"), ("chong", "is"), ("shuang", "ul"),
            ("ni", "ni"), ("hao", "hc"), ("pin", "pb"), ("nv", "nv"),
            ("lv", "lv"), ("nve", "nt"), ("lve", "lt"), ("xue", "xt"),
            ("ju", "ju"), ("qu", "qu"), ("xu", "xu"), ("yu", "yu"),
            ("juan", "jr"), ("jun", "jy"), ("yi", "yi"), ("yin", "yb"),
            ("ying", "yk"), ("wo", "wo"), ("yong", "ys"), ("wen", "wf"),
        ] {
            assert_eq!(xiaohe_code(syllable).map(|v| String::from_utf8(v.to_vec()).unwrap()), Some(expected.to_owned()), "{syllable}");
        }
    }
    #[test]
    fn key_table_has_unique_final_rows_and_ascii_letter_codes() {
        for (i, (final_, key)) in XIAOHE_FINALS.iter().enumerate() {
            assert!(key.is_ascii_lowercase());
            assert!(!XIAOHE_FINALS[..i].iter().any(|(other, _)| other == final_));
        }
    }
}

fn main(){ for syllable in ["a","ai","an","ang","ao","ba","bai","ban","bang","bao","bei","ben","beng","bi","bian","biao","bie","bin","bing","bo","bu","ca","cai","can","cang","cao","ce","cen","ceng","cha","chai","chan","chang","chao","che","chen","cheng","chi","chong","chou","chu","chua","chuai","chuan","chuang","chui","chun","chuo","ci","cong","cou","cu","cuan","cui","cun","cuo","da","dai","dan","dang","dao","de","dei","den","deng","di","dia","dian","diao","die","ding","diu","dong","dou","du","duan","dui","dun","duo","e","ei","en","eng","er","fa","fan","fang","fei","fen","feng","fo","fou","fu","ga","gai","gan","gang","gao","ge","gei","gen","geng","gong","gou","gu","gua","guai","guan","guang","gui","gun","guo","ha","hai","han","hang","hao","he","hei","hen","heng","hong","hou","hu","hua","huai","huan","huang","hui","hun","huo","ji","jia","jian","jiang","jiao","jie","jin","jing","jiong","jiu","ju","juan","jue","jun","ka","kai","kan","kang","kao","ke","kei","ken","keng","kong","kou","ku","kua","kuai","kuan","kuang","kui","kun","kuo","la","lai","lan","lang","lao","le","lei","leng","li","lia","lian","liang","liao","lie","lin","ling","liu","lo","long","lou","lu","luan","lun","luo","lv","lve","ma","mai","man","mang","mao","me","mei","men","meng","mi","mian","miao","mie","min","ming","miu","mo","mou","mu","na","nai","nan","nang","nao","ne","nei","nen","neng","ni","nian","niang","niao","nie","nin","ning","niu","nong","nou","nu","nuan","nuo","nv","nve","o","ou","pa","pai","pan","pang","pao","pei","pen","peng","pi","pian","piao","pie","pin","ping","po","pou","pu","qi","qia","qian","qiang","qiao","qie","qin","qing","qiong","qiu","qu","quan","que","qun","ran","rang","rao","re","ren","reng","ri","rong","rou","ru","ruan","rui","run","ruo","sa","sai","san","sang","sao","se","sen","seng","sha","shai","shan","shang","shao","she","shei","shen","sheng","shi","shou","shu","shua","shuai","shuan","shuang","shui","shun","shuo","si","song","sou","su","suan","sui","sun","suo","ta","tai","tan","tang","tao","te","teng","ti","tian","tiao","tie","ting","tong","tou","tu","tuan","tui","tun","tuo","wa","wai","wan","wang","wei","wen","weng","wo","wu","xi","xia","xian","xiang","xiao","xie","xin","xing","xiong","xiu","xu","xuan","xue","xun","ya","yan","yang","yao","ye","yi","yin","ying","yo","yong","you","yu","yuan","yue","yun","za","zai","zan","zang","zao","ze","zei","zen","zeng","zha","zhai","zhan","zhang","zhao","zhe","zhei","zhen","zheng","zhi","zhong","zhou","zhu","zhua","zhuai","zhuan","zhuang","zhui","zhun","zhuo","zi","zong","zou","zu","zuan","zui","zun","zuo"] { println!("{}\t{}",syllable,xiaohe_code(syllable).map(|v|String::from_utf8(v.to_vec()).unwrap()).unwrap_or_else(||"UNMAPPED".into()));}}
