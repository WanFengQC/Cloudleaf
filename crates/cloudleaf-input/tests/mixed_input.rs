use cloudleaf_input::{
    DoublePinyinLayout, InputConfig, InputEngine, PinyinScheme, SchemeConfig, SearchSnapshot,
};
use cloudleaf_search::{Config, Method, Ranker};
use cloudleaf_types::{ErrorCode, EventResult, InputEvent, Key, KeyDisposition};
use ring::digest::{SHA256, digest};
use std::sync::Arc;

fn engine_with(config: Config) -> InputEngine {
    let mut tsv = String::from(
        "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n",
    );
    for (id, pinyin, text) in [
        ("tomorrow", "ming tian", "明天"),
        ("what-time", "ji dian", "几点"),
        ("start", "kai shi", "开始"),
        ("please", "bang wo", "帮我"),
        ("have-a-look", "yi xia", "一下"),
        ("this", "zhe ge", "这个"),
        ("file", "wen jian", "文件"),
        ("input-method", "shu ru fa", "输入法"),
        ("system", "xi tong", "系统"),
        ("language", "yu yan", "语言"),
        ("develop", "kai fa", "开发"),
        ("need", "xu yao", "需要"),
        ("again", "chong xin", "重新"),
        ("check", "jian cha", "检查"),
        ("greeting", "ni hao", "你好"),
        ("you", "ni", "你"),
        ("good", "hao", "好"),
        ("same-pronunciation", "ni hao", "拟好"),
        ("me", "me", "么"),
        ("e", "e", "饿"),
        ("ting", "ting", "听"),
        ("che", "che", "车"),
        ("descending", "xia", "下"),
        ("spray", "pen", "喷"),
        ("favor", "en", "恩"),
        ("reason", "li", "理"),
        ("side", "ce", "侧"),
        ("plot", "tu", "图"),
        ("land", "di", "地"),
        ("oh", "o", "哦"),
        ("bright", "ming ming", "明明"),
        ("simple", "jian dan", "简单"),
        ("secure", "an", "安"),
        ("amend", "gai", "改"),
        ("lou", "lou", "楼"),
    ] {
        tsv.push_str(&format!(
            "{id}\t{pinyin}\t{text}\t\tunknown\t\tcloudleaf:test\t{id}\tMIT\n"
        ));
    }
    let sha: String = digest(&SHA256, tsv.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let snapshot = Arc::new(SearchSnapshot::from_verified_tsv(&tsv, &sha).unwrap());
    let mut engine = InputEngine::with_search(snapshot, config, InputConfig::default()).unwrap();
    engine.handle(InputEvent::Activate);
    engine
}

fn config() -> Config {
    Config {
        method: Method::PositionDag,
        ranker: Ranker::Structural,
        output_k: 8,
        ..Config::default()
    }
}

fn enabled() -> InputEngine {
    let mut engine = engine_with(config());
    assert!(!engine.mixed_input_enabled());
    engine.reconfigure_mixed_input(true).unwrap();
    engine
}

fn key(engine: &mut InputEngine, key: Key) -> EventResult {
    assert_eq!(
        engine.key_disposition(key),
        KeyDisposition::Handle,
        "{key:?}"
    );
    let result = engine.handle(InputEvent::Key(key));
    assert!(result.handled && result.error.is_none(), "{result:?}");
    result
}

fn type_raw(engine: &mut InputEngine, text: &str) {
    for character in text.chars() {
        assert!(
            key(engine, Key::Char(character)).commit.is_none(),
            "{character}"
        );
    }
    assert_eq!(engine.frame().preedit, text);
}

#[test]
fn frozen_cases_cover_mixed_words_preserved_literals_and_real_commit() {
    let fixtures = include_str!("fixtures/mixed-input.tsv");
    for record in fixtures
        .lines()
        .filter(|line| !line.starts_with('#'))
        .skip(1)
    {
        let fields: Vec<_> = record.split('\t').collect();
        let [id, raw, expected] = fields.as_slice() else {
            panic!("冻结样例必须含三列");
        };
        let mut engine = enabled();
        type_raw(&mut engine, raw);
        let frame = engine.frame();
        assert_eq!(&frame.candidates[0].text, expected, "{id}");
        let request = key(&mut engine, Key::Enter).commit.unwrap();
        assert_eq!(&request.text, expected, "{id}");
        assert_eq!(
            engine.key_disposition(Key::Char('A')),
            KeyDisposition::Blocked(ErrorCode::CommitPending)
        );
        assert!(
            engine
                .handle(InputEvent::CommitSucceeded { id: request.id })
                .frame
                .preedit
                .is_empty()
        );
    }
}

#[test]
fn english_codas_do_not_borrow_bytes_from_anchored_chinese() {
    let mut engine = enabled();
    for word in ["check", "back", "quick", "track", "block", "stock", "draft"] {
        let original = format!("bangwo{word}yixiazhegewenjian");
        type_raw(&mut engine, &original);
        let expected = format!("帮我{word}一下这个文件");
        let frame = engine.frame();
        assert_eq!(frame.candidates[0].text, expected, "{word}");
        let edges = &engine.search_diagnostic().unwrap().paths[0].edges;
        let english_end = 6 + word.len();
        let left_end = edges
            .iter()
            .map(|edge| edge.raw_end)
            .filter(|end| *end <= 6)
            .max();
        let right_start = edges
            .iter()
            .map(|edge| edge.raw_start)
            .filter(|start| *start >= 6)
            .min();
        assert_eq!(
            (left_end, right_start),
            (Some(6), Some(english_end)),
            "{word}"
        );
        let request = key(&mut engine, Key::Enter).commit.unwrap();
        assert_eq!(request.text, expected, "{word}");
        let evidence = engine.pending_selection().unwrap();
        assert_eq!(evidence.commit_id, request.id);
        assert_eq!(evidence.revision, frame.revision);
        assert_eq!(
            evidence
                .entries
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>(),
            ["please", "have-a-look", "this", "file"],
            "{word}"
        );
        engine.handle(InputEvent::CommitSucceeded { id: request.id });
    }
    type_raw(&mut engine, "nihaojiandangaixia");
    let frame = engine.frame();
    assert_eq!(frame.candidates[0].text, "你好简单改下");
    assert_ne!(frame.candidates[0].source, "cloudleaf:mixed");
}

#[test]
fn single_capitals_prevent_splitting_proper_names_without_masking_acronyms() {
    let mut engine = enabled();
    for (original, expected) in [
        ("OpenAI", "OpenAI"),
        ("VisualStudioxitong", "VisualStudio系统"),
        ("Alicemingming", "Alice明明"),
        ("SKUni", "SKU你"),
        ("zhegeSKUzhege", "这个SKU这个"),
    ] {
        type_raw(&mut engine, original);
        let frame = engine.frame();
        assert_eq!(frame.candidates[0].text, expected);
        let receipt = key(&mut engine, Key::Enter).commit.unwrap();
        assert_eq!(receipt.text, expected);
        if expected == original {
            assert!(engine.pending_selection().is_none());
        }
        engine.handle(InputEvent::CommitSucceeded { id: receipt.id });
    }
}

#[test]
fn adjacent_mixed_edit_invalidates_identity_and_saves_the_complete_original() {
    let mut engine = enabled();
    type_raw(&mut engine, "Rustyuyankaifa");
    let first = engine.frame();
    key(&mut engine, Key::Left);
    key(&mut engine, Key::Backspace);
    key(&mut engine, Key::Backspace);
    let edited = engine.frame();
    assert_eq!(edited.preedit, "Rustyuyankaa");
    assert_eq!(edited.cursor, 11);
    assert_eq!(
        engine
            .select_candidate(first.revision, &first.candidates[0].id)
            .error
            .unwrap()
            .code,
        ErrorCode::StaleCandidate
    );
    let commit = engine.confirm_literal(edited.revision, "").commit.unwrap();
    assert_eq!(commit.text, "Rustyuyankaa");
    assert!(engine.pending_selection().is_none());
    let failed = engine.handle(InputEvent::CommitFailed { id: commit.id });
    assert_eq!(failed.frame.preedit, "Rustyuyankaa");
    assert!(engine.reconfigure_mixed_input(false).is_err());
    let retry = engine.handle(InputEvent::RetryCommit).commit.unwrap();
    assert_eq!(retry.text, commit.text);
    assert_ne!(retry.id, commit.id);
    assert!(engine.pending_selection().is_none());
    assert!(
        engine
            .handle(InputEvent::CommitSucceeded { id: retry.id })
            .frame
            .preedit
            .is_empty()
    );
}

#[test]
fn digits_are_text_until_candidate_navigation_and_only_chinese_entries_can_learn() {
    let mut engine = enabled();
    type_raw(&mut engine, "zhegeSKUxuyao");
    let before = engine.frame();
    assert_eq!(before.candidates[0].text, "这个SKU需要");
    key(&mut engine, Key::Down);
    let receipt = key(&mut engine, Key::Char('1')).commit.unwrap();
    assert_eq!(receipt.text, "这个SKU需要");
    let evidence = engine.pending_selection().unwrap();
    assert_eq!(evidence.commit_id, receipt.id);
    assert_eq!(
        evidence
            .entries
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        ["this", "need"]
    );
    let edges = &engine.search_diagnostic().unwrap().paths[0].edges;
    assert_eq!((edges[0].raw_start, edges[0].raw_end), (0, 5));
    assert_eq!((edges[1].raw_start, edges[1].raw_end), (8, 13));
    assert!(
        engine
            .handle(InputEvent::CommitSucceeded { id: receipt.id })
            .frame
            .preedit
            .is_empty()
    );
    type_raw(&mut engine, "Windows");
    let commit = key(&mut engine, Key::Space).commit.unwrap();
    assert_eq!(commit.text, "Windows ");
    assert!(engine.pending_selection().is_none());
    engine.handle(InputEvent::CommitSucceeded { id: commit.id });
    type_raw(&mut engine, "11xitong");
    let rest = key(&mut engine, Key::Enter).commit.unwrap();
    assert_eq!(format!("{}{}", commit.text, rest.text), "Windows 11系统");
}

#[test]
fn punctuation_confirmation_and_explicit_preview_keep_distinct_literal_choices() {
    let mut engine = enabled();
    type_raw(&mut engine, "nihao");
    let with_mark = key(&mut engine, Key::Char(',')).commit.unwrap();
    assert_eq!(with_mark.text, "你好，");
    engine.handle(InputEvent::CommitSucceeded { id: with_mark.id });

    type_raw(&mut engine, "nihao.");
    let frame = engine.frame();
    assert_eq!(frame.candidates[0].text, "你好。");
    let original = frame
        .candidates
        .iter()
        .find(|candidate| candidate.text == "nihao.")
        .unwrap();
    let commit = engine
        .select_candidate(frame.revision, &original.id)
        .commit
        .unwrap();
    assert_eq!(commit.text, "nihao.");
    assert!(engine.pending_selection().is_none());
}

#[test]
fn maximum_ascii_composition_never_truncates_the_literal() {
    let mut engine = enabled();
    let original = format!("A_{}", "z".repeat(62));
    type_raw(&mut engine, &original);
    let rejected = engine.handle(InputEvent::Key(Key::Char('9')));
    assert_eq!(rejected.error.unwrap().code, ErrorCode::InputTooLong);
    assert_eq!(rejected.frame.preedit, original);
    assert_eq!(key(&mut engine, Key::Enter).commit.unwrap().text, original);
}

#[test]
fn disabling_preserves_current_text_and_pure_scheme_paths_still_work() {
    let mut engine = enabled();
    type_raw(&mut engine, "nihao12");
    key(&mut engine, Key::Left);
    let before = engine.frame();
    engine.reconfigure_mixed_input(false).unwrap();
    let after = engine.frame();
    assert_eq!(
        (after.preedit, after.cursor),
        (before.preedit, before.cursor)
    );
    assert!(after.revision > before.revision);
    key(&mut engine, Key::Escape);
    for scheme in [
        PinyinScheme::AbbreviatedPinyin,
        PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
    ] {
        engine
            .reconfigure_scheme(SchemeConfig {
                scheme,
                ..SchemeConfig::default()
            })
            .unwrap();
        let pinyin = if scheme == PinyinScheme::AbbreviatedPinyin {
            "nh"
        } else {
            "nihc"
        };
        type_raw(&mut engine, pinyin);
        let baseline = engine.frame().candidates[0].clone();
        engine.reconfigure_mixed_input(true).unwrap();
        assert_eq!(engine.frame().candidates[0], baseline);
        key(&mut engine, Key::Escape);
        engine.reconfigure_mixed_input(false).unwrap();
    }
}

#[test]
fn mixed_budget_failure_keeps_all_bytes_without_learning() {
    let mut engine = engine_with(Config {
        max_edges: 2,
        ..config()
    });
    engine.reconfigure_mixed_input(true).unwrap();
    let original = "nihaoSKUnihao";
    type_raw(&mut engine, original);
    assert!(
        engine
            .search_diagnostic()
            .unwrap()
            .statistics
            .budget
            .is_some()
    );
    assert_eq!(engine.frame().candidates[0].text, original);
    let commit = key(&mut engine, Key::Enter).commit.unwrap();
    assert_eq!(commit.text, original);
    assert!(engine.pending_selection().is_none());
}
