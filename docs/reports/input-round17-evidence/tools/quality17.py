from pathlib import Path
import hashlib, json, re, subprocess, sys
root=Path(__file__).resolve().parents[1]
out=root/'.local/round17-quality';out.mkdir(exist_ok=False)
runtime=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
supp=root/'.local/round17-supplement-a'
models=[root/'.local/round15-language/signal.tsv',root/'.local/round16-language/character.tsv',root/'.local/round16-language/character-witten.tsv']
rebound=[supp/'model-2.tsv',supp/'model-0.tsv',supp/'model-1.tsv']
targets={'r16-all32':Path('C:/Users/EDY/.codex/tmp/r17-data/traditional-b/r16-combined-targets.tsv'),'news-dev':Path('C:/Users/EDY/.codex/tmp/r17-data/final-audit/dev-targets.tsv')}
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
rows=[]
for corpus,target in targets.items():
    texts={f[0]:f[2] for line in target.read_text(encoding='utf-8').splitlines() if line and not line.startswith('#') and (f:=line.split('\t'))[0]!='case'}
    for layer,lex,modelset in [('baseline',runtime,models),('supplement',supp/'combined-runtime.tsv',rebound)]:
        for kind,model in zip(['lexical','addone','witten'],modelset):
            label=f'quality-{corpus}-{layer}-{kind}'
            command=[str(root/'.local/build/release/examples/coverage_probe.exe'),str(lex),sha(lex),str(model),sha(model),str(target)]
            p=subprocess.run([sys.executable,'-X','utf8','.local/run17.py',label,*command],cwd=root,capture_output=True)
            if p.returncode:raise RuntimeError(p.stdout.decode('utf-8',errors='replace')[-1000:]+p.stderr.decode('utf-8',errors='replace')[-1000:])
            log=(root/'.local/round17-runs'/label/'stdout.txt').read_text(encoding='utf-8')
            cases={}
            for line in log.splitlines():
                f=line.split('\t')
                if f[0]=='CASE': cases[f[1]]={'target':texts[f[1]],'case_diagnostic':f[4:]}
                if f[0]=='RESULT':
                    candidates=[json.loads(v) for v in re.findall(r'Candidate \{ text: ("(?:[^"\\]|\\.)*")',f[2])]
                    cases[f[1]].update({'texts':candidates,'source_rank':candidates.index(texts[f[1]])+1 if texts[f[1]] in candidates else None,'statistics':f[2].split('statistics: ')[-1],'unknown_counts':[int(n) for n in re.findall(r'character_unknown: (\d+)',f[2])]})
            row={'corpus':corpus,'layer':layer,'kind':kind,'runtime_sha256':sha(lex),'model_sha256':sha(model),'target_sha256':sha(target),'tasks':len(texts),'cases':cases,'source_first':sum(v.get('source_rank')==1 for v in cases.values()),'source_top5':sum(0<v.get('source_rank',0)<=5 for v in cases.values() if v.get('source_rank')),'source_top10':sum(v.get('source_rank') is not None for v in cases.values()),'target_reachable':sum(any('target_paths_capped_u64=' in d and not d.endswith('=0') for d in v['case_diagnostic']) for v in cases.values()),'budget_failures':sum('budget: Some' in v.get('statistics','') for v in cases.values()),'raw_log':str(root/'.local/round17-runs'/label),'formal_top1':None,'formal_top5':None,'formal_mrr':None}
            rows.append(row)
            (out/'summary.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
            print(json.dumps({k:v for k,v in row.items() if k!='cases'},ensure_ascii=False),flush=True)
