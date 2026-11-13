"""固定原20案例，新旧实际Rust查询配对；资源仅整进程采样。"""
from pathlib import Path
from datetime import datetime,timezone
import ctypes,gzip,hashlib,json,math,re,subprocess,time
from ctypes import wintypes
p=Path(__file__).resolve().parent;root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=p/'paired-final';out.mkdir(exist_ok=False)
runtime=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
cases=root/'docs/reports/input-round18-evidence/performance/abbreviated-cases.tsv'
models={'lexical':root/'.local/round15-language/signal.tsv','addone':root/'.local/round16-language/character.tsv','witten':root/'.local/round16-language/character-witten.tsv'}
bins={'old':p/'baseline-source-finish/target/release/r19-search-probe.exe','new':p/'source/target/release/r19-search-probe.exe'}
sha=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
class Memory(ctypes.Structure):
    _fields_=[('cb',wintypes.DWORD),('faults',wintypes.DWORD)]+[(n,ctypes.c_size_t)for n in ['peak','working','peak_paged','paged','peak_nonpaged','nonpaged','pagefile','peak_pagefile','private']]
api=ctypes.WinDLL('psapi',use_last_error=True);api.GetProcessMemoryInfo.argtypes=[wintypes.HANDLE,ctypes.POINTER(Memory),wintypes.DWORD]
source={n:sha(p/'source/crates/cloudleaf-search/src'/n)for n in ['search.rs','character.rs','lexicon.rs','layer.rs','model.rs']}
records=[]
for iteration in range(1,4):
    for scheme in ['full','abbreviated']:
        for kind,model in models.items():
            for variant in (['old','new']if iteration%2 else['new','old']):
                folder=out/f'{iteration}-{scheme}-{kind}-{variant}';folder.mkdir()
                command=[str(bins[variant]),str(runtime),sha(runtime),str(model),str(cases),scheme]
                samples=[];start=time.perf_counter()
                with(folder/'stdout.txt').open('wb')as stdout,(folder/'stderr.txt').open('wb')as stderr:
                    child=subprocess.Popen(command,cwd=p,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
                    while child.poll()is None:
                        value=Memory();value.cb=ctypes.sizeof(value)
                        if api.GetProcessMemoryInfo(int(child._handle),ctypes.byref(value),value.cb):samples.append({'t':time.perf_counter()-start,'working':value.working,'private':value.private,'peak_working':value.peak})
                        time.sleep(.05)
                    code=child.wait()
                rows=[]
                for line in(folder/'stdout.txt').read_text(encoding='utf-8').splitlines():
                    fields=line.split('\t',5);assert fields[0]=='CASE'
                    found=re.search(r'search_ns=(\d+)\t(.*)',fields[5]if len(fields)>5 else fields[4])
                    if found is None:found=re.search(r'search_ns=(\d+)\t(.*)',line)
                    assert found,line[:200]
                    result=found[2]
                    rows.append({'case':fields[1],'raw':fields[2],'lattice_ns':int(fields[3].split('=')[1]),'search_ns':int(found[1]),'budget':re.search(r'budget: (None|Some\(\w+\))',result)[1],'result':result})
                assert len(rows)==20
                record={'iteration':iteration,'scheme':scheme,'kind':kind,'variant':variant,'command':command,'returncode':code,'seconds':time.perf_counter()-start,'binary_sha256':sha(bins[variant]),'runtime_sha256':sha(runtime),'model_sha256':sha(model),'case_file_sha256':sha(cases),'resources_scope':'Whole process: raw loaded originals, model, sharedindex, search, diagnostics; not algorithm allocation or Host budget','samples':samples,'queries':rows,'stdout_sha256':sha(folder/'stdout.txt'),'stderr_sha256':sha(folder/'stderr.txt')}
                (folder/'result.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');records.append(record)
                print(json.dumps({'iteration':iteration,'scheme':scheme,'kind':kind,'variant':variant,'returncode':code,'queries':len(rows),'budgets':sum(r['budget']!='None'for r in rows)}),flush=True)
summary={'at_utc':datetime.now(timezone.utc).isoformat(),'base':'cf54c84d400b627dfa9b246b3800a82d65f061b8','source':source,'binary':{k:{'path':str(v),'sha256':sha(v)}for k,v in bins.items()},'runtime':{'path':str(runtime),'sha256':sha(runtime)},'cases':{'path':str(cases),'sha256':sha(cases)},'models':{k:{'path':str(v),'sha256':sha(v)}for k,v in models.items()},'scope':'Plain final-query K10 same20cases, lexical PositionDag, character ExactDag, same200000/50000budgets; 3freshprocesses/condition; no warmup or tail discarded. Ordinary Windows desktop; no realtime/TSF acceptance. Not R18逐键profile denominator.','formal_metrics':None,'runs':records,'metrics':{},'comparisons':[]}
def quantile(values):
    v=sorted(values);return{'n':len(v),**{f'p{q}':v[math.ceil(len(v)*q/100)-1]for q in [50,95,99]},'max':max(v)}
for scheme in ['full','abbreviated']:
    for kind in models:
        for variant in bins:
            chosen=[r for r in records if(r['scheme'],r['kind'],r['variant'])==(scheme,kind,variant)]
            key=f'{scheme}/{kind}/{variant}';rows=[q for r in chosen for q in r['queries']]
            metrics={'queries':len(rows),'budgets':sum(q['budget']!='None'for q in rows),'budget_cases':sorted({q['case']for q in rows if q['budget']!='None'}),'returncodes':[r['returncode']for r in chosen]}
            for name in ['lattice_ns','search_ns']:metrics[name]=quantile([q[name]for q in rows])
            metrics['total_query_ns']=quantile([q['lattice_ns']+q['search_ns']for q in rows])
            metrics['resource_max']={name:max(s[name]for r in chosen for s in r['samples'])for name in ['working','private','peak_working']}
            summary['metrics'][key]=metrics
        for iteration in range(1,4):
            pair={r['variant']:r for r in records if(r['iteration'],r['scheme'],r['kind'])==(iteration,scheme,kind)}
            for before,after in zip(pair['old']['queries'],pair['new']['queries']):
                assert before['case']==after['case']
                # 只剔除新增严格零值偏好字段；默认完整统计也须逐字保持。
                a=before['result'];b=after['result'].replace('user_preference_units: 0, ','')
                if scheme=='full':equal=a==b
                else:
                    a=a.split(', raw_fallback:',1)[0];b=b.split(', raw_fallback:',1)[0]
                    equal=a==b
                comparable=before['budget']=='None'and after['budget']=='None'
                summary['comparisons'].append({'iteration':iteration,'scheme':scheme,'kind':kind,'case':before['case'],'comparable':comparable,'equal':equal,'old_budget':before['budget'],'new_budget':after['budget']})
                if comparable:assert equal,(scheme,kind,before['case'])
summary['source_after']={n:sha(p/'source/crates/cloudleaf-search/src'/n)for n in source};assert summary['source_after']==source
(out/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'runs':len(records),'queries':sum(len(r['queries'])for r in records),'comparable':sum(r['comparable']for r in summary['comparisons']),'complete_differences':sum(r['comparable']and not r['equal']for r in summary['comparisons'])}))
