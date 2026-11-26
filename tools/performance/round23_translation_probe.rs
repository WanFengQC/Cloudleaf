//! 本机研究探针。只通过现有私有管道调用显式固定的离线 worker。
use cloudleaf_translation_integrity::{
    Direction as IntegrityDirection, Glossary, check_translation,
};
use cloudleaf_translation_ipc::{
    Direction, ResourceThreads, SourceIdentity, WorkerClient, WorkerConfig, WorkerEvent,
};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

fn evaluate(
    mode: &str,
    id: &str,
    topic: &str,
    direction: IntegrityDirection,
    source: &str,
    output: &str,
    glossary: &Glossary,
) {
    let report = check_translation(source, output, direction, glossary);
    let findings = report
        .findings
        .iter()
        .map(|finding| format!("{:?}:{:?}", finding.kind, finding.status))
        .collect::<Vec<_>>()
        .join(",");
    assert!(!output.contains(['\n', '\r', '\t']));
    println!(
        "OBSERVATION\t{mode}\t{id}\t{topic}\t{}\t{findings}\t{output}",
        report.requires_acknowledgement()
    );
}

fn run(cases: &str, glossary: &Glossary, client: &mut WorkerClient) -> Result<(), Box<dyn Error>> {
    for (index, row) in cases.lines().skip(1).enumerate() {
        let fields = row.split('\t').collect::<Vec<_>>();
        let [mode, id, direction, topic, source, intentional_output] = fields.as_slice() else {
            return Err(format!("样本行 {} 列格式错误", index + 2).into());
        };
        let (wire_direction, direction) = match *direction {
            "zh-en" => (Direction::ZhEn, IntegrityDirection::ChineseToEnglish),
            "en-zh" => (Direction::EnZh, IntegrityDirection::EnglishToChinese),
            _ => return Err("未知研究样本方向".into()),
        };
        match *mode {
            "control" if *intentional_output != "-" => {
                evaluate(
                    mode,
                    id,
                    topic,
                    direction,
                    source,
                    intentional_output,
                    glossary,
                );
            }
            "real" if *intentional_output == "-" => {
                let key = client.submit(
                    SourceIdentity {
                        session: 1,
                        epoch: 1,
                        input_revision: index as u64 + 1,
                    },
                    wire_direction,
                    (*source).to_owned(),
                    Duration::from_secs(45),
                )?;
                let deadline = Instant::now() + Duration::from_secs(50);
                loop {
                    match client.try_event() {
                        Some(WorkerEvent::Completed {
                            key: actual,
                            output,
                            ..
                        }) if actual == key => {
                            let translation = &output.raw_translation;
                            println!(
                                "MODEL\t{id}\t{}\t{}\t{}\t{}\t{}",
                                output.model_identity,
                                output.catalog_sha256,
                                output.metrics.inference_ns,
                                output.metrics.load_ns,
                                output.metrics.compute_threads
                            );
                            evaluate(mode, id, topic, direction, source, translation, glossary);
                            break;
                        }
                        Some(WorkerEvent::Failed { key: actual, error }) if actual == key => {
                            println!("FAILED\t{id}\t{error:?}");
                            break;
                        }
                        Some(WorkerEvent::Cancelled { key: actual }) if actual == key => {
                            println!("FAILED\t{id}\tCancelled");
                            break;
                        }
                        _ if Instant::now() >= deadline => return Err("IPC 等待超限".into()),
                        _ => thread::sleep(Duration::from_millis(5)),
                    }
                }
            }
            _ => return Err("样本模式与人工构造译文列不一致".into()),
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let [worker, catalog, catalog_sha, cases, glossary] = args.as_slice() else {
        return Err(
            "用法：round23_translation_probe <worker> <catalog> <catalog-sha> <cases> <glossary>"
                .into(),
        );
    };
    let worker = Path::new(worker).canonicalize()?;
    let config = WorkerConfig::sibling(
        PathBuf::from(catalog),
        catalog_sha.to_string_lossy().into_owned(),
        ResourceThreads::One,
    )?
    .for_host_executable(worker)?;
    let cases = std::fs::read_to_string(cases)?;
    let glossary = Glossary::parse_tsv(&std::fs::read_to_string(glossary)?)?;
    let mut client = WorkerClient::new(config);
    let result = run(&cases, &glossary, &mut client);
    client.close();
    if !client.wait_for_shutdown(Duration::from_secs(5)) {
        return Err("未确认离线 worker 回收".into());
    }
    result
}
