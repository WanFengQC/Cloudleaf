from pathlib import Path
import difflib,hashlib,json
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=Path(__file__).resolve().parent
name='adapters/windows/isolated_input.rs'
before=(repo/name).read_text(encoding='utf-8')
needle='''                    if matches!(virtual_key, x if x == VK_F6.0 || x == VK_F7.0 || x == VK_F8.0) {'''
after=before.replace(needle,'''                    if matches!(virtual_key, x if x == VK_F6.0 || x == VK_F7.0 || x == VK_F8.0)
                        && GetKeyState(i32::from(VK_CONTROL.0)) >= 0
                        && GetKeyState(i32::from(VK_MENU.0)) >= 0
                        && GetKeyState(i32::from(VK_SHIFT.0)) >= 0 {''',1)
assert before!=after
patch=''.join(difflib.unified_diff(before.splitlines(True),after.splitlines(True),fromfile='a/'+name,tofile='b/'+name))
path=out/'modifier-keys-only.patch';path.write_text(patch,encoding='utf-8',newline='\n')
print(json.dumps({'patch':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'source_UTF8_LF_sha256':hashlib.sha256(before.encode()).hexdigest()}))
