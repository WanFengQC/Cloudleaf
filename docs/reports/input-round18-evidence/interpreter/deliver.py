from pathlib import Path
import difflib,hashlib,json
here=Path(__file__).resolve().parent
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=here/'delivery';out.mkdir(exist_ok=True)
lib=(repo/'crates/cloudleaf-pinyin/src/lib.rs').read_text(encoding='utf-8')
newlib=(here/'source/crates/cloudleaf-pinyin/src/lib.rs').read_text(encoding='utf-8')
scheme=(here/'scheme.rs').read_text(encoding='utf-8')
stub=scheme[scheme.index('#[cfg(test)]\nmod tests {'):]
def patch(name,old,new):
    header=f'diff --git a/{name} b/{name}\n'
    if old is None:header+='new file mode 100644\n'
    return header+''.join(difflib.unified_diff((old or '').splitlines(True),new.splitlines(True),fromfile='a/'+name if old is not None else '/dev/null',tofile='b/'+name))
red=patch('crates/cloudleaf-pinyin/src/lib.rs',lib,newlib)+patch('crates/cloudleaf-pinyin/src/scheme.rs',None,stub)
green=patch('crates/cloudleaf-pinyin/src/scheme.rs',stub,scheme)+patch('crates/cloudleaf-pinyin/src/xiaohe.rs',None,(here/'xiaohe.rs').read_text(encoding='utf-8'))
(out/'tests-only.patch').write_text(red,encoding='utf-8',newline='\n')
(out/'implementation-after-tests.patch').write_text(green,encoding='utf-8',newline='\n')
manifest={str(p.relative_to(here)):{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}for p in [here/'scheme.rs',here/'xiaohe.rs',out/'tests-only.patch',out/'implementation-after-tests.patch',here/'runs/results.json']}
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(manifest,indent=2))
