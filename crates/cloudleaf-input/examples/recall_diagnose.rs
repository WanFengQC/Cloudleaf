//! 离线追踪已知研发正文经过音节、词面、词条格、召回池和排序的路径。
use cloudleaf_pinyin::{BaseMatch, PhoneticHypotheses, SchemeConfig, interpret};
use cloudleaf_search::{
    BigramModel, Config, Entry, LanguageModelSnapshot, Lattice, Lexicon, Method, Ranker,
    ResearchRanker, SearchResult, WordContextModel, search_graph,
};
use ring::digest::{SHA256, digest};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;

type Outcome<T> = Result<T, Box<dyn Error>>;
type SurfaceIndex<'a> = BTreeMap<&'a str, Vec<&'a Entry>>;
const MAX_KEYS: usize = 64;
const MAX_TARGET_CHARS: usize = 64;
const POOL: usize = 50;

fn verified(path: &str, expected: &str) -> Outcome<String> {
    let text = fs::read_to_string(path)?;
    let sha: String = digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if sha != expected {
        return Err("研究资源 SHA-256 不匹配".into());
    }
    Ok(text)
}

fn indices(target: &str) -> Vec<usize> {
    target
        .char_indices()
        .map(|(index, _)| index)
        .chain([target.len()])
        .collect()
}

fn surface_coverage(
    index: &SurfaceIndex<'_>,
    target: &str,
    bytes: &[usize],
) -> (usize, Vec<usize>) {
    let mut reachable = vec![false; bytes.len()];
    reachable[0] = true;
    for (start, &offset) in bytes.iter().enumerate() {
        if reachable[start] {
            for (end, &end_offset) in bytes.iter().enumerate().skip(start + 1) {
                reachable[end] |= index.contains_key(&target[offset..end_offset]);
            }
        }
    }
    let singleton_gaps = bytes
        .windows(2)
        .enumerate()
        .filter(|(_, bounds)| !index.contains_key(&target[bounds[0]..bounds[1]]))
        .map(|(index, _)| index)
        .collect();
    (
        reachable.iter().rposition(|value| *value).unwrap_or(0),
        singleton_gaps,
    )
}

fn lexical_reading(
    index: &SurfaceIndex<'_>,
    target: &str,
    bytes: &[usize],
    phonetics: &PhoneticHypotheses,
) -> (bool, bool, bool) {
    let mut arcs = BTreeMap::new();
    let mut full_syllables = vec![false; phonetics.keys.len() + 1];
    full_syllables[0] = true;
    for arc in phonetics
        .arcs
        .iter()
        .filter(|arc| arc.base == BaseMatch::Exact)
    {
        arcs.insert((arc.start, arc.syllable.as_str()), arc.end);
        full_syllables[arc.end] |= full_syllables[arc.start];
    }

    let mut flat = vec![BTreeSet::new(); bytes.len()];
    let mut exact = flat.clone();
    flat[0].insert(0);
    exact[0].insert(0);
    for (start, &offset) in bytes.iter().enumerate() {
        let flat_starts = std::mem::take(&mut flat[start]);
        let exact_starts = std::mem::take(&mut exact[start]);
        if start == bytes.len() - 1 {
            let end = phonetics.keys.len();
            return (
                !phonetics.waiting && full_syllables[end],
                flat_starts.contains(&end),
                !phonetics.waiting && exact_starts.contains(&end),
            );
        }
        for (end, &end_offset) in bytes.iter().enumerate().skip(start + 1) {
            if let Some(entries) = index.get(&target[offset..end_offset]) {
                for entry in entries {
                    let pinyin = entry.pinyin.replace(' ', "");
                    for &pos in &flat_starts {
                        if phonetics.keys[pos..].starts_with(&pinyin) {
                            flat[end].insert(pos + pinyin.len());
                        }
                    }
                    for &pos in &exact_starts {
                        let next = entry
                            .pinyin
                            .split_ascii_whitespace()
                            .try_fold(pos, |key, syl| arcs.get(&(key, syl)).copied());
                        if let Some(position) = next {
                            exact[end].insert(position);
                        }
                    }
                }
            }
        }
    }
    unreachable!("目标字节索引至少含有终点")
}

#[derive(Clone, Copy, Default)]
struct PathProof {
    count: u64,
    previous_edge: Option<usize>,
    previous_offset: usize,
}

