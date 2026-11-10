//! 显式离线输入方案观测；原始轨迹仅由调用者指定，不进入产品日志。
use cloudleaf_input::{InputConfig, InputEngine, LanguageSnapshot, SearchSnapshot};
use cloudleaf_pinyin::{
    CorrectionConfig, DoublePinyinLayout, FuzzyDirection, FuzzyPair, FuzzyRule, PinyinScheme,
    SchemeConfig,
};
use cloudleaf_search::{
    Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker, profile_scheme_query,
    search_graph,
};
use cloudleaf_types::{InputEvent, Key};
use ring::digest::{SHA256, digest};
use std::{
    error::Error,
    fs,
    io::{self, Read},
    sync::Arc,
    time::Instant,
};

fn sha(text: &str) -> String {
    digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !matches!(args.len(), 4 | 6) {
        return Err(
            "usage: scheme_trace RUNTIME SHA SCHEME engine|profile [MODEL MODEL_SHA]".into(),
        );
    }
    let runtime = fs::read_to_string(&args[0])?;
    if sha(&runtime) != args[1] {
        return Err("词典SHA不符".into());
    }
    let mut scheme = SchemeConfig::default();
    match args[2].as_str() {
        "full" => {}
        "abbreviated" => scheme.scheme = PinyinScheme::AbbreviatedPinyin,
        "xiaohe" => scheme.scheme = PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
        "fuzzy-nl" => scheme.fuzzy.push(FuzzyRule {
            pair: FuzzyPair::NL,
            direction: FuzzyDirection::Both,
        }),
        "correction" => scheme.correction = Some(CorrectionConfig::default()),
        _ => return Err("未支持的显式研发配置".into()),
    }
    scheme.validate()?;
    let model_text = if args.len() == 6 {
        let text = fs::read_to_string(&args[4])?;
        if sha(&text) != args[5] {
            return Err("模型SHA不符".into());
        }
        Some(text)
    } else {
        None
    };
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    if input.len() > 65536 {
        return Err("研发轨迹超限".into());
    }
    let cases: Vec<_> = input
        .lines()
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(|line| line.split_once('\t').ok_or("轨迹字段错误"))
        .collect::<Result<_, _>>()?;
    let ranker = if let Some(text) = &model_text {
        if text.contains("\nkind\tlexical\n") {
            Ranker::Lexical
        } else if text.contains("\nkind\tcharacter_bigram") {
            Ranker::CharacterBigram
        } else {
            Ranker::Bigram
        }
    } else {
        Ranker::Structural
    };
    let config = Config {
        method: if matches!(ranker, Ranker::Structural | Ranker::Lexical) {
            Method::PositionDag
        } else {
            Method::ExactDag
        },
        ranker,
        output_k: 10,
        max_input_bytes: 64,
        max_letters: 64,
        ..Config::default()
    };
    println!(
        "IDENTITY\truntime={}\tmodel={}\tinput={}\tscheme={}\tK=10\tformal_gold=null",
        args[1],
        args.get(5).map_or("none", String::as_str),
        sha(&input),
        scheme.version_id()
    );
    match args[3].as_str() {
        "profile" => {
            let lexicon = Lexicon::from_tsv(&runtime)?;
            let language = model_text
                .as_ref()
                .map(|text| LanguageModelSnapshot::from_tsv(text, &lexicon, &args[1]))
                .transpose()?;
            for (case, raw) in cases {
                let started = Instant::now();
                let graph = Lattice::build_with_scheme(&lexicon, raw, &config, &scheme)?;
                let plain = search_graph(
                    &lexicon,
                    &graph,
                    &config,
                    language.as_ref().map(|m| m.model()),
                    None,
                )?;
                let plain_ns = started.elapsed().as_nanos();
                let (observed, profile, interpretation) = profile_scheme_query(
                    &lexicon,
                    raw,
                    &config,
                    &scheme,
                    language.as_ref().map(|m| m.model()),
                );
                let (observed_graph, observed) = observed?;
                assert_eq!(graph.edges(), observed_graph.edges());
                assert_eq!(plain, observed);
                println!(
                    "PROFILE\t{case}\tplain_ns={plain_ns}\tinterpretation={interpretation:?}\tclock_reads={}\tstats={:?}",
                    profile.clock_reads(),
                    graph.scheme_statistics()
                );
                for (phase, value) in profile.measurements() {
                    println!(
                        "PHASE\t{case}\t{}\t{}\t{}",
                        phase.name(),
                        value.nanos,
                        value.calls
                    );
                }
                println!("RESULT\t{case}\t{plain:?}");
            }
        }
        "engine" => {
            let snapshot = Arc::new(SearchSnapshot::from_verified_tsv(&runtime, &args[1])?);
            let language = model_text
                .as_ref()
                .map(|text| {
                    LanguageSnapshot::from_verified_tsv(text, &args[5], &snapshot).map(Arc::new)
                })
                .transpose()?;
            let mut engine = if let Some(model) = language {
                InputEngine::with_language(snapshot, config, model, None, InputConfig::default())?
            } else {
                InputEngine::with_search(snapshot, config, InputConfig::default())?
            };
            engine.reconfigure_scheme(scheme)?;
            engine.handle(InputEvent::Activate);
            for (case, raw) in cases {
                engine.handle(InputEvent::Key(Key::Escape));
                for (index, key) in raw.chars().enumerate() {
                    let now = Instant::now();
                    let result = engine.handle(InputEvent::Key(Key::Char(key)));
                    let ns = now.elapsed().as_nanos();
                    println!("EVENT\t{case}\t{index}\t{ns}\terror={:?}", result.error);
                    if result.error.is_some() {
                        return Err("输入事件被拒绝，未隐藏失败".into());
                    }
                }
                println!("FRAME\t{case}\t{:?}", engine.frame());
                println!("DIAGNOSTIC\t{case}\t{:?}", engine.search_diagnostic());
            }
        }
        _ => return Err("必须选择engine或profile".into()),
    }
    Ok(())
}
