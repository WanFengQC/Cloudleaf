from pathlib import Path
import difflib,json,hashlib,subprocess,time
root=Path(__file__).resolve().parent;out=root/'runs-final';out.mkdir(exist_ok=False)
source=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf/crates/cloudleaf-search/src/search.rs')
base=source.read_text(encoding='utf-8');red=(root/'search-tests-only.rs').read_text(encoding='utf-8');new=(root/'crates/cloudleaf-search/src/search.rs').read_text(encoding='utf-8')
def patch(a,b):return ''.join(difflib.unified_diff(a.splitlines(keepends=True),b.splitlines(keepends=True),fromfile='a/crates/cloudleaf-search/src/search.rs',tofile='b/crates/cloudleaf-search/src/search.rs'))
for name,a,b in [('tests-only.patch',base,red),('implementation-after-tests.patch',red,new),('complete.patch',base,new)]:
    (root/name).write_text(patch(a,b),encoding='utf-8',newline='\n')
commands=[];cargo='C:/Users/EDY/.cargo/bin/cargo.exe'
for label,args in [('tests',[cargo,'test','-p','cloudleaf-search','--locked','--offline']),('lint',[cargo,'clippy','-p','cloudleaf-search','--all-targets','--locked','--offline','--','-D','warnings']),('fmt',[cargo,'fmt','--all','--check'])]:
    start=time.perf_counter();p=subprocess.run(args,cwd=root,capture_output=True)
    (out/(label+'.stdout')).write_bytes(p.stdout);(out/(label+'.stderr')).write_bytes(p.stderr)
    commands.append({'label':label,'command':args,'returncode':p.returncode,'seconds':time.perf_counter()-start,'stdout_sha256':hashlib.sha256(p.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(p.stderr).hexdigest()})
    assert p.returncode==0,label
record={'scope':'Temporary search.rs compile verification with default-zero Lattice stubs; not actual interpreted graph integration, no product quality claim','actual_base_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'search_sha256':hashlib.sha256((root/'crates/cloudleaf-search/src/search.rs').read_bytes()).hexdigest(),'patches':{n:hashlib.sha256((root/n).read_bytes()).hexdigest()for n in ['tests-only.patch','implementation-after-tests.patch','complete.patch']},'commands':commands,'prior_failures':'runs retains initial missing temp lock, compile-red and type_complexity lint failure; type alias fixes lint without suppression','future_state_invariant':'edited path cap is global; interpreted states keyed last model history and used_edits; full default graph uses old algorithms'}
(out/'summary.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n');print(json.dumps(record,indent=2))