fn graph_path(lexicon: &Lexicon, graph: &Lattice<'_>, target: &str) -> (u64, Vec<usize>) {
    let bytes = indices(target);
    let last = bytes.len() - 1;
    let mut reached = vec![vec![PathProof::default(); bytes.len()]; graph.letters().len() + 1];
    reached[0][0].count = 1;
    // 边按起点排序，所有边消耗至少一个按键。
    for (edge_index, edge) in graph.edges().iter().enumerate() {
        let word = &lexicon.entries()[edge.entry].text;
        for (index, &offset) in bytes.iter().enumerate().take(last) {
            let prior = reached[edge.start][index];
            if prior.count == 0 || !target[offset..].starts_with(word) {
                continue;
            }
            let next = bytes
                .binary_search(&(offset + word.len()))
                .expect("匹配词条后仍为字符边界");
            let proof = &mut reached[edge.end][next];
            if proof.count == 0 {
                proof.previous_edge = Some(edge_index);
                proof.previous_offset = index;
            }
            proof.count = proof.count.saturating_add(prior.count);
        }
    }
    let mut key = graph.letters().len();
    let mut offset = last;
    let count = reached[key][offset].count;
    let mut example = Vec::new();
    while let Some(edge_index) = reached[key][offset].previous_edge {
        offset = reached[key][offset].previous_offset;
        key = graph.edges()[edge_index].start;
        example.push(edge_index);
    }
    example.reverse();
    (count, example)
}

fn audit_pool(
    graph: &Lattice<'_>,
    target: &str,
    id: &str,
    mode: &str,
    pool: &SearchResult,
    models: (&WordContextModel, &BigramModel),
) -> Outcome<()> {
    let (words, characters) = models;
    let target_index = pool.candidates.iter().position(|item| item.text == target);
    println!(
        "POOL\t{id}\t{mode}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{:?}",
        pool.candidates.len(),
        pool.statistics.budget,
        pool.guaranteed,
        pool.statistics.expansions,
        pool.statistics.resident_high_water,
        pool.statistics.beam_pruned,
        target_index.map(|index| index + 1)
    );
    if let Some(index) = target_index {
        let candidate = &pool.candidates[index];
        println!(
            "REPRESENTATIVE\t{id}\t{mode}\t{:?}\t{:?}\t{:?}",
            candidate.entry_ids, candidate.edge_indices, candidate.features
        );
        for (label, ranker) in [
            ("finite-character", ResearchRanker::Character),
            ("word", ResearchRanker::Word),
            ("mixed", ResearchRanker::Mixed),
        ] {
            let ordered = words.rank_pool(graph, pool, ranker, Some(characters))?;
            if let Some(rank) = ordered.iter().position(|row| row.pool_index == index) {
                println!(
                    "ORDER\t{id}\t{mode}\t{label}\t{}\t{:?}",
                    rank + 1,
                    ordered[rank]
                );
            }
        }
    }
    Ok(())
}

