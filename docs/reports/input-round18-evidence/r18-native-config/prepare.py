from pathlib import Path
import difflib, hashlib, json
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=Path(__file__).resolve().parent
names=['adapters/windows/isolated_input.rs','adapters/windows/input_review.rs']
original={name:((out/'original'/name) if (out/'original'/name).exists() else (repo/name)).read_text(encoding='utf-8') for name in names}
header='# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n'
rows=[('nihao','ni hao','你好'),('nonghu','nong hu','农户'),('zhongguo','zhong guo','中国')]
rows.extend((f'ni{i}','ni',text) for i,text in enumerate(['你','尼','泥','拟','逆','匿','妮','霓','腻','溺','呢','旎']))
tsv=header+''.join(f'{id}\t{pinyin}\t{text}\t\tunknown\t\tmechanical\t{id}\tMIT\n'for id,pinyin,text in rows)
sha=hashlib.sha256(tsv.encode()).hexdigest()
rust_tsv=json.dumps(tsv,ensure_ascii=False)
tests='''
    fn scheme_local() -> LocalHost {
        let snapshot = Arc::new(SearchSnapshot::from_verified_tsv(TSV, SHA).unwrap());
        let config = cloudleaf_search::Config {
            ranker: cloudleaf_search::Ranker::Structural,
            method: cloudleaf_search::Method::PositionDag,
            output_k: 10, max_input_bytes: 64, max_letters: 64,
            ..Default::default()
        };
        LocalHost::new(Host::with_search(snapshot, config, InputConfig::default()).unwrap()).unwrap()
    }

    #[test]
    fn scheme_switch_preserves_raw_cursor_and_rejects_the_old_candidate() {
        let local = scheme_local();
        type_text(&local, "nihao");
        local.process(InputEvent::Key(Key::Left), None, &|_| CommitOutcome::Applied).unwrap();
        let before = local.host.engine.borrow().frame();
        let config = cloudleaf_input::SchemeConfig {
            scheme: cloudleaf_input::PinyinScheme::AbbreviatedPinyin,
            ..Default::default()
        };
        let after = local.switch_scheme(config.clone()).unwrap();
        assert_eq!(after.preedit, before.preedit);
        assert_eq!(after.cursor, before.cursor);
        assert!(after.revision > before.revision);
        assert_eq!(local.host.engine.borrow().pinyin_scheme(), Some(&config));
        assert_eq!(local.host.projection.borrow().pinyin_scheme(), Some(&config));
        assert!(local.process(InputEvent::Key(Key::Space), Some((before.revision, before.candidates[0].id.clone())), &|_| panic!("旧候选不能写文档")).is_err());
        assert!(local.document.borrow().is_empty());
    }

    #[test]
    fn failed_or_unknown_commit_blocks_settings_without_partial_change() {
        for outcome in [CommitOutcome::NotApplied, CommitOutcome::Unknown] {
            let local = scheme_local();
            type_text(&local, "nihao");
            local.process(InputEvent::Key(Key::Space), None, &|_| outcome).unwrap();
            let actual = local.host.engine.borrow().frame();
            let projected = local.host.projection.borrow().frame();
            let config = cloudleaf_input::SchemeConfig {
                scheme: cloudleaf_input::PinyinScheme::AbbreviatedPinyin,
                ..Default::default()
            };
            assert!(local.switch_scheme(config).is_err());
            assert_eq!(local.host.engine.borrow().frame(), actual);
            assert_eq!(local.host.projection.borrow().frame(), projected);
            assert_eq!(local.host.engine.borrow().pinyin_scheme(), Some(&cloudleaf_input::SchemeConfig::default()));
            assert!(local.document.borrow().is_empty());
        }
    }

    #[test]
    fn projection_preflight_failure_never_changes_the_first_engine() {
        let local = scheme_local();
        type_text(&local, "nihao");
        local.host.projection.borrow_mut().handle(InputEvent::Key(Key::Space));
        let actual = local.host.engine.borrow().frame();
        let projected = local.host.projection.borrow().frame();
        let config = cloudleaf_input::SchemeConfig {
            scheme: cloudleaf_input::PinyinScheme::AbbreviatedPinyin,
            ..Default::default()
        };
        assert!(local.switch_scheme(config).is_err());
        assert_eq!(local.host.engine.borrow().frame(), actual);
        assert_eq!(local.host.projection.borrow().frame(), projected);
    }

    #[test]
    fn configured_host_keeps_page_selection_in_the_real_input_core() {
        let local = scheme_local();
        type_text(&local, "ni");
        let before = local.host.engine.borrow().frame();
        assert_eq!(before.candidates.len(), 10);
        local.process(InputEvent::Key(Key::PageDown), None, &|_| CommitOutcome::Applied).unwrap();
        let paged = local.host.engine.borrow().frame();
        assert_eq!(paged.page_start, 5);
        let expected = paged.candidates[6].text.clone();
        local.process(InputEvent::Key(Key::Char('2')), None, &|_| CommitOutcome::Applied).unwrap();
        assert_eq!(local.document.borrow().as_str(), expected);
        assert_eq!(local.host.engine.borrow().frame().preedit, "");
    }
'''.replace('TSV',rust_tsv).replace('SHA','"'+sha+'"')
test_only=dict(original)
source=original[names[0]]
where=source.rfind('\n}')
test_only[names[0]]=source[:where]+'\n'+tests+source[where:]
implementation=dict(test_only)
source=implementation[names[0]]
needle='    fn retain(&self, material: &str) -> Result<()> {'
replacement='''    pub fn switch_scheme(&self, config: cloudleaf_input::SchemeConfig) -> Result<InputFrame> {
        self.host.check()?;
        if self.host.quarantined.get() {
            return Err(Error::new(E_FAIL, "提交结果未知，不能切换输入方案"));
        }
        let mut actual = self.host.engine.borrow_mut();
        let mut projected = self.host.projection.borrow_mut();
        // 两份只读预检全部成功后才替换，避免第二引擎拒绝时留下半次设置变化。
        actual.validate_scheme_config(&config).map_err(|_| Error::new(E_FAIL, "当前事务或方案配置不允许切换"))?;
        projected.validate_scheme_config(&config).map_err(|_| Error::new(E_FAIL, "投影事务或方案配置不允许切换"))?;
        actual.reconfigure_scheme(config.clone()).map_err(|_| Error::new(E_FAIL, "输入方案应用失败"))?;
        if projected.reconfigure_scheme(config).is_err() {
            self.host.quarantined.set(true);
            return Err(Error::new(E_FAIL, "双引擎设置未一致，已隔离且未写文档"));
        }
        let frame = actual.frame();
        let mut projection_frame = projected.frame();
        projection_frame.revision = frame.revision;
        if frame != projection_frame {
            self.host.quarantined.set(true);
            return Err(Error::new(E_FAIL, "双引擎方案候选不一致，已隔离且未写文档"));
        }
        Ok(frame)
    }

'''+needle
assert needle in source
source=source.replace(needle,replacement,1)
needle='    fn update(self: &Rc<Self>, frame: InputFrame) -> Result<()> {'
replacement='''    fn settings_key(self: &Rc<Self>, key: u16) -> Result<()> {
        use cloudleaf_input::{CorrectionConfig, DoublePinyinLayout, FuzzyDirection, FuzzyPair, FuzzyRule, PinyinScheme};
        let mut config = self.local.host.engine.borrow().pinyin_scheme().cloned()
            .ok_or_else(|| Error::new(E_FAIL, "此后端不支持方案设置"))?;
        if key == VK_F6.0 {
            config.scheme = match config.scheme {
                PinyinScheme::FullPinyin => PinyinScheme::AbbreviatedPinyin,
                PinyinScheme::AbbreviatedPinyin => PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
                PinyinScheme::DoublePinyin(_) => PinyinScheme::FullPinyin,
            };
            if matches!(config.scheme, PinyinScheme::DoublePinyin(_)) { config.correction = None; }
        } else if key == VK_F7.0 {
            if config.fuzzy.iter().any(|rule| rule.pair == FuzzyPair::NL) {
                config.fuzzy.retain(|rule| rule.pair != FuzzyPair::NL);
            } else {
                config.fuzzy.push(FuzzyRule { pair: FuzzyPair::NL, direction: FuzzyDirection::Both });
            }
        } else if key == VK_F8.0 {
            if matches!(config.scheme, PinyinScheme::DoublePinyin(_)) {
                return Err(Error::new(E_FAIL, "小鹤双拼尚不支持拼写纠错，设置未改变"));
            }
            config.correction = if config.correction.is_some() { None } else { Some(CorrectionConfig::default()) };
        }
        let frame = self.local.switch_scheme(config)?;
        self.update(frame)
    }

'''+needle
assert needle in source
source=source.replace(needle,replacement,1)
needle='''        let message = format!(
            "拼音：{}  修订：{}  {:?}\\r\\n{}{}",'''
