"""最终普通push与精确CI核验完成后，创建一次Git外交付回执。"""
from pathlib import Path
from datetime import datetime,timezone
import hashlib,json,subprocess,gzip,zipfile,platform
root=Path(__file__).resolve().parents[1]
def git(*args):return subprocess.check_output(['git',*args],cwd=root).decode('utf-8').strip()
def read(name):return json.loads((root/name).read_text(encoding='utf-8'))
def identity(p):
    p=Path(p);return {'path':str(p),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
base='794d32c1818c4e1dfd2e504ea4369abdb3621c2a';impl='b948f986c4c9d00bec7cdb7845283c83e6a87970'
head=git('rev-parse','HEAD')
assert git('branch','--show-current')=='codex/input-round14' and not git('status','--porcelain')
assert git('merge-base',base,head)==base and git('rev-parse','origin/codex/input-round14')==head
assert not git('diff','--name-only',impl,head,'--','crates','adapters','apps','scripts','tests','Cargo.toml','Cargo.lock','.github','README.md','THIRD_PARTY_NOTICES.md','data','licenses','docs/input-round18-design.md')
remote=read('.local/round18-remote/final/summary.json')
assert remote['main']==git('rev-parse','main')==git('rev-parse','origin/main')=='a564fd47b0f054ff360592af3ccb8d744021160f'
assert remote['branch']==remote['local_head']==head
baseline=read('.local/round18-remote/baseline/summary.json')
assert len(remote['pulls'])==5 and all(p['draft'] and p['state']=='open' and not p['merged'] for p in remote['pulls'])
assert [(p['number'],p['head'])for p in baseline['pulls']]==[(p['number'],p['head'])for p in remote['pulls']]
ci=read('.local/round18-ci/final/summary.json');downloads=read('.local/round18-ci/final-downloads/downloads.json')
assert ci['verified_pass'] and ci['head_sha']==head==downloads['head']
assert len(ci['jobs'])==2 and all(j['steps']==22 for j in ci['jobs'])
assert len(downloads['files'])==4 and all(v['crc']=='passed' for v in downloads['files'])
for row in downloads['files']:
    p=root/'.local/round18-ci/final-downloads'/row['file'];assert identity(p)['sha256']==row['sha256']
    with zipfile.ZipFile(p)as z:assert z.testzip()is None
artifact=next(v for v in downloads['files']if v['file']=='input-round18-windows.zip')
names=[r['name']for r in artifact['members']]
for name in ['cloudleaf-cli.exe','cloudleaf-ime.exe','cloudleaf_windows_ime.dll','input_trace.exe','scheme_trace.exe','train_language.exe','coverage_probe.exe']:assert any(n.endswith(name)for n in names),name
assert not any(Path(n).name in ['runtime.tsv','combined-runtime.tsv','character.tsv','model-0.tsv','settings.json','pytorch_model.bin','model.safetensors']for n in names)
ev=root/'docs/reports/input-round18-evidence';index=read('docs/reports/input-round18-evidence/index.json')
for name,row in index['files'].items():
    p=ev/name;assert identity(p)['sha256']==row['sha256'],name
    assert git('rev-parse','HEAD:'+str(p.relative_to(root)).replace('\\','/'))==git('hash-object','--no-filters',str(p)),name
    if p.suffix=='.gz':gzip.decompress(p.read_bytes())
commands=read('.local/round18-verified-results.json');assert len(commands)==13 and all(r['returncode']==0 for r in commands)
for name,value in read('.local/round18-runs/verified-04/result.json')['source_files_sha256'].items():assert identity(root/name)['sha256']==value,name
preservation=read('docs/reports/input-round18-evidence/preservation.json')
assert preservation['historical_paths_unchanged']==2793 and preservation['old_rust_tests_retained']==230
perf=json.loads(gzip.decompress((ev/'performance/summary.json.gz').read_bytes()))
assert all(identity(root/n)['sha256']==v for n,v in perf['source_after'].items())
for name in ['round18-ui-final','round18-ui-correction','round18-ui-mixed']:
    completion=read('.local/'+name+'/completion.json');assert completion['closed_owned_window_observed'] and not completion['TSF']
record={'schema_version':1,'round':18,'status':'partial','created_utc':datetime.now(timezone.utc).isoformat(),'branch':'codex/input-round14','base':base,'implementation_head':impl,'final_head':head,'clean_worktree':True,'main_unchanged':True,'PRs':remote['pulls'],'CI':ci,'downloaded_CI':downloads,'local_checks':commands,'tests':{'rust_passed':254,'rust_ignored':2,'rust_failed':0,'python_passed':71},'evidence':{'index':identity(ev/'index.json'),'verified_files':len(index['files']),'preservation':preservation},'production_sources_match_full_local_checks':True,'performance_core_source_match':True,'data':read('docs/reports/input-round18-evidence/data-identities.json'),'binaries':read('docs/reports/input-round18-evidence/binary-identities.json'),'differential':{'new_reference_queries':33480,'new_complete_comparisons':83700,'legacy_reference_queries':22320,'legacy_complete_comparisons':44640,'final_failures':0,'original_provenance_failures_retained':592},'performance_gate':'not passed: default character P95 +7.85%/+6.73%; scheme abbreviation budgets90/837 final profiles; full raw tails retained','actual_window_tests':'full/abbreviation/paging/digit/Xiaohe/switch/fuzzy/correction/middle-edit/mouse/Enter/Escape/raw fallback/Unknown/new-session no replay; owned windows only','formal_metrics':{'top1':None,'top5':None,'mrr':None},'research_only':True,'production_approved':False,'default_input_engine':'legacy/full unchanged','not_implemented':['Microsoft/Natural Code layouts','Xiaohe correction','fuzzy incomplete prefixes','user lexicon persistence/learning','incremental preemption'],'not_run':['TSF install/register/activate/uninstall','8h/24h soak','formal Windows application acceptance','independent human-gold quality assessment'],'CL010':'closed','PR6_created':False,'report':identity(root/'docs/reports/input-round18-status.md'),'tool':identity(__file__),'environment':{'platform':platform.platform(),'python':platform.python_version()}}
dest=root/'.local/round18-delivery-receipt.json'
with dest.open('x',encoding='utf-8',newline='\n')as f:json.dump(record,f,ensure_ascii=False,indent=2);f.write('\n')
print(json.dumps({'head':head,'status':record['status'],'receipt':str(dest),'CI':ci['url'],'evidence_files':len(index['files'])},ensure_ascii=False))
