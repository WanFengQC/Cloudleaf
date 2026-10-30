use cloudleaf_translation::{
    Language, LocalDictionaryProvider, ProviderRegistry, ReadPolicy, ServiceLimits, TaskState,
    TranslationContext, TranslationDirection, TranslationKind, TranslationProviderConfig,
    TranslationRequest, TranslationService,
};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: dictionary_lookup CATALOG research|production zh-en|en-zh|auto-en|auto-zh word|phrase TEXT".into());
    }
    let policy = match args[1].as_str() {
        "research" => ReadPolicy::Research,
        "production" => ReadPolicy::Production,
        _ => return Err("invalid policy".into()),
    };
    let direction = match args[2].as_str() {
        "zh-en" => TranslationDirection {
            source: Language::Chinese,
            target: Language::English,
        },
        "en-zh" => TranslationDirection {
            source: Language::English,
            target: Language::Chinese,
        },
        "auto-en" => TranslationDirection {
            source: Language::Auto,
            target: Language::English,
        },
        "auto-zh" => TranslationDirection {
            source: Language::Auto,
            target: Language::Chinese,
        },
        _ => return Err("invalid direction".into()),
    };
    let kind = match args[3].as_str() {
        "word" => TranslationKind::Word,
        "phrase" => TranslationKind::Phrase,
        _ => return Err("invalid kind".into()),
    };
    let started = Instant::now();
    let provider =
        LocalDictionaryProvider::load("local-dictionary", Path::new(&args[0]), policy, None)?;
    let load_ns = started.elapsed().as_nanos();
    let stats = provider.stats();
    let catalog_sha256 = provider.catalog_sha256().to_owned();
    let mut registry = ProviderRegistry::new();
    registry.register(
        Arc::new(provider),
        TranslationProviderConfig {
            read_policy: policy,
            ..TranslationProviderConfig::default()
        },
    )?;
    let mut service = TranslationService::new(registry, ServiceLimits::default())?;
    let task = service.submit(
        "local-dictionary",
        TranslationRequest {
            text: args[4].clone(),
            direction,
            kind,
            context: TranslationContext {
                session: 1,
                epoch: 1,
                input_revision: 1,
            },
            send_authorized_to: None,
            use_cache: false,
        },
    )?;
    let snapshot = loop {
        let current = service.poll(task)?;
        if current.state.is_terminal() {
            break current;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    service.shutdown()?;
    if snapshot.state != TaskState::Succeeded {
        return Err(snapshot.error.ok_or("missing terminal error")?.into());
    }
    let result = snapshot.result.ok_or("missing translation result")?;
    println!(
        "{}",
        json!({"entries": stats.entries, "raw_glosses": stats.raw_glosses, "mappings": stats.mappings,
        "forward_keys": stats.forward_keys, "reverse_keys": stats.reverse_keys, "catalog_sha256": catalog_sha256,
        "load_ns": load_ns, "measurement_scope": "single offline load, not language accuracy or input latency",
        "quality": format!("{:?}", result.output.quality), "output_kind": format!("{:?}", result.output.output_kind),
        "resolved_source": format!("{:?}", result.output.resolved_source), "text": result.output.translated_text,
        "source_ids": result.output.source_ids, "human_identity_or_rights_proven": false,
        "production_policy_bound": stats.production_policy_bound})
    );
    Ok(())
}
