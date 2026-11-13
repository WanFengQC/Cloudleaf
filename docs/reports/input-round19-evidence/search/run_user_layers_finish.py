"""同一837最终查询在关闭、空层和固定规模合成用户层的实测。"""
from pathlib import Path
from datetime import datetime,timezone
from ctypes import wintypes
import ctypes,gzip,hashlib,json,math,re,subprocess,time
p=Path(__file__).resolve().parent;root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=p/'user-layer-performance';out.mkdir(exist_ok=False)
runtime=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
cases=root/'docs/reports/input-round18-evidence/performance'
models={'lexical':root/'.local/round15-language/signal.tsv','addone':root/'.local/round16-language/character.tsv','witten':root/'.local/round16-language/character-witten.tsv'}
exe=p/'user-layer-source/target/release/user_layer_probe.exe'
sha=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
class Memory(ctypes.Structure):
    _fields_=[('cb',wintypes.DWORD),('faults',wintypes.DWORD)]+[(n,ctypes.c_size_t)for n in ['peak','working','peak_paged','paged','peak_nonpaged','nonpaged','pagefile','peak_pagefile','private']]
api=ctypes.WinDLL('psapi',use_last_error=True);api.GetProcessMemoryInfo.argtypes=[wintypes.HANDLE,ctypes.POINTER(Memory),wintypes.DWORD]
records=[]
for kind,model in models.items():
    for size in [-1,0,4,512,4096]:
        folder=out/f'{kind}-{size}';folder.mkdir()
        command=[str(exe),str(runtime),sha(runtime),str(model),str(size),str(cases)]
        samples=[];now=time.perf_counter()
        with(folder/'stdout.txt').open('wb')as stdout,(folder/'stderr.txt').open('wb')as stderr:
            child=subprocess.Popen(command,cwd=p,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
            while child.poll()is None:
                value=Memory();value.cb=ctypes.sizeof(value)
                if api.GetProcessMemoryInfo(int(child._handle),ctypes.byref(value),value.cb):samples.append({'t':time.perf_counter()-now,'working':value.working,'private':value.private,'peak_working':value.peak})
                time.sleep(.05)
            code=child.wait()
        rows=[];identity=None;user_tsv=None
        for line in(folder/'stdout.txt').read_text(encoding='utf-8').splitlines():
            fields=line.split('\t')
            if fields[0]=='IDENTITY':identity={k:int(v)if v.lstrip('-').isdigit()else v for k,v in (field.split('=',1)for field in fields[1:])}
            elif fields[0]=='USER_TSV':user_tsv=line
            elif fields[0]=='CASE':rows.append({'iteration':int(fields[1]),'scheme':fields[2],'case':fields[3],'raw':fields[4],'lattice_ns':int(fields[5].split('=')[1]),'search_ns':int(fields[6].split('=')[1]),'budget':re.search(r'budget: (None|Some\(\w+\))',fields[7])[1],'result':fields[7]})
        record={'kind':kind,'size':size,'command':command,'at_utc':datetime.now(timezone.utc).isoformat(),'returncode':code,'seconds':time.perf_counter()-now,'identity':identity,'user_fixture_debug_sha256':hashlib.sha256((user_tsv or'').encode()).hexdigest(),'samples':samples,'queries':rows,'runtime_sha256':sha(runtime),'model_sha256':sha(model),'binary_sha256':sha(exe),'stdout_sha256':sha(folder/'stdout.txt'),'stderr_sha256':sha(folder/'stderr.txt'),'scope':'Whole-process memory includes simultaneousbase/combined/model/syntheticoriginals. No Host budget admission or peralgorithm memory claim.'}
        (folder/'result.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');records.append(record)
        print(json.dumps({'kind':kind,'size':size,'returncode':code,'queries':len(rows),'budgets':sum(r['budget']!='None'for r in rows),'identity':identity}),flush=True)
        assert code==0 and len(rows)==279
summary={'created_utc':datetime.now(timezone.utc).isoformat(),'scope':'15processes, each279queries=20/13fixedcases×5schemes×3rounds. OFF/empty/4/512/4096syntheticmanualwords;ascendingIDs,seed0,exactsameR18inputs. PlainfinalqueryK10;freshloadpermodel/size,roundswithinprocess;no discardedwarmup/tails. NotEngine/Host/TSF or production-language quality.','source':json.loads((p/'user-layer-source-identity.json').read_text(encoding='utf-8')),'runs':records,'metrics':{},'empty_layer_candidate_equality':[],'formal_metrics':None,'tool_sha256':sha(__file__)}
def q(v):
    values=sorted(v);return{'n':len(values),**{f'p{n}':values[math.ceil(len(values)*n/100)-1]for n in [50,95,99]},'max':max(values)}
for record in records:
    for scheme in ['full','abbreviated','xiaohe','fuzzy-nl','correction']:
        rows=[r for r in record['queries']if r['scheme']==scheme]
        summary['metrics'][f'{record["kind"]}/{record["size"]}/{scheme}']={'queries':len(rows),'budget_failures':sum(r['budget']!='None'for r in rows),'budget_cases':sorted({r['case']for r in rows if r['budget']!='None'}),'lattice_ns':q([r['lattice_ns']for r in rows]),'search_ns':q([r['search_ns']for r in rows]),'total_query_ns':q([r['lattice_ns']+r['search_ns']for r in rows]),'resource_max':{name:max(s[name]for s in record['samples'])for name in ['working','private','peak_working']},'returncode':record['returncode']}
for kind in models:
    before=next(r for r in records if r['kind']==kind and r['size']==-1)
    after=next(r for r in records if r['kind']==kind and r['size']==0)
    for a,b in zip(before['queries'],after['queries']):
        assert (a['iteration'],a['scheme'],a['case'])==(b['iteration'],b['scheme'],b['case'])
        equal=a['result']==b['result']
        summary['empty_layer_candidate_equality'].append({'kind':kind,'iteration':a['iteration'],'scheme':a['scheme'],'case':a['case'],'equal':equal})
        assert equal,(kind,a['scheme'],a['case'])
(out/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'runs':len(records),'queries':sum(len(r['queries'])for r in records),'empty_layer_equal_queries':len(summary['empty_layer_candidate_equality']),'failures':sum(q['budget']!='None'for r in records for q in r['queries'])}))
