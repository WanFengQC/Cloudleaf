from pathlib import Path
from datetime import datetime,timezone
import json
import subprocess
import sys
import urllib.request

root=Path(__file__).resolve().parents[1]
head,label=sys.argv[1:]
out=root/'.local/round17-ci'/label;out.mkdir(parents=True,exist_ok=False)
raw=subprocess.run(['git','-c','credential.interactive=never','credential','fill'],input=b'protocol=https\nhost=github.com\n\n',capture_output=True,timeout=30)
if raw.returncode:raise RuntimeError('Existing Git credential unavailable')
credential=dict(line.split('=',1) for line in raw.stdout.decode().splitlines() if '=' in line)
headers={'Authorization':'Bearer '+credential['password'],'Accept':'application/vnd.github+json','User-Agent':'Cloudleaf-Round17-final-discovery','Cache-Control':'no-cache'}
api='https://api.github.com/repos/WanFengQC/Cloudleaf'
records={}
for suffix,name in [(f'/actions/runs?head_sha={head}&event=push&per_page=10','runs'),(f'/commits/{head}/check-runs?per_page=100','checks')]:
    with urllib.request.urlopen(urllib.request.Request(api+suffix+'&_cloudleaf='+datetime.now(timezone.utc).strftime('%Y%m%d%H%M%S'),headers=headers),timeout=45) as response:data=response.read()
    (out/(name+'.json')).write_bytes(data);records[name]=json.loads(data)
print(json.dumps({'runs':[{k:v[k] for k in ['id','head_sha','status','conclusion','html_url']} for v in records['runs']['workflow_runs']], 'checks':[{k:v[k] for k in ['id','name','status','conclusion','details_url']} for v in records['checks']['check_runs']]},indent=2))
