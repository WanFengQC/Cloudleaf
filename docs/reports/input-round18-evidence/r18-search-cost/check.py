from pathlib import Path
import subprocess,json,hashlib,time
root=Path(__file__).resolve().parent;out=root/'runs';out.mkdir(exist_ok=False)
cargo='C:/Users/EDY/.cargo/bin/cargo.exe'
commands=[]
def run(label,args):
    start=time.perf_counter();p=subprocess.run(args,cwd=root,capture_output=True)
    (out/(label+'.stdout')).write_bytes(p.stdout);(out/(label+'.stderr')).write_bytes(p.stderr)
    row={'label':label,'command':args,'returncode':p.returncode,'seconds':time.perf_counter()-start,'stdout_sha256':hashlib.sha256(p.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(p.stderr).hexdigest()};commands.append(row)
    (out/'commands.json').write_text(json.dumps(commands,indent=2)+'\n',encoding='utf-8',newline='\n');print(json.dumps(row),flush=True)
    return p
run('initial-locked',[cargo,'test','-p','cloudleaf-search','--locked','--offline'])
assert run('temp-lock',[cargo,'generate-lockfile','--offline']).returncode==0
search=root/'crates/cloudleaf-search/src/search.rs';green=search.read_bytes()
search.write_bytes((root/'search-tests-only.rs').read_bytes())
assert run('red',[cargo,'test','-p','cloudleaf-search','interpretation_regression','--locked','--offline']).returncode!=0
search.write_bytes(green)
assert run('green',[cargo,'test','-p','cloudleaf-search','--locked','--offline']).returncode==0
assert run('lint',[cargo,'clippy','-p','cloudleaf-search','--all-targets','--locked','--offline','--','-D','warnings']).returncode==0
assert run('fmt',[cargo,'fmt','--all','--check']).returncode==0
