//! 独立研发评估。只有诊断和交付行消费来源正文。
use cloudleaf_input::SchemeConfig;
use cloudleaf_search::{
    Cancellation, Config, LanguageModelSnapshot, Lattice, Lexicon, Ranker, ResearchRanker,
    WordContextModel, expected_reachability, recall_context, search_graph,
};
use ring::digest::{SHA256, digest};
use std::{error::Error, fs, time::Instant};

type Outcome<T> = Result<T, Box<dyn Error>>;

fn verified(path: &str, expected: &str) -> Outcome<String> {
    let bytes = fs::read(path)?;
    let identity: String = digest(&SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if identity != expected {
        return Err("研发文件 SHA-256 不符".into());
    }
    Ok(String::from_utf8(bytes)?)
}

fn budget(value: Option<cloudleaf_search::Budget>) -> String {
    value.map_or_else(|| "none".to_owned(), |kind| format!("{kind:?}"))
}

fn main() -> Outcome<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [
        lexicon,
        lexicon_sha,
        word,
        word_sha,
        character,
        character_sha,
        cases,
        cases_sha,
    ] = args.as_slice()
    else {
        return Err("usage: language_audit LEX SHA WORD SHA CHAR SHA CASES SHA".into());
    };
    let lexicon = Lexicon::from_tsv(&verified(lexicon, lexicon_sha)?)?;
    let words = WordContextModel::from_tsv(&verified(word, word_sha)?, &lexicon, lexicon_sha)?;
    let chars = LanguageModelSnapshot::from_tsv(
        &verified(character, character_sha)?,
        &lexicon,
        lexicon_sha,
    )?;
    println!(
        "MODEL\t{}\t{}\t{}\t{}",
        words.corpus_sha256(),
        words.estimated_bytes(),
        words.pair_count(),
        chars.estimated_bytes()
    );
    let config = Config {
        ranker: Ranker::CharacterBigram,
        output_k: 50,
        context_pool: 50,
        max_input_bytes: 64,
        max_letters: 64,
        ..Config::default()
    };
    let cancel = Cancellation::default();
    let scheme = SchemeConfig::default();
    for line in verified(cases, cases_sha)?.lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        let [id, status, raw, desired] = fields.as_slice() else {
            return Err("研发样本列不符".into());
        };
        if *status != "full" {
            println!("CASE\t{id}\t{status}\tundefined");
            continue;
        }
        let start = Instant::now();
        let graph = Lattice::build_with_scheme(&lexicon, raw, &config, &scheme)?;
        let lattice_ns = start.elapsed().as_nanos();
        println!(
            "CASE\t{id}\tfull\t{}\t{}\t{lattice_ns}",
            expected_reachability(&graph, desired, 200_000).map_or("unknown", |v| if v {
                "yes"
            } else {
                "no"
            }),
            graph.edges().len()
        );
        let start = Instant::now();
        let exact = search_graph(&lexicon, &graph, &config, Some(chars.model()), None)?;
        let exact_ns = start.elapsed().as_nanos();
        println!(
            "RUN\t{id}\tcharacter\t{exact_ns}\t{}\t{}\t{}\t{}\t{}",
            exact.candidates.len(),
            budget(exact.statistics.budget),
            exact.statistics.expansions,
            exact.statistics.resident_high_water,
            exact.guaranteed
        );
        let start = Instant::now();
        let old = words.rank_pool(&graph, &exact, ResearchRanker::Word, Some(chars.model()))?;
        let old_ns = start.elapsed().as_nanos();
        println!(
            "RUN\t{id}\tword\t{old_ns}\t{}\t{}\t0\t0\t{}",
            old.len(),
            budget(exact.statistics.budget),
            exact.guaranteed
        );
        let start = Instant::now();
        let current = words.rank_supported_character_pool(&graph, &exact)?;
        let current_ns = start.elapsed().as_nanos();
        println!(
            "RUN\t{id}\tsupported\t{current_ns}\t{}\t{}\t0\t0\t{}",
            current.len(),
            budget(exact.statistics.budget),
            exact.guaranteed
        );
        let mut word = vec![None; old.len()];
        for (rank, value) in old.iter().enumerate() {
            word[value.pool_index] = Some((rank + 1, value));
        }
        for (rank, value) in current.iter().enumerate() {
            let path = &exact.candidates[value.pool_index];
            let (word_rank, old) = word[value.pool_index].expect("同一个固定候选池");
            println!(
                "PATH\t{id}\t{}\t{}\t{word_rank}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}",
                value.pool_index + 1,
                rank + 1,
                path.text == *desired,
                value.character_units,
                old.unigram_units,
                old.word_units,
                old.boundary_units,
                old.observed_words,
                old.unknown_words,
                old.final_units,
                value.transition_gain,
                value.supporting_transitions,
                value.bonus_units,
                path.features.character_count,
                path.features.structural_units,
                path.entry_ids.join(","),
                path.edge_indices
            );
        }
        let start = Instant::now();
        let beam = recall_context(&graph, &config, chars.model(), &words, &cancel)?;
        let beam_ns = start.elapsed().as_nanos();
        println!(
            "RUN\t{id}\tbeam\t{beam_ns}\t{}\t{}\t{}\t{}\t{}",
            beam.candidates.len(),
            budget(beam.work.budget),
            beam.work.expansions,
            beam.work.resident_high_water,
            beam.exhaustive_for_score
        );
        for (rank, row) in beam.candidates.iter().enumerate() {
            let path = &row.path;
            println!(
                "BEAM\t{id}\t{}\t{}\t{}\t{}\t{}\t{:?}",
                rank + 1,
                path.text == *desired,
                path.features.character_units,
                row.word_gain,
                path.entry_ids.join(","),
                path.edge_indices
            );
        }
    }
    Ok(())
}
