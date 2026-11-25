use cloudleaf_offline::{MarianProvider, MarianTokenizer};
use cloudleaf_translation::*;
use std::io::BufRead;
use std::path::Path;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|value| value == "tokenize") && args.len() == 5 {
        let prefix = (args[4] == "en-zh").then_some(">>cmn_Hans<<");
        let tokenizer = MarianTokenizer::from_files(
            Path::new(&args[1]),
            Path::new(&args[2]),
            Path::new(&args[3]),
            prefix,
        )?;
        for line in std::io::stdin().lock().lines() {
            let text: String = serde_json::from_str(&line?)?;
            let encoded = tokenizer.encode(&text)?;
            println!(
                "{}",
                serde_json::json!({"pieces":encoded.pieces,"ids":encoded.ids,"decoded_using_target":tokenizer.decode(&encoded.ids)?})
            );
        }
        return Ok(());
    }
    if args.len() != 1 {
        return Err("usage: offline_translate CATALOG < samples.jsonl; or tokenize SOURCE_SPM TARGET_SPM VOCAB DIRECTION".into());
    }
    let start = Instant::now();
    let provider = MarianProvider::load(Path::new(&args[0]))?;
    println!(
        "{}",
        serde_json::json!({"event":"loaded","load_ms":start.elapsed().as_millis(),"version":provider.identity().version})
    );
    for (index, line) in std::io::stdin().lock().lines().enumerate() {
        let value: serde_json::Value = serde_json::from_str(&line?)?;
        let text = value["text"].as_str().ok_or("missing text")?;
        let direction = match value["direction"].as_str() {
            Some("zh-en") => TranslationDirection {
                source: Language::Chinese,
                target: Language::English,
            },
            Some("en-zh") => TranslationDirection {
                source: Language::English,
                target: Language::Chinese,
            },
            _ => return Err("unknown direction".into()),
        };
        let cancellation = TranslationCancellation::new(Duration::from_secs(
            value["timeout_seconds"].as_u64().unwrap_or(120),
        ));
        if value["cancel"].as_bool().unwrap_or(false) {
            cancellation.cancel();
        }
        let request = TranslationRequest {
            text: text.into(),
            direction,
            kind: TranslationKind::Sentence,
            context: TranslationContext {
                session: 1,
                epoch: 1,
                input_revision: index as u64 + 1,
            },
            send_authorized_to: None,
            use_cache: false,
        };
        let start = Instant::now();
        let record = match provider.translate_observed(&request, &cancellation) {
            Ok((output, stats)) => {
                serde_json::json!({"index":index,"input":text,"direction":value["direction"],"output":output.translated_text,"quality":"Generated","stats":stats})
            }
            Err(error) => {
                serde_json::json!({"index":index,"input":text,"direction":value["direction"],"error":error.to_string(),"elapsed_ms":start.elapsed().as_millis()})
            }
        };
        println!("{record}");
    }
    Ok(())
}
