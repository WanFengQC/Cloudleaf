//! 只比较现有排序与移除精确优先的消融，不修改产品算法。

use std::collections::BTreeSet;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use cloudleaf_dictionary::{Dictionary, PROTOTYPE_TSV, compare_entries};
use cloudleaf_pinyin::{MAX_PARSES, ParseResult, parse};
use cloudleaf_types::{Candidate, MatchKind};

const LIMIT: usize = 50;
const WARMUP: usize = 20;
const SAMPLES: usize = 100;
const ALGORITHMS: [&str; 2] = ["production-rules-v1", "frequency-only-ablation-v1"];
const HEADER: &str = "id\tsplit\tcategory\tgroup\tinput\tcontext\tgold\tcorrection\tnote";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut dataset = None;
    let mut output = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dataset" => dataset = Some(PathBuf::from(args.next().ok_or("缺少数据集路径")?)),
            "--output" => output = Some(PathBuf::from(args.next().ok_or("缺少输出路径")?)),
            _ => return Err("只接受 --dataset 和 --output 参数".into()),
        }
    }
    let dataset = dataset.ok_or("需要 --dataset")?;
    let output = output.ok_or("需要 --output")?;
    let dataset_resolved = dataset.canonicalize().map_err(|_| "数据集路径无法解析")?;
    let output_resolved = if output.exists() {
        output.canonicalize().map_err(|_| "输出路径无法解析")?
    } else {
        let parent = output
            .parent()
            .filter(|value| !value.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        parent
            .canonicalize()
            .map_err(|_| "输出目录必须存在")?
            .join(output.file_name().ok_or("输出路径缺少文件名")?)
    };
    if dataset_resolved == output_resolved {
        return Err("输出不能覆盖数据集".into());
    }
    let text = std::fs::read_to_string(dataset).map_err(|_| "数据集无法以UTF-8读取")?;
    let mut lines = text.lines();
    if lines.next() != Some("# cloudleaf-algorithm-evaluation-v1") || lines.next() != Some(HEADER) {
        return Err("评估数据版本或表头无效".into());
    }
    let dictionary = Dictionary::from_tsv(PROTOTYPE_TSV).map_err(|error| error.to_string())?;
    let lexicon: BTreeSet<_> = PROTOTYPE_TSV
        .lines()
        .skip(2)
        .filter_map(|line| line.split('\t').nth(2))
        .collect();
    let mut ids = BTreeSet::new();
    let mut report = String::from(
        "id\talgorithm\tcandidates\tgold_rank\tlexicon_known\tcorrection_attempt\tcorrection_correct\tsamples\tp50_ns\tp95_ns\tp99_ns\tmax_ns\terror\tdurations_ns\n",
    );
    for line in lines {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 9 || !ids.insert(fields[0]) || fields[0].is_empty() {
            return Err("数据集字段或唯一ID无效".into());
        }
        let id = fields[0];
        let input = fields[4];
        let gold: Vec<_> = fields[6].split('|').collect();
        let known = gold.iter().any(|value| lexicon.contains(value));
        for algorithm in ALGORITHMS {
            let expected = evaluate(input, &dictionary, algorithm);
            for _ in 0..WARMUP {
                if black_box(evaluate(input, &dictionary, algorithm)) != expected {
                    return Err(format!("预热候选不稳定：{id}"));
                }
            }
            let mut times = Vec::with_capacity(SAMPLES);
            for _ in 0..SAMPLES {
                let start = Instant::now();
                let actual = evaluate(input, &dictionary, algorithm);
                let elapsed = start.elapsed().as_nanos();
                if black_box(actual) != expected {
                    return Err(format!("正式测量候选不稳定：{id}"));
                }
                times.push(elapsed);
            }
            times.sort_unstable();
            let rank = expected
                .0
                .iter()
                .position(|value| gold.contains(&value.as_str()))
                .map_or(0, |index| index + 1);
            let percentile = |percent: usize| times[(times.len() * percent).div_ceil(100) - 1];
            let durations = times
                .iter()
                .map(u128::to_string)
                .collect::<Vec<_>>()
                .join(",");
            report.push_str(&format!("{id}\t{algorithm}\t{}\t{rank}\t{known}\tfalse\tfalse\t{SAMPLES}\t{}\t{}\t{}\t{}\t{}\t{durations}\n",
                expected.0.join("|"), percentile(50), percentile(95), percentile(99), times[times.len() - 1], expected.1.as_deref().unwrap_or("-")));
        }
    }
    if ids.is_empty() {
        return Err("数据集不能为空".into());
    }
    std::fs::write(output, report.as_bytes()).map_err(|_| "无法写入评估TSV")?;
    println!(
        "完成 {} 条样例、两个排序方案，每例预热 {WARMUP} 次、正式测量 {SAMPLES} 次。",
        ids.len()
    );
    Ok(())
}

