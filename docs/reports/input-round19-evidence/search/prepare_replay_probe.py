"""原R18方案配置保持一致，仅扩展临时探针参数。"""
from pathlib import Path
p=Path(__file__).resolve().parent
path=p/'source/probe/src/main.rs'
s=path.read_text(encoding='utf-8')
s=s.replace('use cloudleaf_pinyin::{PinyinScheme, SchemeConfig};','use cloudleaf_pinyin::{PinyinScheme, SchemeConfig, DoublePinyinLayout, CorrectionConfig, FuzzyRule, FuzzyPair, FuzzyDirection};')
start=s.index('    let scheme = SchemeConfig {')
end=s.index('    let config = Config {',start)
s=s[:start]+'''    let mut scheme=SchemeConfig::default();
    match args.get(4).map(String::as_str).unwrap_or("abbreviated") {
        "full"=>{},
        "abbreviated"=>scheme.scheme=PinyinScheme::AbbreviatedPinyin,
        "xiaohe"=>scheme.scheme=PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
        "fuzzy-nl"=>scheme.fuzzy.push(FuzzyRule{pair:FuzzyPair::NL,direction:FuzzyDirection::Both}),
        "correction"=>scheme.correction=Some(CorrectionConfig::default()),
        _=>return Err("未支持的固定方案".into()),
    }
'''+s[end:]
path.write_text(s,encoding='utf-8')
