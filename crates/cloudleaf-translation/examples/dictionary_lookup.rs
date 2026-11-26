use cloudleaf_input::{InputConfig, InputEngine, SearchSnapshot};
use cloudleaf_translation::SemanticAnnotationIdentity;
use cloudleaf_translation::{
    Language, LocalDictionaryProvider, ProviderRegistry, ReadPolicy, SemanticDictionary,
    ServiceLimits, TaskState, TranslationContext, TranslationDirection, TranslationKind,
    TranslationProviderConfig, TranslationRequest, TranslationService,
};
use cloudleaf_types::{InputEvent, Key};
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
    if args.first().is_some_and(|arg| arg == "semantic") {
        return run_semantic(&args[1..]);
    }
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

fn run_semantic(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let [index, sha, mode, query @ ..] = args else {
        return Err("usage: dictionary_lookup semantic INDEX SHA zh|en|english-gloss|english-keyword TEXT | entry|phrases NAMESPACE ID | relations SENSE_ID".into());
    };
    let started = Instant::now();
    let lookup = SemanticDictionary::load(Path::new(index), sha, ReadPolicy::Research)?;
    let load_ns = started.elapsed().as_nanos();
    let observed = match (mode.as_str(), query) {
        ("zh" | "en", [text]) => json!(lookup.headword(mode, text)),
        ("english-gloss", [text]) => json!(lookup.english_gloss_exact(text)),
        ("english-keyword", [text]) => json!(lookup.english_gloss_keyword(text)),
        ("entry", [namespace, id]) => json!(lookup.entry(namespace, id)),
        ("phrases", [namespace, id]) => json!(lookup.phrases_for_entry(namespace, id)),
        ("relations", [id]) => json!(lookup.relations(id)),
        ("candidate-annotation", query) => candidate_annotation(&lookup, query)?,
        _ => return Err("invalid semantic query arguments".into()),
    };
    println!(
        "{}",
        json!({
            "scope": "research_only",
            "can_commit": false,
            "mode": mode,
            "source_evidence": lookup.sources(),
            "catalogs": lookup.catalogs(),
            "bundle_manifest_sha256": lookup.bundle_manifest_sha256(),
            "lookup_index_sha256": lookup.lookup_index_sha256(),
            "content_sha256": lookup.content_sha256(),
            "load_ns": load_ns,
            "result": observed,
        })
    );
    Ok(())
}

fn candidate_annotation(
    lookup: &SemanticDictionary,
    query: &[String],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let [runtime, runtime_sha, namespace, keys, selected] = query else {
        return Err("usage: dictionary_lookup semantic INDEX SHA candidate-annotation RUNTIME RUNTIME_SHA NAMESPACE PINYIN CANDIDATE".into());
    };
    if keys.is_empty() || keys.len() > 64 || !keys.bytes().all(|key| key.is_ascii_lowercase()) {
        return Err("invalid research pinyin input".into());
    }
    let resource = std::fs::read_to_string(runtime)?;
    let snapshot = Arc::new(SearchSnapshot::from_verified_tsv(&resource, runtime_sha)?);
    let mut engine =
        InputEngine::with_search(snapshot, Default::default(), InputConfig::default())?;
    engine.handle(InputEvent::Activate);
    for key in keys.chars() {
        if let Some(error) = engine.handle(InputEvent::Key(Key::Char(key))).error {
            return Err(error.into());
        }
    }
    let frame = engine.frame();
    let candidate = frame
        .candidates
        .iter()
        .find(|candidate| candidate.text == *selected)
        .ok_or("candidate is not in the current actual frame")?;
    let path = engine
        .search_diagnostic()
        .and_then(|diagnostic| {
            diagnostic
                .paths
                .iter()
                .find(|path| path.candidate_id == candidate.id)
        })
        .ok_or("candidate has no lexicon path")?;
    let identity = SemanticAnnotationIdentity {
        session: 1,
        input_revision: frame.revision,
        candidate_id: candidate.id.clone(),
        dictionary_runtime_sha256: runtime_sha.clone(),
        bundle_manifest_sha256: lookup.bundle_manifest_sha256().to_owned(),
        lookup_index_sha256: lookup.lookup_index_sha256().to_owned(),
    };
    let updates = path
        .edges
        .iter()
        .map(|edge| {
            lookup
                .prepare_annotation(identity.clone(), namespace, &edge.entry_id)
                .map(|result| (edge.entry_id.clone(), result))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let frame_unchanged = engine.frame() == frame;
    let preedit_unchanged = engine.frame().preedit == frame.preedit;
    let pending_learning = engine.pending_selection().is_some();
    let current = updates
        .iter()
        .all(|(_, update)| update.is_current(&identity));
    let edited = engine.handle(InputEvent::Key(Key::Backspace));
    let after_edit = SemanticAnnotationIdentity {
        input_revision: edited.frame.revision,
        ..identity
    };
    let old_result_rejected = updates
        .iter()
        .all(|(_, update)| !update.is_current(&after_edit));
    Ok(json!({
        "candidate":candidate.text,
        "candidate_id":candidate.id,
        "research_annotations":updates,
        "frame_unchanged":frame_unchanged,
        "preedit_unchanged":preedit_unchanged,
        "pending_learning":pending_learning,
        "current":current,
        "old_result_rejected_after_real_edit":old_result_rejected,
    }))
}
