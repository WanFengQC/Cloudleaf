//! 离线研发训练器；源序列和元数据显式提供，产物不自动进入输入配置。
use cloudleaf_search::{LanguageModelSnapshot, Lexicon};
use ring::digest::{SHA256, digest};
use std::{error::Error, fs, io::Write};

fn sha256(text: &str) -> String {
    digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err(
            "usage: train_language RUNTIME RUNTIME_SHA SEQUENCES METADATA_HEADER NEW_OUTPUT".into(),
        );
    }
    let runtime = fs::read_to_string(&args[0])?;
    let lexicon_sha = sha256(&runtime);
    if lexicon_sha != args[1] {
        return Err("词典实际SHA不符".into());
    }
    let lexicon = Lexicon::from_tsv(&runtime)?;
    let source = fs::read_to_string(&args[2])?;
    let header = fs::read_to_string(&args[3])?;
    if source.len() > 16 * 1024 * 1024 || header.len() > 4096 {
        return Err("训练源或元数据超出研发预算".into());
    }
    let expected_source = header
        .lines()
        .find_map(|line| line.strip_prefix("source_sha256\t"));
    let source_sha = sha256(&source);
    if expected_source != Some(source_sha.as_str()) {
        return Err("实际训练原件与元数据SHA不符".into());
    }
    let sequences: Vec<Vec<String>> = source
        .lines()
        .map(|line| {
            if line.is_empty() {
                Vec::new()
            } else {
                line.split(' ').map(str::to_owned).collect()
            }
        })
        .collect();
    let model = LanguageModelSnapshot::train(&header, &lexicon, &lexicon_sha, &sequences)?;
    let artifact = model.to_tsv();
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[4])?;
    output.write_all(artifact.as_bytes())?;
    println!(
        "model_sha256={} sequences={} metadata={:?} production_approved=false formal_quality=null",
        sha256(&artifact),
        sequences.len(),
        model.metadata()
    );
    Ok(())
}
