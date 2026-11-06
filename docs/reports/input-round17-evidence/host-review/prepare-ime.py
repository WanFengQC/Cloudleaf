from pathlib import Path
import difflib, hashlib, json
out=Path(__file__).resolve().parent
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
path='adapters/windows/isolated_input.rs'
old=(out/'fixed.rs').read_text(encoding='utf-8')
test='''
    #[test]
    fn isolated_window_and_child_do_not_inherit_external_ime() {
        use windows::Win32::UI::Input::Ime::{ImmGetContext, ImmReleaseContext};
        let view = Rc::new(Window {
            local: LocalHost::new(Host::new().unwrap()).unwrap(),
            window: Cell::new(HWND::default()), document: Cell::new(HWND::default()),
            status: Cell::new(HWND::default()), fault: Cell::new(false),
            ui: RefCell::new(None),
        });
        // 安全：检查本测试隐藏父子窗口的上下文；不切换用户布局或其他窗口的输入法。
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
            let class = w!("CloudleafRound17OwnImeRegression");
            let wc = WNDCLASSW { lpfnWndProc: Some(window_proc), hInstance: instance,
                lpszClassName: class, ..Default::default() };
            assert_ne!(RegisterClassW(&wc), 0);
            let window = CreateWindowExW(
                WINDOW_EX_STYLE::default(), class, w!(""), WS_OVERLAPPEDWINDOW,
                0, 0, 770, 580, None, None, Some(instance),
                Some(Rc::as_ptr(&view).cast()),
            ).unwrap();
            let mut inherited = Vec::new();
            for hwnd in [window, view.document.get()] {
                let context = ImmGetContext(hwnd);
                inherited.push(!context.is_invalid());
                if !context.is_invalid() { assert!(ImmReleaseContext(hwnd, context).as_bool()); }
            }
            DestroyWindow(window).unwrap();
            let mut quit = MSG::default();
            let _ = PeekMessageW(&mut quit, None, WM_QUIT, WM_QUIT, PM_REMOVE);
            UnregisterClassW(class, Some(instance)).unwrap();
            assert_eq!(inherited, [false, false], "隔离键盘入口不得再把拼音交给系统输入法");
        }
    }
'''
pos=old.rfind('\n}')
withtest=old[:pos]+test+old[pos:]
feature_path='apps/windows-ime/Cargo.toml'
feature_old=(repo/feature_path).read_text(encoding='utf-8')
feature_new=feature_old.replace('"Win32_UI_Input_KeyboardAndMouse"','"Win32_UI_Input_KeyboardAndMouse", "Win32_UI_Input_Ime"')
assert feature_old!=feature_new
def diff(path,a,b):return 'diff --git a/'+path+' b/'+path+'\n'+''.join(difflib.unified_diff(a.splitlines(keepends=True),b.splitlines(keepends=True),fromfile='a/'+path,tofile='b/'+path))
(out/'tests-only-ime.patch').write_text(diff(feature_path,feature_old,feature_new)+diff(path,old,withtest),encoding='utf-8',newline='\n')
fixed=withtest.replace('UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},','UI::{Input::{KeyboardAndMouse::*, Ime::{HIMC, IACE_CHILDREN, ImmAssociateContextEx}}, WindowsAndMessaging::*},')
needle='                    view.document.set(CreateWindowExW(WS_EX_CLIENTEDGE,w!("EDIT"),w!(""),WS_CHILD|WS_VISIBLE|WS_VSCROLL|WINDOW_STYLE(ES_READONLY as u32|ES_MULTILINE as u32|ES_AUTOVSCROLL as u32),12,320,720,200,Some(window),None,None,None)?);'
assert needle in fixed
fixed=fixed.replace(needle,needle+'''
                    // 只解绑本窗口及已创建子控件的继承IME；拼音由本隔离核心处理，不改变系统布局。
                    if !ImmAssociateContextEx(window, HIMC::default(), 0).as_bool()
                        || !ImmAssociateContextEx(window, HIMC::default(), IACE_CHILDREN).as_bool() {
                        return Ok(LRESULT(-1));
                    }''')
(out/'implementation-after-tests-ime.patch').write_text(diff(path,withtest,fixed),encoding='utf-8',newline='\n')
(out/'fixed-ime.rs').write_text(fixed,encoding='utf-8',newline='\n')
(out/'ime-binding-review.json').write_text(json.dumps({'bindings':'pinned windows0.62.2 Win32_UI_Input_Ime','feature_only_no_new_dependency':True,'documentation':'https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-immassociatecontextex','IACE_CHILDREN_only_children':'Rootmustuseflag0formainandIACE_CHILDRENforalreadycreatedchildren','scope':'OnlynewisolatedHostHWNDanditsownthreadchildren; no globalsettings/profile/layout/TSFregistration','test_execution':'not-run; rootwillrunactualred/green','patch_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in out.glob('*ime.patch')}},ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print((out/'ime-binding-review.json').read_text(encoding='utf-8'))
