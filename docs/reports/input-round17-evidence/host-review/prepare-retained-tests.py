from pathlib import Path
import difflib, hashlib, json, subprocess
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=Path(__file__).resolve().parent
path='adapters/windows/isolated_input.rs'
old=(repo/path).read_text(encoding='utf-8')
tests='''
    #[test]
    fn repeated_focus_loss_and_unknown_preserve_each_unacknowledged_raw_input() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        type_text(&local, "nihao");
        local.process(InputEvent::FocusLost, None, &|_| CommitOutcome::Applied).unwrap();
        assert!(local.retained.borrow().contains("nihao"));
        local.process(InputEvent::Activate, None, &|_| CommitOutcome::Applied).unwrap();
        type_text(&local, "zhongguo");
        local.process(InputEvent::FocusLost, None, &|_| CommitOutcome::Applied).unwrap();
        assert!(local.retained.borrow().contains("nihao"));
        assert!(local.retained.borrow().contains("zhongguo"));
        local.process(InputEvent::Activate, None, &|_| CommitOutcome::Applied).unwrap();
        type_text(&local, "woaini");
        local.process(InputEvent::Key(Key::Space), None, &|_| CommitOutcome::Unknown).unwrap();
        assert!(local.host.quarantined.get());
        for raw in ["nihao", "zhongguo", "woaini"] {
            assert!(local.retained.borrow().contains(raw), "尚未确认的旧材料不能被后续失焦或Unknown覆盖：{raw}");
        }
        local.reset().unwrap();
        assert!(local.retained.borrow().is_empty(), "明确新会话才确认并清除旧保全材料");
    }

    #[test]
    fn retained_capacity_failure_quarantines_before_clearing_current_composition() {
        let local = LocalHost::new(Host::new().unwrap()).unwrap();
        *local.retained.borrow_mut() = "x".repeat(16384);
        type_text(&local, "nihao");
        let before = local.host.engine.borrow().frame();
        let projected_before = local.host.projection.borrow().frame();
        let result = local.process(InputEvent::FocusLost, None, &|_| CommitOutcome::Applied);
        assert!(result.is_err());
        assert!(local.host.quarantined.get());
        assert_eq!(local.retained.borrow().as_str(), "x".repeat(16384));
        assert_eq!(local.host.engine.borrow().frame(), before);
        assert_eq!(local.host.projection.borrow().frame(), projected_before);
        assert_eq!(before.preedit, "nihao");
        assert!(local.document.borrow().is_empty());
    }
'''
pos=old.rfind('\n}')
new=old[:pos]+tests+old[pos:]
patch='diff --git a/'+path+' b/'+path+'\n'+''.join(difflib.unified_diff(old.splitlines(keepends=True),new.splitlines(keepends=True),fromfile='a/'+path,tofile='b/'+path))
p=out/'tests-only-retained.patch'
p.write_text(patch,encoding='utf-8',newline='\n')
record={'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo).decode().strip(),'source_before_sha256':hashlib.sha256(old.encode('utf-8')).hexdigest(),'patch_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'test_execution':'not-run; root will apply then execute red/green'}
(out/'retained-test-identity.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(record,indent=2))
