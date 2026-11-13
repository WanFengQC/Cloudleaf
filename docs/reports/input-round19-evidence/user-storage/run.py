from pathlib import Path
import hashlib,json,os,subprocess,sys,time
out=Path(__file__).resolve().parent;label=sys.argv[1];command=sys.argv[2:]
folder=out/'runs'/label;folder.mkdir(parents=True,exist_ok=False)
env=os.environ.copy();env['PATH']=str(Path.home()/'.cargo/bin')+os.pathsep+env['PATH'];env['PYTHONUTF8']='1';env['PYTHONIOENCODING']='utf-8'
command[0]=shutil_path=__import__('shutil').which(command[0],path=env['PATH'])or command[0]
start=time.perf_counter();result=subprocess.run(command,cwd=out/'source',env=env,capture_output=True)
for name,raw in [('stdout.txt',result.stdout),('stderr.txt',result.stderr)]: (folder/name).write_bytes(raw)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
record={'command':command,'returncode':result.returncode,'seconds':time.perf_counter()-start,'source':{str(p.relative_to(out/'source')):sha(p)for p in (out/'source/crates/cloudleaf-user').rglob('*')if p.is_file()},'design_head':'8dd3505','stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()}
(folder/'result.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8');print(json.dumps({'label':label,'returncode':result.returncode,'seconds':record['seconds']}));print(result.stderr.decode('utf-8',errors='replace')[-3000:]);sys.exit(result.returncode)
