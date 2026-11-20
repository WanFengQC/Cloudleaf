use cloudleaf_input::{
    CorrectionConfig, DoublePinyinLayout, FuzzyDirection, FuzzyPair, FuzzyRule, InputConfig,
    InputEngine, LanguageSnapshot, PinyinScheme, SchemeConfig, SearchSnapshot,
};
use cloudleaf_search::{Config, Method, Ranker};
use cloudleaf_types::{InputEvent, Key};
use ring::digest::{SHA256, digest};
use std::{collections::BTreeMap, sync::Arc};

const HEADER: &str = "# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";

fn hash(text: &str) -> String {
    digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn fixture() -> (Arc<SearchSnapshot>, Arc<LanguageSnapshot>, Config) {
    let text = format!(
        "{HEADER}a\tni\t你\t\tunknown\t\tfixture\ta\tMIT\nb\thao\t好\t\tunknown\t\tfixture\tb\tMIT\nc\tni hao\t你好\t\tunknown\t\tfixture\tc\tMIT\nd\tli\t里\t\tunknown\t\tfixture\td\tMIT\n"
    );
    let base = Arc::new(SearchSnapshot::from_verified_tsv(&text, &hash(&text)).unwrap());
    let text = format!(
        "# cloudleaf-language-v1\nlexicon_sha256\t{}\nsource_sha256\t{}\nsource_id\tmechanical\nlicense_id\tMIT\ntokenization\tstable-ids\nusage\tmechanical\nkind\tlexical\ntraining_split\ttrain\nprevious\tnext\tcount\n<lexical>\ta\t10\n<lexical>\tb\t20\n<lexical>\tc\t30\n",
        base.version(),
        "a".repeat(64)
    );
    let model = Arc::new(LanguageSnapshot::from_verified_tsv(&text, &hash(&text), &base).unwrap());
    let config = Config {
        method: Method::PositionDag,
        ranker: Ranker::Lexical,
        output_k: 10,
        ..Config::default()
    };
    (base, model, config)
}

fn pair() -> (InputEngine, InputEngine) {
    let (base, model, config) = fixture();
    let mut on = InputEngine::with_language(
        base.clone(),
        config.clone(),
        model.clone(),
        None,
        InputConfig::default(),
    )
    .unwrap();
    on.set_search_cache_enabled(true);
    let off =
        InputEngine::with_language(base, config, model, None, InputConfig::default()).unwrap();
    (on, off)
}

fn event(on: &mut InputEngine, off: &mut InputEngine, input: InputEvent) {
    assert_eq!(on.handle(input), off.handle(input));
    assert_eq!(on.search_diagnostic(), off.search_diagnostic());
}

#[test]
fn cache_enabled_and_disabled_preserve_all_frames_paths_features_and_revisions() {
    let (mut on, mut off) = pair();
    event(&mut on, &mut off, InputEvent::Activate);
    let schemes = [
        SchemeConfig::default(),
        SchemeConfig {
            scheme: PinyinScheme::AbbreviatedPinyin,
            ..Default::default()
        },
        SchemeConfig {
            scheme: PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
            ..Default::default()
        },
        SchemeConfig {
            fuzzy: vec![FuzzyRule {
                pair: FuzzyPair::NL,
                direction: FuzzyDirection::Both,
            }],
            ..Default::default()
        },
        SchemeConfig {
            correction: Some(CorrectionConfig::default()),
            ..Default::default()
        },
    ];
    for scheme in schemes {
        on.reconfigure_scheme(scheme.clone()).unwrap();
        off.reconfigure_scheme(scheme).unwrap();
        assert_eq!(on.frame(), off.frame());
        for ch in "nihao".chars() {
            event(&mut on, &mut off, InputEvent::Key(Key::Char(ch)));
        }
        for key in [
            Key::Backspace,
            Key::Char('o'),
            Key::Left,
            Key::Left,
            Key::Char('i'),
            Key::Backspace,
            Key::Right,
            Key::PageDown,
        ] {
            event(&mut on, &mut off, InputEvent::Key(key));
        }
        let before = on.frame();
        event(&mut on, &mut off, InputEvent::Key(Key::Char('n')));
        if let Some(candidate) = before.candidates.first() {
            let a = on.select_candidate(before.revision, &candidate.id);
            let b = off.select_candidate(before.revision, &candidate.id);
            assert_eq!(a, b);
            assert!(a.error.is_some());
        }
        event(&mut on, &mut off, InputEvent::Key(Key::Escape));
        assert_eq!(on.search_cache_statistics().unwrap().resident_entries, 0);
    }
    assert!(on.search_cache_statistics().unwrap().hits > 0);
    assert_eq!(off.search_cache_statistics().unwrap().hits, 0);
    assert!(on.search_cache_capacity_bytes() < 100 * 1024);
}

#[test]
fn user_revision_preference_model_and_resource_changes_cannot_reuse_old_candidates() {
    let (mut on, mut off) = pair();
    event(&mut on, &mut off, InputEvent::Activate);
    let scheme = SchemeConfig {
        scheme: PinyinScheme::AbbreviatedPinyin,
        ..Default::default()
    };
    on.reconfigure_scheme(scheme.clone()).unwrap();
    off.reconfigure_scheme(scheme).unwrap();
    for ch in "nh".chars() {
        event(&mut on, &mut off, InputEvent::Key(Key::Char(ch)));
    }
    let old = on.frame();
    let (base, model) = on.search_resources().unwrap();
    let model = model.unwrap();
    let users = format!(
        "{HEADER}user:1\tni hao\t拟好\t\tunknown\t\tuser\tuser:1\tLicenseRef-User-Private\n"
    );
    let mut target = base
        .with_user_layer(&users, &BTreeMap::from([("user:1".into(), 4)]))
        .unwrap();
    target.bind_user_state(b"manual-fixture-revision1").unwrap();
    let mut model_target = model.for_user_layer(&base, &target).unwrap();
    model_target.bind_user_state(&target).unwrap();
    let target = Arc::new(target);
    let model_target = Arc::new(model_target);
    on.replace_search_resources(target.clone(), Some(model_target.clone()))
        .unwrap();
    off.replace_search_resources(target, Some(model_target))
        .unwrap();
    assert_eq!(on.frame(), off.frame());
    assert_eq!(on.search_diagnostic(), off.search_diagnostic());
    let candidate = &old.candidates[0];
    assert!(
        on.select_candidate(old.revision, &candidate.id)
            .error
            .is_some()
    );
    assert!(
        off.select_candidate(old.revision, &candidate.id)
            .error
            .is_some()
    );
    assert_eq!(off.search_cache_statistics().unwrap().hits, 0);
    on.replace_search_resources(base.clone(), Some(model.clone()))
        .unwrap();
    off.replace_search_resources(base, Some(model)).unwrap();
    assert_eq!(on.frame(), off.frame());
    assert_eq!(on.search_diagnostic(), off.search_diagnostic());
    assert!(on.frame().candidates.iter().all(|c| c.text != "拟好"));
    event(&mut on, &mut off, InputEvent::Key(Key::Space));
    event(&mut on, &mut off, InputEvent::Key(Key::Escape));
    assert_eq!(on.search_cache_statistics().unwrap().resident_entries, 0);
}
