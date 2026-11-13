"""独立保存命令、cwd、源码/工具/日志身份，不覆盖先前临时试验。"""
from pathlib import Path
from datetime import datetime,timezone
import hashlib,json,os,subprocess,sys,time,shutil
p=Path(__file__).resolve().parent
label,cwd,*command=sys.argv[1:]
folder=p/'runs'/label;folder.mkdir(exist_ok=False)
cwd=Path(cwd).resolve()
env=os.environ.copy();env['PATH']=str(Path.home()/'.cargo/bin')+os.pathsep+env['PATH'];env['PYTHONUTF8']='1';env['PYTHONIOENCODING']='utf-8'
command[0]=shutil.which(command[0],path=env['PATH'])or command[0]
sha=lambda path:hashlib.sha256(Path(path).read_bytes()).hexdigest()
paths=[*sorted((cwd/'crates').rglob('*.rs')),*sorted((cwd/'probe').rglob('*.rs'))] if(cwd/'crates').exists()else[]
source_before={str(f.relative_to(cwd)).replace('\\','/'):sha(f)for f in paths}
now=time.perf_counter();result=subprocess.run(command,cwd=cwd,env=env,capture_output=True)
(folder/'stdout.txt').write_bytes(result.stdout);(folder/'stderr.txt').write_bytes(result.stderr)
record={'command':command,'cwd':str(cwd),'at_utc':datetime.now(timezone.utc).isoformat(),'seconds':time.perf_counter()-now,'returncode':result.returncode,'source_before':source_before,'source_after':{str(f.relative_to(cwd)).replace('\\','/'):sha(f)for f in paths},'stdout_sha256':sha(folder/'stdout.txt'),'stderr_sha256':sha(folder/'stderr.txt'),'tool_sha256':sha(__file__)}
(folder/'result.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in record.items()if k not in ['source_before','source_after']}))
raise SystemExit(result.returncode)
