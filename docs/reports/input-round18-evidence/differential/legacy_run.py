from pathlib import Path
from datetime import datetime,timezone
import gzip,hashlib,json,subprocess,time
root=Path(__file__).resolve().parent;repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf');out=root/'legacy-final';out.mkdir(exist_ok=False)
old=Path('C:/Users/EDY/.codex/tmp/r17-actual-diff/src/differential.rs')
src=root/'src/bin';src.mkdir(exist_ok=True);(src/'legacy.rs').write_bytes(old.read_bytes())
paths=[*sorted((repo/'crates/cloudleaf-search/src').glob('*.rs')),*sorted((repo/'crates/cloudleaf-pinyin/src').glob('*.rs'))]
hashes=lambda:{str(p.relative_to(repo)).replace('\\','/'):hashlib.sha256(p.read_bytes()).hexdigest()for p in paths}
record={'at_utc':datetime.now(timezone.utc).isoformat(),'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo).decode().strip(),'source_before':hashes(),'unchanged_old_test_source_sha256':hashlib.sha256(old.read_bytes()).hexdigest(),'commands':[]}
def run(label,args):
    start=time.perf_counter();p=subprocess.run(args,cwd=root,capture_output=True)
    (out/(label+'.stdout')).write_bytes(p.stdout);(out/(label+'.stderr')).write_bytes(p.stderr)
    row={'label':label,'command':args,'returncode':p.returncode,'seconds':time.perf_counter()-start,'stdout_sha256':hashlib.sha256(p.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(p.stderr).hexdigest()};record['commands'].append(row);print(json.dumps(row),flush=True)
    return p
try:
    p=run('build',['C:/Users/EDY/.cargo/bin/cargo.exe','build','--bin','legacy','--release','--locked','--offline']);assert p.returncode==0
    p=run('differential',[str(root/'target/release/legacy.exe')]);record['summary_lines']=[s for s in p.stdout.decode('utf-8').splitlines()if s.startswith(('FAIL\t','SUMMARY\t'))]
    z=gzip.compress(p.stdout,mtime=0);assert gzip.decompress(z)==p.stdout;(out/'differential.stdout.gz').write_bytes(z)
    record['gzip']={'bytes':len(z),'sha256':hashlib.sha256(z).hexdigest(),'full_CRC_and_uncompressed_bytes_equal':True};record['run_passed']=p.returncode==0
finally:
    record['source_after']=hashes();record['source_unchanged']=record['source_before']==record['source_after'];record['head_after']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo).decode().strip()
    record['binary_sha256']=hashlib.sha256((root/'target/release/legacy.exe').read_bytes()).hexdigest()
    record['tool_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    (out/'summary.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n');print(json.dumps({k:record[k]for k in ['summary_lines','source_unchanged','run_passed']if k in record}),flush=True)
