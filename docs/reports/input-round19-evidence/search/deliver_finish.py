"""交付仅搜索和字符缓存补丁；不写实际仓库。"""
from pathlib import Path
import difflib,hashlib,json,subprocess
p=Path(__file__).resolve().parent
actual=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=p/'delivery-finish';out.mkdir(exist_ok=True)
files=['crates/cloudleaf-search/src/search.rs','crates/cloudleaf-search/src/character.rs','crates/cloudleaf-search/tests/interpreted_resources.rs','crates/cloudleaf-search/tests/user_preferences.rs']
parts=[];identities={}
for name in files:
    source=p/'source'/name;new=source.read_text(encoding='utf-8')
    old=subprocess.run(['git','show','cf54c84d400b627dfa9b246b3800a82d65f061b8:'+name],cwd=actual,capture_output=True)
    before=old.stdout.decode('utf-8') if old.returncode==0 else ''
    parts.append(''.join(difflib.unified_diff(before.splitlines(keepends=True),new.splitlines(keepends=True),fromfile='a/'+name if before else '/dev/null',tofile='b/'+name)))
    dst=out/Path(name).name;dst.write_bytes(source.read_bytes())
    identities[name]={'sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'bytes':source.stat().st_size,'old_sha256':hashlib.sha256(old.stdout).hexdigest() if before else None}
(out/'search-character.patch').write_text(''.join(parts),encoding='utf-8',newline='\n')
(out/'source-identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'files':len(files),'patch_bytes':(out/'search-character.patch').stat().st_size}))
