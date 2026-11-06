"""每条命令独立保留原始输出、返回码及源码身份，不覆盖历史运行。"""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
root=Path(__file__).resolve().parents[1]
label,*command=sys.argv[1:]
out=root/'.local/round17-runs'/label
out.mkdir(parents=True,exist_ok=False)
env=os.environ.copy()
env['PATH']=str(Path.home()/'.cargo/bin')+os.pathsep+env['PATH']
env['CARGO_TARGET_DIR']=str(root/'.local/build')
env['PYTHONUTF8']='1';env['PYTHONIOENCODING']='utf-8'
command[0]=shutil.which(command[0],path=env['PATH']) or command[0]
files=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard'],cwd=root).decode('utf-8').splitlines()
source={name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in files
        if (root/name).is_file() and (name.endswith(('.rs','.toml','.py','.yml')) or name=='Cargo.lock')}
diff=subprocess.check_output(['git','diff','--binary','HEAD'],cwd=root)
(out/'worktree.diff').write_bytes(diff)
start=time.perf_counter();result=subprocess.run(command,cwd=root,env=env,capture_output=True)
record={'command':command,'returncode':result.returncode,'seconds':time.perf_counter()-start,
    'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),
    'at_utc':datetime.now(timezone.utc).isoformat(),'CARGO_TARGET_DIR':env['CARGO_TARGET_DIR'],
    'tool_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'source_files_sha256':source,'worktree_diff_sha256':hashlib.sha256(diff).hexdigest()}
for name,value in [('stdout',result.stdout),('stderr',result.stderr)]:
    (out/(name+'.txt')).write_bytes(value)
    record[name]={'bytes':len(value),'sha256':hashlib.sha256(value).hexdigest()}
(out/'result.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in record.items() if k!='source_files_sha256'},ensure_ascii=False))
print(result.stdout.decode('utf-8',errors='replace')[-2500:]);print(result.stderr.decode('utf-8',errors='replace')[-3500:])
raise SystemExit(result.returncode)
