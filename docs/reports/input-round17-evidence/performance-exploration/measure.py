from pathlib import Path
import hashlib
import json
import math
import statistics
import subprocess
import time

root=Path(__file__).resolve().parent
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
runtime=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
runtime_sha='03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
assert hashlib.sha256(runtime.read_bytes()).hexdigest()==runtime_sha
rows=Path('C:/Users/EDY/.codex/tmp/r16-phase-final-f2/cases.tsv').read_text(encoding='utf-8').splitlines()
for line in (repo/'docs/reports/input-round16-evidence/corpus/word-contrast/development-tasks.jsonl').read_text(encoding='utf-8').splitlines():
    task=json.loads(line)
    if task['status']=='ready':rows.append('\t'.join([task['case'],task['case'],task['pinyin'],'engineering-source-dev']))
cases=root/'cases.tsv';cases.write_text('\n'.join(rows)+'\n',encoding='utf-8',newline='\n')
assert len(rows)==27
out=root/'measurements';out.mkdir(exist_ok=False)
results=[]
variants=['baseline','one-allocation','prefilter']
for iteration in range(1,4):
    order=variants[iteration%3:]+variants[:iteration%3]
    for kind,filename in [('addone','character.tsv'),('witten','character-witten.tsv')]:
        for variant in order:
            exe=root/variant/'target/release/r17-perf-probe.exe'
            model=repo/'.local/round16-language'/filename
            command=[str(exe),str(runtime),str(cases),'exact','character',str(model),runtime_sha,'64','5','1','16384','200000','50000','10']
            start=time.perf_counter();result=subprocess.run(command,cwd=root,capture_output=True)
            name=f'{iteration}-{kind}-{variant}'
            (out/(name+'.stdout.json')).write_bytes(result.stdout);(out/(name+'.stderr.txt')).write_bytes(result.stderr)
            metadata={'iteration':iteration,'kind':kind,'variant':variant,'command':command,'returncode':result.returncode,'seconds':time.perf_counter()-start,'binary_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'runtime_sha256':runtime_sha,'model_sha256':hashlib.sha256(model.read_bytes()).hexdigest(),'cases_sha256':hashlib.sha256(cases.read_bytes()).hexdigest(),'stdout_sha256':hashlib.sha256(result.stdout).hexdigest()}
            if result.returncode:
                print(result.stderr.decode('utf-8',errors='replace'));raise RuntimeError(name)
            raw=json.loads(result.stdout)
            assert all(all(s['equal'] for s in c['samples']) for c in raw['cases'])
            metadata['cases']=raw['cases'];results.append(metadata)
            print(json.dumps({k:metadata[k] for k in ['iteration','kind','variant','returncode','seconds']}),flush=True)
comparisons=0
for iteration in range(1,4):
    for kind in ['addone','witten']:
        base=next(r for r in results if r['iteration']==iteration and r['kind']==kind and r['variant']=='baseline')
        for variant in variants[1:]:
            changed=next(r for r in results if r['iteration']==iteration and r['kind']==kind and r['variant']==variant)
            for a,b in zip(base['cases'],changed['cases']):
                assert a['id']==b['id']
                assert a['result']==b['result'] and a['result_debug']==b['result_debug'],(iteration,kind,variant,a['id'])
                comparisons+=1
summary={}
for kind in ['addone','witten']:
    summary[kind]={}
    for variant in variants:
        selected=[r for r in results if r['kind']==kind and r['variant']==variant]
        times=[s['plain_ns'] for r in selected for c in r['cases'] for s in c['samples']]
        sorted_times=sorted(times)
        summary[kind][variant]={'samples':len(times),'p50_ns':sorted_times[math.ceil(len(times)*.5)-1],'p95_ns':sorted_times[math.ceil(len(times)*.95)-1],'p99_ns':sorted_times[math.ceil(len(times)*.99)-1],'max_ns':max(times),
            'per_case_median_ns':{c['id']:statistics.median(s['plain_ns'] for r in selected for x in r['cases'] if x['id']==c['id'] for s in x['samples']) for c in selected[0]['cases']}}
report={'source_base':'521472db18ecd7926f6e5b58b0afd7b5da674a07','comparisons_full_result_including_statistics':comparisons,'all_equal':True,'results':summary,'commands':[{k:v for k,v in r.items() if k!='cases'} for r in results],'scope':'Pure query including lattice, 25 fixed cases from R16, 3 fresh processes x5 timed samples; warm1 percase, alltimed samples retained; exploratory background load not guaranteed idle, not InputEngine/Host/TSF P95.'}
(out/'summary.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({k:report[k] for k in ['comparisons_full_result_including_statistics','all_equal','results']},indent=2))
