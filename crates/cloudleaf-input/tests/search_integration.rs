use cloudleaf_input::{InputConfig, InputEngine, SearchSnapshot};
use cloudleaf_search::{Config, Method, Ranker};
use cloudleaf_types::{ErrorCode, InputEvent, InputState, Key};
use ring::digest::{SHA256, digest};
use std::sync::Arc;

const WORDS: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\nwo\two\t我\t\tunknown\t\tcloudleaf:fixture\two\tMIT\nzhengzai\tzheng zai\t正在\t\tunknown\t\tcloudleaf:fixture\tzhengzai\tMIT\nkaifa\tkai fa\t开发\t\tunknown\t\tcloudleaf:fixture\tkaifa\tMIT\nyi\tyi\t一\t\tunknown\t\tcloudleaf:fixture\tyi\tMIT\nkuan\tkuan\t款\t\tunknown\t\tcloudleaf:fixture\tkuan\tMIT\nzhongwen\tzhong wen\t中文\t\tunknown\t\tcloudleaf:fixture\tzhongwen\tMIT\nshurufa\tshu ru fa\t输入法\t\tunknown\t\tcloudleaf:fixture\tshurufa\tMIT\nxi\txi\t西\t\tunknown\t\tcloudleaf:fixture\txi\tMIT\nan\tan\t安\t\tunknown\t\tcloudleaf:fixture\tan\tMIT\nxian\txian\t先\t\tunknown\t\tcloudleaf:fixture\txian\tMIT\n";

fn snapshot(text: &str) -> Arc<SearchSnapshot> {
    let hash: String = digest(&SHA256, text.as_bytes()).as_ref().iter().map(|b| format!("{b:02x}")).collect();
    Arc::new(SearchSnapshot::from_verified_tsv(text, &hash).unwrap())
}

fn config() -> Config {
    Config { method: Method::PositionDag, ranker: Ranker::Structural, output_k: 5, ..Config::default() }
}

fn engine() -> InputEngine {
    let mut value = InputEngine::with_search(snapshot(WORDS), config(), InputConfig::default()).unwrap();
    value.handle(InputEvent::Activate);
    value
}

fn input(engine: &mut InputEngine, value: &str) {
    for ch in value.chars() {
        assert!(engine.handle(InputEvent::Key(Key::Char(ch))).error.is_none());
    }
}

#[test]
fn full_sentence_uses_existing_search_and_preserves_unknown_frequency() {
    let mut engine = engine();
    input(&mut engine, "wozhengzaikaifayikuanzhongwenshurufa");
    let frame = engine.frame();
    assert_eq!(frame.candidates[0].text, "我正在开发一款中文输入法");
    assert_eq!(frame.candidates[0].frequency, None);
    let diagnostic = engine.search_diagnostic().unwrap();
    assert_eq!(diagnostic.revision, frame.revision);
    assert!(diagnostic.guaranteed);
    assert_eq!(diagnostic.paths[0].edges.first().unwrap().raw_start, 0);
    assert_eq!(diagnostic.paths[0].edges.last().unwrap().raw_end, frame.preedit.len());
    let commit = engine.handle(InputEvent::Key(Key::Space)).commit.unwrap();
    assert_eq!(commit.text, frame.candidates[0].text);
    let pending = engine.handle(InputEvent::Key(Key::Enter));
    assert!(pending.commit.is_none());
    assert_eq!(pending.error.unwrap().code, ErrorCode::CommitPending);
    engine.handle(InputEvent::CommitSucceeded { id: commit.id });
    assert_eq!(engine.frame().state, InputState::Idle);
}

#[test]
fn revision_snapshot_and_path_ids_reject_stale_candidates() {
    let mut engine = engine();
    input(&mut engine, "xian");
    let previous = engine.frame();
    assert!(previous.candidates.iter().any(|c| c.text == "西安"));
    engine.handle(InputEvent::Key(Key::Left));
    assert_eq!(engine.select_candidate(previous.revision, &previous.candidates[0].id).error.unwrap().code, ErrorCode::StaleCandidate);
    let before = engine.frame();
    let next = snapshot(&WORDS.replace("cloudleaf:fixture", "cloudleaf:version2"));
    engine.replace_search_snapshot(next).unwrap();
    let after = engine.frame();
    assert_eq!(after.preedit, before.preedit);
    assert_ne!(after.candidates[0].id, before.candidates[0].id);
    assert!(after.revision > before.revision);
    let commit = engine.handle(InputEvent::Key(Key::Enter)).commit.unwrap();
    assert!(engine.replace_search_snapshot(snapshot(WORDS)).is_err());
    engine.handle(InputEvent::CommitFailed { id: commit.id });
    assert!(engine.replace_search_snapshot(snapshot(WORDS)).is_err());
}

#[test]
fn budget_and_unknown_input_keep_complete_original_fallback() {
    let mut cfg = config(); cfg.max_edges = 1;
    let mut engine = InputEngine::with_search(snapshot(WORDS), cfg, InputConfig::default()).unwrap();
    engine.handle(InputEvent::Activate);
    input(&mut engine, "xian");
    assert!(engine.frame().candidates.is_empty());
    assert!(engine.search_diagnostic().unwrap().statistics.budget.is_some());
    assert_eq!(engine.handle(InputEvent::Key(Key::Enter)).commit.unwrap().text, "xian");
    let mut engine = self::engine();
    input(&mut engine, "qz");
    assert!(engine.frame().candidates.is_empty());
    assert_eq!(engine.handle(InputEvent::Key(Key::Space)).commit.unwrap().text, "qz");
    assert!(SearchSnapshot::from_verified_tsv(WORDS, &"0".repeat(64)).is_err());
}

#[test]
fn projection_and_executor_keep_same_ids_despite_prior_revision_history() {
    let snapshot = snapshot(WORDS);
    let mut first = InputEngine::with_search(snapshot.clone(), config(), InputConfig::default()).unwrap();
    let mut second = InputEngine::with_search(snapshot, config(), InputConfig::default()).unwrap();
    for engine in [&mut first, &mut second] { engine.handle(InputEvent::Activate); }
    input(&mut first, "xi"); first.handle(InputEvent::Key(Key::Escape));
    input(&mut first, "xian"); input(&mut second, "xian");
    assert_ne!(first.frame().revision, second.frame().revision);
    assert_eq!(first.frame().candidates, second.frame().candidates);
}