replacement='''        let config = self.local.host.engine.borrow().pinyin_scheme().cloned().unwrap_or_default();
        let name = match config.scheme {
            cloudleaf_input::PinyinScheme::FullPinyin => "全拼",
            cloudleaf_input::PinyinScheme::AbbreviatedPinyin => "全拼/简拼混合",
            cloudleaf_input::PinyinScheme::DoublePinyin(_) => "小鹤双拼",
        };
        let message = format!(
            "方案：{name}  n/l模糊：{}  纠错：{}\\r\\n拼音：{}  修订：{}  {:?}\\r\\n{}{}",
            if config.fuzzy.is_empty() { "关" } else { "开" },
            if config.correction.is_none() { "关" } else { "开" },'''
assert needle in source
source=source.replace(needle,replacement,1)
source=source.replace('w!("直接键入拼音；Space/数字选词，方向键/翻页，Enter确认，未识别拼音原文回退，Esc取消。")','w!("F6方案 / F7 n↔l / F8纠错；Space/数字选词，方向键/翻页，Enter确认/原文，Esc取消。")')
source=source.replace('12,44,720,56,','12,44,720,76,').replace('x,104,width,30,','x,128,width,30,').replace('rect.top + 142','rect.top + 170')
needle='''                WM_KEYDOWN => {
                    let key = match wparam.0 as u16 {'''
