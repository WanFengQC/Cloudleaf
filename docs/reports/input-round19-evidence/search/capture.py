"""保存临时探针每条命令、返回码与完整原始输出。"""
from pathlib import Path
from datetime import datetime,timezone
import hashlib,json,os,subprocess,sys,time,shutil
out=Path(__file__).resolve().parent
label,*command=sys.argv[1:]
folder=out/'runs'/label;folder.mkdir(parents=True,exist_ok=False)
env=os.environ.copy();env['PATH']=str(Path.home()/'.cargo/bin')+os.pathsep+env['PATH'];env['PYTHONUTF8']='1';env['PYTHONIOENCODING']='utf-8'
command[0]=shutil.which(command[0],path=env['PATH']) or command[0]
start=time.perf_counter();result=subprocess.run(command,cwd=out/'source',capture_output=True,env=env)
(folder/'stdout.txt').write_bytes(result.stdout);(folder/'stderr.txt').write_bytes(result.stderr)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
record={'command':command,'returncode':result.returncode,'seconds':time.perf_counter()-start,'at_utc':datetime.now(timezone.utc).isoformat(),'source':{n:sha(out/'source/crates/cloudleaf-search/src'/n)for n in ['search.rs','character.rs','lexicon.rs']},'stdout_sha256':sha(folder/'stdout.txt'),'stderr_sha256':sha(folder/'stderr.txt'),'tool_sha256':sha(__file__)}
(folder/'result.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8');print(json.dumps(record));raise SystemExit(result.returncode)
