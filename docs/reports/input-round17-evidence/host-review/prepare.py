from pathlib import Path
import difflib, hashlib, json, re, subprocess
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=Path(__file__).resolve().parent
path='adapters/windows/isolated_input.rs'
old=(repo/path).read_text(encoding='utf-8')
test='''
    #[test]
    fn child_edit_focus_then_host_deactivation_preserves_raw_and_closes_candidates() {
        let view = Rc::new(Window {
            local: LocalHost::new(Host::new().unwrap()).unwrap(),
            window: Cell::new(HWND::default()),
            document: Cell::new(HWND::default()),
            status: Cell::new(HWND::default()),
            fault: Cell::new(false),
            ui: RefCell::new(None),
        });
        type_text(&view.local, "nihao");
        let frame = view.local.host.engine.borrow().frame();
        *view.ui.borrow_mut() = Some(CandidateUi::isolated(
            view.local.host.clone(), frame, Rc::new(|_, _| Ok(())),
        ).unwrap());
        // 安全：隐藏窗口使用实际WndProc；只向本测试创建的父子HWND发送焦点/激活消息。
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
            let class = w!("CloudleafRound17ChildFocusRegression");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc), hInstance: instance,
                lpszClassName: class, ..Default::default()
            };
            assert_ne!(RegisterClassW(&wc), 0);
            let window = CreateWindowExW(
                WINDOW_EX_STYLE::default(), class, w!(""), WS_OVERLAPPEDWINDOW,
                0, 0, 770, 580, None, None, Some(instance),
                Some(Rc::as_ptr(&view).cast()),
            ).unwrap();
            SetFocus(Some(window)).unwrap();
            SetFocus(Some(view.document.get())).unwrap();
            assert_eq!(GetFocus(), view.document.get());
            assert_eq!(view.local.host.engine.borrow().frame().preedit, "nihao");
            SendMessageW(window, WM_ACTIVATE, Some(WPARAM(WA_INACTIVE as usize)), Some(LPARAM(0)));
            let after = view.local.host.engine.borrow().frame();
            let retained = view.local.retained.borrow().clone();
            let candidates_closed = view.ui.borrow().is_none();
            DestroyWindow(window).unwrap();
            let mut quit = MSG::default();
            let _ = PeekMessageW(&mut quit, None, WM_QUIT, WM_QUIT, PM_REMOVE);
            UnregisterClassW(class, Some(instance)).unwrap();
            assert!(after.preedit.is_empty(), "整窗失活必须取消旧组合，而不是依赖父窗仍有键盘焦点");
            assert_eq!(retained, "nihao");
            assert!(candidates_closed);
            assert!(view.local.document.borrow().is_empty());
        }
    }
'''
pos=old.rfind('\n}')
withtest=old[:pos]+test+old[pos:]
before='                WM_KILLFOCUS if !view.local.host.quarantined.get() && GetAncestor(HWND(wparam.0 as *mut _),GA_ROOT)!=window => { view.process(InputEvent::FocusLost,None)?; Ok(LRESULT(0)) }'
after='''                // 顶层激活状态覆盖EDIT/按钮持有焦点时的外部切换，子控件间切换不取消组合。
                WM_ACTIVATE if wparam.0 & 0xffff == WA_INACTIVE as usize => { if !view.local.host.quarantined.get() { view.process(InputEvent::FocusLost,None)?; } Ok(LRESULT(0)) }'''
assert before in withtest
fixed=withtest.replace(before,after)
for name,a,b in [('tests-only.patch',old,withtest),('implementation-after-tests.patch',withtest,fixed)]:
    diff=''.join(difflib.unified_diff(a.splitlines(keepends=True),b.splitlines(keepends=True),fromfile='a/'+path,tofile='b/'+path))
    (out/name).write_text('diff --git a/'+path+' b/'+path+'\n'+diff,encoding='utf-8',newline='\n')
(out/'with-tests.rs').write_text(withtest,encoding='utf-8',newline='\n')
(out/'fixed.rs').write_text(fixed,encoding='utf-8',newline='\n')
base='521472db18ecd7926f6e5b58b0afd7b5da674a07'
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo).decode().strip()
historical=subprocess.check_output(['git','ls-tree','-r','--name-only',base,'--','docs/reports','data','tests','licenses'],cwd=repo).decode().splitlines()
changed=subprocess.check_output(['git','diff','--name-only',base,head],cwd=repo).decode().splitlines()
assert not set(historical).intersection(changed)
paths=subprocess.check_output(['git','ls-tree','-r','--name-only',base,'--','crates','adapters','apps'],cwd=repo).decode().splitlines()
names=[]
for path in paths:
    if not path.endswith('.rs'):continue
    previous=subprocess.check_output(['git','show',base+':'+path],cwd=repo).decode('utf-8')
    current=(repo/path).read_text(encoding='utf-8')
    tests=re.findall(r'#\[test\][\s\S]*?\bfn\s+(\w+)\s*\(',previous)
    for name in tests:
        assert re.search(r'\bfn\s+'+re.escape(name)+r'\s*\(',current),path+':'+name
        names.append(path+':'+name)
record={'base':base,'reviewed_head':head,'historical_reports_data_tests_licenses_paths_unchanged':len(historical),'old_rust_test_functions_retained':len(names),'rust_test_names':names,'changed_files':changed,'focus_gap':{'file':'adapters/windows/isolated_input.rs','repro':'Parent gets WM_KILLFOCUS into its readonly child, deliberately ignored; later child loses focus externally and parent gets only WM_ACTIVATE(WA_INACTIVE), currently unhandled. Composition remains.','severity':'P1 isolated-host focus/recovery contract; owned-document-only boundary remains','test_execution':'not-run; parent will execute red then fix on actual worktree'},'patch_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in out.glob('*.patch')}}
(out/'review-preservation.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({k:v for k,v in record.items() if k not in ['rust_test_names','changed_files']},ensure_ascii=False,indent=2))
