from pathlib import Path
import hashlib,json,os,subprocess,time
root=Path(__file__).resolve().parent
project=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
env=os.environ.copy();env['CARGO_TARGET_DIR']=str(root.parent/'final-target')
files=['crates/cloudleaf-input/src/lib.rs','crates/cloudleaf-input/src/sentence.rs','crates/cloudleaf-search/src/lexicon.rs','crates/cloudleaf-types/src/lib.rs']
before={name:hashlib.sha256((project/name).read_bytes()).hexdigest() for name in files}
commands=[]
for label,args,cwd in [('input-integration',['C:/Users/EDY/.cargo/bin/cargo.exe','+1.90.0','test','-p','cloudleaf-input','--test','search_integration','--locked','--offline'],project),('engine-budget-sweep',['C:/Users/EDY/.cargo/bin/cargo.exe','+1.90.0','run','--offline'],root)]:
 start=time.perf_counter();r=subprocess.run(args,cwd=cwd,env=env,capture_output=True)
 (root/(label+'.stdout')).write_bytes(r.stdout);(root/(label+'.stderr')).write_bytes(r.stderr)
 commands.append({'label':label,'command':args,'cwd':str(cwd),'returncode':r.returncode,'seconds':time.perf_counter()-start,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()})
 assert r.returncode==0,(label,r.stderr.decode())
 print(label,r.returncode,r.stdout.decode()[-500:])
after={name:hashlib.sha256((project/name).read_bytes()).hexdigest() for name in files}
(root/'results.json').write_text(json.dumps({'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=project).decode().strip(),'commands':commands,'source_before':before,'source_after':after,'source_unchanged_during_check':before==after},ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
