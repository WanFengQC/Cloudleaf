"""逐命令归档R13原始字节、返回码和实际环境；不覆盖既有运行。"""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
sys.stdout.reconfigure(encoding='utf-8')

root=Path(__file__).resolve().parents[1]
label,*command=sys.argv[1:]
directory=root/'.local/round13-runs'/label
directory.mkdir(parents=True,exist_ok=False)
environment=os.environ.copy()
environment['PATH']=str(Path.home()/'.cargo/bin')+os.pathsep+environment['PATH']
environment['CARGO_TARGET_DIR']=str(root/'.local/round13-build')
environment['PYTHONUTF8']='1'
environment['PYTHONIOENCODING']='utf-8'
command[0]=shutil.which(command[0],path=environment['PATH']) or command[0]
source_files=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard'],cwd=root).decode('utf-8').splitlines()
source_hashes={name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in source_files
               if (root/name).is_file() and (name.endswith(('.rs','.toml','.py')) or name=='Cargo.lock')}
diff=subprocess.check_output(['git','diff','--binary','HEAD'],cwd=root)
(directory/'worktree.diff').write_bytes(diff)
start=time.perf_counter()
result=subprocess.run(command,cwd=root,env=environment,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
record={'command':command,'returncode':result.returncode,'seconds':time.perf_counter()-start,
        'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),
        'at_utc':datetime.now(timezone.utc).isoformat(),'CARGO_TARGET_DIR':environment['CARGO_TARGET_DIR'],
        'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
record['source_files_sha256']=source_hashes
record['worktree_diff_sha256']=hashlib.sha256(diff).hexdigest()
for name,value in [('stdout',result.stdout),('stderr',result.stderr)]:
    (directory/(name+'.txt')).write_bytes(value)
    record[name]={'bytes':len(value),'sha256':hashlib.sha256(value).hexdigest()}
(directory/'result.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({key:value for key,value in record.items() if key!='source_files_sha256'},ensure_ascii=False))
print(result.stdout.decode('utf-8',errors='replace')[-5000:])
print(result.stderr.decode('utf-8',errors='replace')[-5000:])
sys.exit(result.returncode)
