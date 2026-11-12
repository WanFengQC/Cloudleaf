use cloudleaf_pinyin::*;
fn main() {
    let config = SchemeConfig {
        correction: Some(CorrectionConfig::default()),
        ..SchemeConfig::default()
    };
    for text in [
        "zhnog",
        "zhnogguo",
        "nihso",
        "nihoa",
        "woaoni",
        "jintiantiqnqihenhao",
        "wozhengzaikaifayikuanzhongwenshurufa",
    ] {
        let before = std::time::Instant::now();
        let graph = interpret(text, &config).unwrap();
        println!(
            "{text}\tbudget={:?}\twork={}\tarcs={}\tcorrections={}\telapsed={}",
            graph.budget,
            graph.work,
            graph.arcs.len(),
            graph
                .arcs
                .iter()
                .filter(|a| a.kind == PhoneticMatch::Correction)
                .count(),
            before.elapsed().as_nanos()
        );
    }
}
