from pathlib import Path
import hashlib, io, json, os, subprocess, zipfile
root=Path(__file__).resolve().parents[1]
source=root/'.local/round17-baseline-source'
source.mkdir(exist_ok=True)
commit='521472db18ecd7926f6e5b58b0afd7b5da674a07'
raw=subprocess.check_output(['git','archive','--format=zip',commit],cwd=root)
with zipfile.ZipFile(io.BytesIO(raw)) as z:z.extractall(source)
env=os.environ.copy();env['PATH']=str(Path.home()/'.cargo/bin')+os.pathsep+env['PATH'];env['CARGO_TARGET_DIR']=str(root/'.local/round17-baseline-build')
command=[str(Path.home()/'.cargo/bin/cargo.exe'),'build','-p','cloudleaf-input','--example','input_trace','--release','--locked','--offline']
p=subprocess.run(command,cwd=source,env=env,capture_output=True)
for label,data in [('stdout',p.stdout),('stderr',p.stderr)]: (root/f'.local/round17-baseline-build.{label}').write_bytes(data)
(root/'.local/round17-baseline-source.json').write_text(json.dumps({'commit':commit,'archive_sha256':hashlib.sha256(raw).hexdigest(),'command':command,'returncode':p.returncode,'source':str(source)},indent=2)+'\n',encoding='utf-8')
print(p.stderr.decode('utf-8',errors='replace')[-2000:]);raise SystemExit(p.returncode)
