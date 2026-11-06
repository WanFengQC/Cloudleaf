from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import subprocess
import sys
import urllib.request

root=Path(__file__).resolve().parents[1]
label=sys.argv[1]
out=root/'.local/round16-remote'/label
out.mkdir(parents=True,exist_ok=False)
raw=subprocess.run(['git','-c','credential.interactive=never','credential','fill'],input=b'protocol=https\nhost=github.com\n\n',capture_output=True,timeout=30)
if raw.returncode:raise RuntimeError('Existing Git credential unavailable')
credential=dict(line.split('=',1) for line in raw.stdout.decode().splitlines() if '=' in line)
headers={'Authorization':'Bearer '+credential['password'],'Accept':'application/vnd.github+json','User-Agent':'Cloudleaf-Round16','Cache-Control':'no-cache'}
api='https://api.github.com/repos/WanFengQC/Cloudleaf'
def get(suffix,name):
    with urllib.request.urlopen(urllib.request.Request(api+suffix,headers=headers),timeout=45) as response:data=response.read()
    (out/name).write_bytes(data)
    return json.loads(data)
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip()
main=get('/branches/main','main.json')
branch=get('/branches/codex%2Finput-round14','branch.json')
runs=get('/actions/runs?event=push&per_page=100','runs.json')
prs=[]
for n in range(1,6):
    data=get(f'/pulls/{n}',f'pr{n}.json')
    prs.append({'number':n,'state':data['state'],'draft':data['draft'],'merged':data['merged'],'head':data['head']['sha'],'url':data['html_url']})
record={'at_utc':datetime.now(timezone.utc).isoformat(),'local_head':head,'main':main['commit']['sha'],'branch':branch['commit']['sha'],'pulls':prs,
    'push_runs':[{'id':v['id'],'head_sha':v['head_sha'],'status':v['status'],'conclusion':v['conclusion'],'url':v['html_url']} for v in runs['workflow_runs'] if v['head_branch']=='codex/input-round14'],
    'files':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in out.iterdir() if p.is_file()}}
(out/'summary.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(record,ensure_ascii=False,indent=2))