fn evaluate(
    input: &str,
    dictionary: &Dictionary,
    algorithm: &str,
) -> (Vec<String>, Option<String>) {
    let parsed = match parse(input) {
        Ok(parsed) => parsed,
        Err(error) => return (Vec::new(), Some(format!("{:?}", error.code))),
    };
    let texts = if algorithm == "production-rules-v1" {
        cloudleaf_candidate::generate(&parsed, dictionary, LIMIT)
            .into_iter()
            .map(|candidate| candidate.text)
            .collect()
    } else {
        frequency_only(&parsed, dictionary)
    };
    (texts, None)
}

fn frequency_only(parsed: &ParseResult, dictionary: &Dictionary) -> Vec<String> {
    let mut pool = Vec::new();
    for path in parsed.parses.iter().take(MAX_PARSES) {
        let kind = if path.incomplete {
            MatchKind::Prefix
        } else {
            MatchKind::Exact
        };
        pool.extend(
            dictionary
                .query_limited(path, LIMIT)
                .into_iter()
                .map(|entry| (entry, kind)),
        );
    }
    pool.sort_unstable_by(|left, right| compare_entries(left.0, right.0));
    let mut seen = BTreeSet::new();
    // 两种方案同样物化候选字段，避免字段复制差异成为排序规则的性能收益。
    let candidates: Vec<_> = pool
        .into_iter()
        .filter(|(entry, _)| seen.insert(entry.text.as_str()))
        .take(LIMIT)
        .map(|(entry, match_kind)| Candidate {
            id: entry.id.clone(),
            text: entry.text.clone(),
            annotation: entry.annotation.clone(),
            source: entry.source.clone(),
            frequency: entry.frequency,
            match_kind,
        })
        .collect();
    candidates
        .into_iter()
        .map(|candidate| candidate.text)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dictionary(rows: &str) -> Dictionary {
        Dictionary::from_tsv(&format!(
            "# cloudleaf-dictionary-v1\nid\tpinyin\ttext\tfrequency\tannotation\tsource\n{rows}"
        ))
        .unwrap()
    }

    #[test]
    fn removing_exact_priority_changes_the_synthetic_control() {
        let dictionary = dictionary("a\tni\t你\t1\tyou\ttest\nb\tning\t宁\t1000\tpeace\ttest\n");
        assert_eq!(evaluate("ni", &dictionary, ALGORITHMS[0]).0, ["你", "宁"]);
        assert_eq!(evaluate("ni", &dictionary, ALGORITHMS[1]).0, ["宁", "你"]);
    }

    #[test]
    fn ablation_keeps_duplicate_texts_stable_across_parse_paths() {
        let dictionary = dictionary(
            "a\txian\t先\t1\tfirst\ttest\nb\txi an\t先\t100\tfirst\ttest\nc\txi an\t西安\t50\tcity\ttest\n",
        );
        let expected = vec!["先".to_owned(), "西安".to_owned()];
        for _ in 0..20 {
            assert_eq!(evaluate("xian", &dictionary, ALGORITHMS[1]).0, expected);
        }
    }
}
