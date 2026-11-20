from pathlib import Path
from datetime import datetime,timezone
import subprocess,json,hashlib,sys,time,os,shutil
p=Path(__file__).resolve().parent
label,*command=sys.argv[1:];out=p/'runs'/label;out.mkdir(parents=True,exist_ok=False)
env=os.environ.copy();env['PATH']=str(Path.home()/'.cargo/bin')+os.pathsep+env['PATH'];env['PYTHONIOENCODING']='utf-8';env['PYTHONUTF8']='1'
command[0]=shutil.which(command[0],path=env['PATH']) or command[0]
now=time.perf_counter();r=subprocess.run(command,cwd=p,env=env,capture_output=True)
for n,v in [('stdout',r.stdout),('stderr',r.stderr)]: (out/(n+'.txt')).write_bytes(v)
files={str(f.relative_to(p/'source')):hashlib.sha256(f.read_bytes()).hexdigest() for f in (p/'source/crates').rglob('*.rs')}
(out/'result.json').write_text(json.dumps({'at_utc':datetime.now(timezone.utc).isoformat(),'command':command,'returncode':r.returncode,'seconds':time.perf_counter()-now,'source_sha':files,'tool_sha':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'output_sha':{n:hashlib.sha256((out/(n+'.txt')).read_bytes()).hexdigest()for n in ['stdout','stderr']}},indent=2),encoding='utf-8')
print(label,r.returncode,r.stdout.decode('utf-8',errors='replace')[-1000:],r.stderr.decode('utf-8',errors='replace')[-1000:]);sys.exit(r.returncode)
