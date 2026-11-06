//! 显式只读研发入口；不获取TSF manager，不启动平台资源，也不注册服务。
use super::*;
use std::path::Path;

pub(crate) fn run(runtime: &str, expected_sha256: &str) -> Result<()> {
    run_options(runtime, expected_sha256, None)
}

pub(crate) fn run_with_language(
    runtime: &str,
    expected_sha256: &str,
    model: &str,
    model_sha: &str,
) -> Result<()> {
    run_options(runtime, expected_sha256, Some((model, model_sha)))
}

fn run_options(runtime: &str, expected_sha256: &str, model: Option<(&str, &str)>) -> Result<()> {
    let text = std::fs::read_to_string(Path::new(runtime))
        .map_err(|_| Error::new(E_FAIL, "无法读取指定研发词典"))?;
    let snapshot = Arc::new(
        SearchSnapshot::from_verified_tsv(&text, expected_sha256)
            .map_err(|_| Error::new(E_FAIL, "研发词典快照校验失败"))?,
    );
    let version = snapshot.version().to_owned();
    let entries = snapshot.entries().len();
    let mut config = cloudleaf_search::Config {
        method: cloudleaf_search::Method::PositionDag,
        ranker: cloudleaf_search::Ranker::Structural,
        max_input_bytes: 64,
        max_letters: 64,
        output_k: 5,
        ..cloudleaf_search::Config::default()
    };
    let host = if let Some((path, expected)) = model {
        let text = std::fs::read_to_string(path)
            .map_err(|_| Error::new(E_FAIL, "无法读取研发语言模型"))?;
        let language = Arc::new(
            cloudleaf_input::LanguageSnapshot::from_verified_tsv(&text, expected, &snapshot)
                .map_err(|_| Error::new(E_FAIL, "语言模型快照校验失败"))?,
        );
        println!(
            "model={} metadata={:?} estimated_bytes={}",
            language.version(),
            language.metadata(),
            language.estimated_bytes()
        );
        if language.metadata().kind == "lexical" {
            config.ranker = cloudleaf_search::Ranker::Lexical;
        } else {
            config.ranker = cloudleaf_search::Ranker::Bigram;
            config.method = cloudleaf_search::Method::ExactDag;
        }
        Host::with_language(snapshot, config, language, InputConfig::default())?
    } else {
        Host::with_search(snapshot, config, InputConfig::default())?
    };
    if !host.reserve_epoch() {
        return Err(Error::new(E_FAIL, "当前模块容量不足，未启动研发输入"));
    }
    println!("review=input-host experimental=true snapshot={version} entries={entries}");
    println!(
        "host_charge={:?} epoch_cost={:?}",
        host.recovery_budget.charge(),
        host.epoch_cost
    );
    println!("platform_context=none TSF_registered=false native_commit=not-run formal_gold=absent");
    let actual = host.engine.borrow_mut().handle(InputEvent::Activate);
    let projected = host.projection.borrow_mut().handle(InputEvent::Activate);
    if !same_semantics(&actual, &projected) {
        return Err(Error::new(E_FAIL, "激活时双引擎语义不一致"));
    }
    for input in ["nihao", "wozhengzaikaifayikuanzhongwenshurufa"] {
        for key in std::iter::once(Key::Escape).chain(input.chars().map(Key::Char)) {
            let actual = host.engine.borrow_mut().handle(InputEvent::Key(key));
            let projected = host.projection.borrow_mut().handle(InputEvent::Key(key));
            if !same_semantics(&actual, &projected) {
                return Err(Error::new(E_FAIL, "实际输入与投影候选不一致"));
            }
        }
        let frame = host.engine.borrow().frame();
        println!(
            "input={input:?} revision={} preedit={:?} candidates={} page_start={} page_size={} paired=true",
            frame.revision,
            frame.preedit,
            frame.candidates.len(),
            frame.page_start,
            frame.page_size
        );
        for (index, candidate) in frame.candidates.iter().enumerate() {
            println!(
                "candidate={} id={:?} text={:?} source={:?} frequency={:?}",
                index + 1,
                candidate.id,
                candidate.text,
                candidate.source,
                candidate.frequency
            );
        }
        if let Some(diagnostic) = host.engine.borrow().search_diagnostic() {
            println!(
                "method={:?} ranker={:?} complete={} waiting={} budget={:?}",
                diagnostic.method,
                diagnostic.ranker,
                diagnostic.guaranteed,
                diagnostic.waiting,
                diagnostic.statistics.budget
            );
            for path in &diagnostic.paths {
                println!(
                    "path={:?} score={} features={:?} edges={:?}",
                    path.candidate_id, path.score, path.features, path.edges
                );
            }
        }
    }
    println!("result=paired-core-only host_selection=not-run real_application_acceptance=false");
    Ok(())
}
