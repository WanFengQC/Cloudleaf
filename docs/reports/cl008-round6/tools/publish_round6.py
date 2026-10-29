"""保存本轮版本对照的原件字节和Git发布件双SHA，不覆盖历史。"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'docs/reports/cl008-round6'
OUT.mkdir(exist_ok=False)
BASE = '08b09277d3beeb90e284145a3423b9f00eca8d6c'
IMPLEMENTATION = '9673c29151380c146ae6ecb16055072cda0f0f00'
sys.path.insert(0,str(ROOT/'scripts'))
from benchmark import source_sha256


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def save(path,value):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')


publication, original_logs, originals = {}, [], []


def copy(source,relative):
    raw=source.read_bytes()
    text=raw.decode('utf-8')
    if source.suffix in ('.txt','.log'):
        original_logs.append({'path':relative,'sha256':sha(raw),'bytes':len(raw),'utf8':text})
        # 展示副本只规范换行/空白，原字节在上面的UTF-8字段中完整保留。
        display='\n'.join(line.rstrip() for line in text.splitlines()).rstrip()
        if display:
            display+='\n'
    else:
        display=text.replace('\r\n','\n')
        if display and not display.endswith('\n'):
            display+='\n'
        if display.encode('utf-8')!=raw:
            originals.append({'path':relative,'sha256':sha(raw),'bytes':len(raw),'utf8':text})
    target=OUT/relative
    target.parent.mkdir(parents=True,exist_ok=True)
    target.write_text(display,encoding='utf-8',newline='\n')
    publication[relative]={'original_path':str(source),'original_sha256':sha(raw),
                           'original_bytes':len(raw),'publication_sha256':sha(target.read_bytes())}


for source_folder,target_folder in [('.local/round6-checks-9673c29','checks'),('.local/round6-matrix','matrix'),('.local/round6-red','red'),('.local/round6-before','baseline')]:
    for path in sorted((ROOT/source_folder).rglob('*')):
        if path.is_file() and path.suffix!='.exe':
            copy(path,target_folder+'/'+path.relative_to(ROOT/source_folder).as_posix())
for name in ['LICENSE.txt','NOTICE.txt','source-lock.json','manifest.json','quality.json']:
    source=ROOT/'.local/real-lexicon/cccedict-round4-2401b16'/name
    raw=source.read_bytes()
    if name.endswith('.txt'):
        originals.append({'path':'cccedict-source/'+name,'sha256':sha(raw),'bytes':len(raw),'utf8':raw.decode('utf-8')})
    else:
        copy(source,'cccedict-source/'+name)
copy(ROOT/'.local/real-lexicon/cccedict-round4-evidence.json','cccedict-source/evidence.json')
copy(ROOT/'.local/run_round6_matrix.py','tools/run_round6_matrix.py')
copy(ROOT/'.local/run_round6_checks.py','tools/run_round6_checks.py')
copy(Path(__file__),'tools/publish_round6.py')
save(OUT/'original-logs.json',{'encoding':'UTF-8; encode utf8 without newline conversion to reconstruct original bytes','files':original_logs})
save(OUT/'original-artifacts.json',{'encoding':'UTF-8; encode utf8 without newline conversion to reconstruct original bytes','files':originals})
paths=subprocess.check_output(['git','ls-tree','-r','--name-only',BASE,'docs/reports'],cwd=ROOT).decode().splitlines()
changed=subprocess.check_output(['git','diff','--name-only',BASE,'--',*paths],cwd=ROOT).decode().splitlines()
assert not changed,changed
history=[{'path':path,'git_blob_sha1':subprocess.check_output(['git','rev-parse',BASE+':'+path],cwd=ROOT).decode().strip()} for path in paths]
save(OUT/'history-preserved.json',{'starting_head':BASE,'unchanged_reports':history})
summary=json.loads((OUT/'matrix/summary.json').read_text(encoding='utf-8'))
validation=json.loads((OUT/'checks/validation.json').read_text(encoding='utf-8'))
identity=summary['identity']
assert identity['implementation_commit']==validation['implementation_commit']==IMPLEMENTATION
assert source_sha256()==identity['source_sha256']==validation['source_sha256']==validation['source_after_sha256']
assert not summary['failures']
index=[]
for label,run in summary['runs'].items():
    for version in ['before','after']:
        detail=json.loads((OUT/'matrix'/run[version]['path']).read_text(encoding='utf-8'))
        for case in detail['cases']:
            stats=case['result'].get('statistics',{})
            if case['result'].get('status')=='rejected' or stats.get('budget') or stats.get('beam_pruned'):
                index.append({'configuration':label,'version':version,'id':case['id'],'status':case['result']['status'],
                              'budget':stats.get('budget'),'beam_pruned':stats.get('beam_pruned'),
                              'report':run[version]['path'],'report_sha256':run[version]['sha256']})
save(OUT/'failure-and-budget-index.json',{'cases':index,'all_failures_retained':True,'count':len(index)})
save(OUT/'provenance.json',{'schema_version':1,'implementation_commit':IMPLEMENTATION,
    'source_fingerprint_schema':identity['source_fingerprint_schema'],'source_sha256':identity['source_sha256'],
    'baseline':identity['baseline'],'binary_sha256':identity['current_binary_sha256'],'runtime_sha256':identity['runtime_sha256'],
    'publication':publication,'historical_reports_unchanged':len(paths),'raw_log_count':len(original_logs),'raw_artifact_count':len(originals),
    'language_accuracy_valid':False,'single_algorithm_allocation_live_rss_measured':False})
print(json.dumps({'published':len(publication),'history':len(paths),'failures_incomplete_index':len(index),'raw_logs':len(original_logs),'raw_artifacts':len(originals)}))
