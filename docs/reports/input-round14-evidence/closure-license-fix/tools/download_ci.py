"""下载最终精确CI日志/产物，跨站签名URL不携带Git凭据。"""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
import zipfile

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,request,file,code,message,headers,new_url):
        return None

root=Path(__file__).resolve().parents[1]
head,run_id,label=sys.argv[1:]
out=root/'.local/round14-ci'/label
out.mkdir(parents=True,exist_ok=False)
raw=subprocess.run(['git','-c','credential.interactive=never','credential','fill'],input=b'protocol=https\nhost=github.com\n\n',capture_output=True,timeout=30)
if raw.returncode:raise RuntimeError('Existing Git credential unavailable')
credential=dict(line.split('=',1) for line in raw.stdout.decode().splitlines() if '=' in line)
headers={'Authorization':'Bearer '+credential['password'],'Accept':'application/vnd.github+json','User-Agent':'Cloudleaf-Round14-artifacts'}
api='https://api.github.com/repos/WanFengQC/Cloudleaf'

def get_json(suffix):
    with urllib.request.urlopen(urllib.request.Request(api+suffix,headers=headers),timeout=45) as response:
        return json.loads(response.read())

def download(suffix,name):
    request=urllib.request.Request(api+suffix,headers=headers)
    try:
        response=urllib.request.build_opener(NoRedirect).open(request,timeout=45)
    except urllib.error.HTTPError as error:
        if error.code not in (301,302,303,307,308):raise
        location=error.headers['Location']
        if urllib.parse.urlparse(location).scheme!='https':raise RuntimeError('Non-HTTPS redirect')
        response=urllib.request.urlopen(urllib.request.Request(location,headers={'User-Agent':'Cloudleaf-Round14-artifacts'}),timeout=45)
    with response,(out/name).open('wb') as target:
        while block:=response.read(1024*1024):target.write(block)
    path=out/name
    with zipfile.ZipFile(path) as archive:
        assert archive.testzip() is None
        files=[{'name':row.filename,'bytes':row.file_size,'sha256':hashlib.sha256(archive.read(row)).hexdigest()}
               for row in archive.infolist() if not row.is_dir()]
    return {'file':name,'bytes':path.stat().st_size,'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'crc':'passed','members':files}

run=get_json(f'/actions/runs/{run_id}')
assert run['head_sha']==head and run['event']=='push' and run['conclusion']=='success'
artifacts=get_json(f'/actions/runs/{run_id}/artifacts?per_page=100')['artifacts']
records=[download(f'/actions/runs/{run_id}/logs','logs.zip')]
for artifact in artifacts:
    assert artifact['workflow_run']['head_sha']==head and not artifact['expired']
    result=download(f"/actions/artifacts/{artifact['id']}/zip",artifact['name']+'.zip')
    assert artifact['digest']=='sha256:'+result['sha256']
    if artifact['name']=='input-round14-windows':
        names=[row['name'] for row in result['members']]
        assert any(name.endswith('cloudleaf-cli.exe') for name in names)
        assert all(any(name.endswith(tool+'.exe') for name in names) for tool in ['cloudleaf-cli','cloudleaf-ime'])
        assert any(name.endswith('cloudleaf_windows_ime.dll') for name in names)
        assert not any(Path(name).name in ['model.safetensors','pytorch_model.bin','settings.json'] for name in names)
    records.append(result)
(out/'downloads.json').write_text(json.dumps({'head':head,'run_id':run_id,'tool_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'files':records},ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'head':head,'archives':len(records),'CRC':'passed','artifact_digests':'all matched'},ensure_ascii=False))