fn main() -> Outcome<()> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let diagnostic_resident = if args.len() == 8 {
        let argument = args.pop().unwrap();
        argument
            .strip_prefix("--diagnostic-resident=")
            .ok_or("无效的离线驻留诊断参数")?
            .parse::<usize>()?
    } else {
        Config::default().max_resident_paths
    };
    if !(50_000..=200_000).contains(&diagnostic_resident) {
        return Err("离线驻留诊断预算应在 50000 至 200000 之间".into());
    }
    let [runtime, lex_sha, word, word_sha, witten, witten_sha, input] = args.as_slice() else {
        return Err("usage: recall_diagnose RUNTIME SHA WORD SHA WITTEN SHA CASES_TSV".into());
    };
    let lexicon = Lexicon::from_tsv(&verified(runtime, lex_sha)?)?;
    let words = WordContextModel::from_tsv(&verified(word, word_sha)?, &lexicon, lex_sha)?;
    let witten =
        LanguageModelSnapshot::from_tsv(&verified(witten, witten_sha)?, &lexicon, lex_sha)?;
    let mut index = SurfaceIndex::new();
    for entry in lexicon.entries() {
        index.entry(&entry.text).or_default().push(entry);
    }
    let cases = fs::read_to_string(input)?;
    if cases.len() > 1024 * 1024 {
        return Err("研发样本文件超限".into());
    }
    println!(
        "SOURCE\t{lex_sha}\t{word_sha}\t{witten_sha}\t{}",
        lexicon.entries().len()
    );
    let structural = Config {
        ranker: Ranker::Structural,
        method: Method::PositionDag,
        output_k: POOL,
        context_pool: POOL,
        max_input_bytes: MAX_KEYS,
        max_letters: MAX_KEYS,
        ..Config::default()
    };
    let character = Config {
        ranker: Ranker::CharacterBigram,
        method: Method::ExactDag,
        max_resident_paths: diagnostic_resident,
        ..structural.clone()
    };
    let scheme = SchemeConfig::default();
    let mut ids = BTreeSet::new();
    for line in cases
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let mut row = line.split('\t');
        let [Some(id), Some(raw), Some(origin), Some(target), None] =
            std::array::from_fn(|_| row.next())
        else {
            return Err("样本应含标识、拼音、拼音来源及正文四列".into());
        };
        if !ids.insert(id)
            || id.is_empty()
            || target.is_empty()
            || target.chars().count() > MAX_TARGET_CHARS
            || !matches!(origin, "historical" | "supplement-counterfactual")
        {
            return Err("样本标识、正文或拼音来源无效".into());
        }
        println!("CASE\t{id}\t{origin}\t{raw}\t{target}");
        let bytes = indices(target);
        let (surface_end, gaps) = surface_coverage(&index, target, &bytes);
        println!(
            "SURFACE\t{id}\t{}\t{surface_end}\t{gaps:?}",
            bytes.len() - 1
        );
        let phonetics = match interpret(raw, &scheme) {
            Ok(value) => value,
            Err(error) => {
                println!("INPUT_ERROR\t{id}\t{error}");
                continue;
            }
        };
        let (legal, flat, exact) = lexical_reading(&index, target, &bytes, &phonetics);
        println!(
            "PHONETIC\t{id}\t{legal}\t{flat}\t{exact}\t{:?}\t{}\t{}",
            phonetics.budget,
            phonetics.arcs.len(),
            phonetics.work
        );
        let graph = match Lattice::build_with_scheme(&lexicon, raw, &structural, &scheme) {
            Ok(value) => value,
            Err(error) => {
                println!("GRAPH_ERROR\t{id}\t{error}");
                continue;
            }
        };
        let (paths, witness) = graph_path(&lexicon, &graph, target);
        let entries: Vec<_> = witness
            .iter()
            .map(|&edge| &lexicon.entries()[graph.edges()[edge].entry].id)
            .collect();
        println!(
            "LATTICE\t{id}\t{:?}\t{}\t{paths}\t{witness:?}\t{entries:?}",
            graph.budget(),
            graph.edges().len()
        );
        for (mode, config, model) in [
            ("structural", &structural, None),
            ("character", &character, Some(witten.model())),
        ] {
            let pool = search_graph(&lexicon, &graph, config, model, None)?;
            audit_pool(&graph, target, id, mode, &pool, (&words, witten.model()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_witness_counts_distinct_paths_and_respects_separator() {
        let lexicon = Lexicon::from_tsv(concat!(
            "# cloudleaf-lexicon-v2\n",
            "id\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n",
            "a1\tan\t安\t\tunknown\t\tx\ta1\tMIT\n",
            "a2\ta\t啊\t\tunknown\t\tx\ta2\tMIT\n",
            "n1\tnan\t南\t\tunknown\t\tx\tn1\tMIT\n",
            "n2\tan\t南\t\tunknown\t\tx\tn2\tMIT\n",
            "p1\tan nan\t安南\t\tunknown\t\tx\tp1\tMIT\n"
        )).unwrap();
        let plain = Lattice::build(&lexicon, "annan", &Config::default()).unwrap();
        let (count, witness) = graph_path(&lexicon, &plain, "安南");
        assert_eq!(count, 2);
        assert!(!witness.is_empty());
        let separated = Lattice::build(&lexicon, "a'nan", &Config::default()).unwrap();
        assert_eq!(graph_path(&lexicon, &separated, "安南"), (0, vec![]));
        assert_eq!(graph_path(&lexicon, &separated, "啊南").0, 1);
        let ph = interpret("a'nan", &SchemeConfig::default()).unwrap();
        let mut index = SurfaceIndex::new();
        for entry in lexicon.entries() {
            index.entry(&entry.text).or_default().push(entry);
        }
        assert_eq!(
            lexical_reading(&index, "安南", &indices("安南"), &ph),
            (true, true, false)
        );
        assert_eq!(
            surface_coverage(&index, "安陌南", &indices("安陌南")),
            (1, vec![1])
        );
    }
}