replacement='''                WM_KEYDOWN => {
                    let virtual_key = wparam.0 as u16;
                    if matches!(virtual_key, x if x == VK_F6.0 || x == VK_F7.0 || x == VK_F8.0)
                        && GetKeyState(i32::from(VK_CONTROL.0)) >= 0
                        && GetKeyState(i32::from(VK_MENU.0)) >= 0
                        && GetKeyState(i32::from(VK_SHIFT.0)) >= 0 {
                        if let Err(error) = view.settings_key(virtual_key) {
                            let message = wide(&format!("设置未改变：{}", error.message()));
                            SetWindowTextW(view.status.get(), PCWSTR(message.as_ptr()))?;
                        }
                        return Ok(LRESULT(0));
                    }
                    let key = match virtual_key {'''
assert needle in source
source=source.replace(needle,replacement,1)
source=source.replace('super::input_review::load_host(runtime, runtime_sha, model)?;','super::input_review::load_host_k(runtime, runtime_sha, model, 10)?;',1)
implementation[names[0]]=source
source=implementation[names[1]]
needle='''    model: Option<(&str, &str)>,
) -> Result<(Shared, String, usize)> {
    let text ='''
replacement='''    model: Option<(&str, &str)>,
) -> Result<(Shared, String, usize)> {
    load_host_k(runtime, expected_sha256, model, 5)
}

pub(super) fn load_host_k(
    runtime: &str,
    expected_sha256: &str,
    model: Option<(&str, &str)>,
    output_k: usize,
) -> Result<(Shared, String, usize)> {
    let text ='''
assert needle in source
source=source.replace(needle,replacement,1).replace('        output_k: 5,','        output_k,',1)
implementation[names[1]]=source
def patch(before,after):
    return ''.join(''.join(difflib.unified_diff(before[name].splitlines(True),after[name].splitlines(True),fromfile='a/'+name,tofile='b/'+name)) for name in names if before[name]!=after[name])
for name,texts in [('original',original),('with-tests',test_only),('complete',implementation)]:
    for rel,text in texts.items():
        target=out/name/rel;target.parent.mkdir(parents=True,exist_ok=True);target.write_text(text,encoding='utf-8',newline='\n')
for name,content in [('tests-only.patch',patch(original,test_only)),('implementation-after-tests.patch',patch(test_only,implementation)),('complete.patch',patch(original,implementation))]:
    (out/name).write_text(content,encoding='utf-8',newline='\n')
manifest={'basis_head': __import__('subprocess').check_output(['git','rev-parse','HEAD'],cwd=repo).decode().strip(),'basis_source_sha256':{n:hashlib.sha256(s.encode()).hexdigest()for n,s in original.items()},'patch_sha256':{n:hashlib.sha256((out/n).read_bytes()).hexdigest()for n in ['tests-only.patch','implementation-after-tests.patch','complete.patch']},'test_fixture_sha256':sha,'not_run':'Awaiting parent stable core API and permission to compile temp copy, no actual repo edits.'}
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print(json.dumps(manifest,indent=2))
