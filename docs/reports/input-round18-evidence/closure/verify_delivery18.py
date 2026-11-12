"""提交前核对证据字节、相对链接及实际检查源码，不重复业务测试。"""
from pathlib import Path
import gzip,hashlib,json,re,subprocess,os
root=Path(__file__).resolve().parents[1];ev=root/'docs/reports/input-round18-evidence'
index=json.loads((ev/'index.json').read_text(encoding='utf-8'))
for name,row in index['files'].items():
    raw=(ev/name).read_bytes();assert hashlib.sha256(raw).hexdigest()==row['sha256'],name
    if name.endswith('.gz'):
        unzipped=gzip.decompress(raw)
        if 'raw_sha256'in row:assert hashlib.sha256(unzipped).hexdigest()==row['raw_sha256'],name
report=root/'docs/reports/input-round18-status.md'
for link in re.findall(r'\]\(([^)]+)\)',report.read_text(encoding='utf-8')):
    if not link.startswith('https://'):assert (report.parent/link).is_file(),link
tested=json.loads((root/'.local/round18-runs/verified-04/result.json').read_text(encoding='utf-8'))
for name,value in tested['source_files_sha256'].items():assert hashlib.sha256((root/name).read_bytes()).hexdigest()==value,name
rust=(root/'.local/round18-runs/verified-04/stdout.txt').read_text(encoding='utf-8')
totals=[tuple(map(int,m))for m in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',rust)]
assert [sum(t[i]for t in totals)for i in range(3)]==[254,0,2]
python=(root/'.local/round18-runs/verified-08/stderr.txt').read_text(encoding='utf-8');assert 'Ran 71 tests'in python and python.rstrip().endswith('OK')
commands=[['git','diff','--check'],['python','scripts/check_workspace.py']]
results=[]
env=os.environ.copy();env['PATH']=str(Path.home()/'.cargo/bin')+os.pathsep+env['PATH'];env['PYTHONUTF8']='1';env['PYTHONIOENCODING']='utf-8'
for command in commands:
    result=subprocess.run(command,cwd=root,capture_output=True,env=env)
    results.append({'command':command,'returncode':result.returncode,'stdout':result.stdout.decode('utf-8'),'stderr':result.stderr.decode('utf-8')})
    assert result.returncode==0,results[-1]
record={'source_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),'indexed_files_verified':len(index['files']),'gzip_CRC':'passed','report_links':'all resolved','production_source_matches_full_regression':True,'rust':[254,0,2],'python':71,'commands':results,'tool_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
(root/'.local/round18-precommit-verification.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'files':len(index['files']),'checks':'passed'}))
