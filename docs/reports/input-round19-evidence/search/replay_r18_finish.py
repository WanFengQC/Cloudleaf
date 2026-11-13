"""原R18全部837个最终案例/配置复跑，不缩减失败分母。"""
from pathlib import Path
from datetime import datetime,timezone
import hashlib,json,re,subprocess,time
p=Path(__file__).resolve().parent;root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=p/'r18-final-replay';out.mkdir(exist_ok=False)
runtime=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
models={'lexical':root/'.local/round15-language/signal.tsv','addone':root/'.local/round16-language/character.tsv','witten':root/'.local/round16-language/character-witten.tsv'}
exe=p/'source/target/release/r19-search-probe.exe'
sha=lambda path:hashlib.sha256(Path(path).read_bytes()).hexdigest()
records=[]
for iteration in range(1,4):
    for scheme in ['full','abbreviated','xiaohe','fuzzy-nl','correction']:
        cases=root/f'docs/reports/input-round18-evidence/performance/{scheme}-cases.tsv'
        for kind,model in models.items():
            folder=out/f'{iteration}-{scheme}-{kind}';folder.mkdir()
            command=[str(exe),str(runtime),sha(runtime),str(model),str(cases),scheme]
            now=time.perf_counter();result=subprocess.run(command,cwd=p,capture_output=True)
            (folder/'stdout.txt').write_bytes(result.stdout);(folder/'stderr.txt').write_bytes(result.stderr)
            lines=result.stdout.decode('utf-8').splitlines();expected=13 if scheme=='xiaohe' else 20
            assert len(lines)==expected
            rows=[]
            for line in lines:
                fields=line.split('\t');assert fields[0]=='CASE'
                rows.append({'case':fields[1],'raw':fields[2],'lattice_ns':int(fields[3].split('=')[1]),'search_ns':int(fields[4].split('=')[1]),'budget':re.search(r'budget: (None|Some\(\w+\))',line)[1]})
            record={'iteration':iteration,'scheme':scheme,'kind':kind,'command':command,'returncode':result.returncode,'seconds':time.perf_counter()-now,'runtime_sha256':sha(runtime),'model_sha256':sha(model),'cases_sha256':sha(cases),'binary_sha256':sha(exe),'source_sha256':{n:sha(p/'source/crates/cloudleaf-search/src'/n)for n in ['search.rs','character.rs','lexicon.rs','layer.rs']},'queries':rows,'stdout_sha256':sha(folder/'stdout.txt'),'stderr_sha256':sha(folder/'stderr.txt')}
            (folder/'result.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');records.append(record)
            print(json.dumps({'iteration':iteration,'scheme':scheme,'kind':kind,'returncode':result.returncode,'queries':len(rows),'budgets':sum(r['budget']!='None'for r in rows)}),flush=True)
rows=[{'iteration':run['iteration'],'scheme':run['scheme'],'kind':run['kind'],**row}for run in records for row in run['queries']]
assert len(rows)==837 and all(r['returncode']==0 for r in records)
failures=[r for r in rows if r['budget']!='None']
summary={'at_utc':datetime.now(timezone.utc).isoformat(),'scope':'SameR18 final20/13cases,5schemes,3models,3rounds; actualplain Rust finalqueryK10 andsame200000/50000budgets; not per-key or TSF; no Reference completeness assumption for long inputs','queries':837,'budget_failures':len(failures),'unique_failures':sorted({(r['scheme'],r['kind'],r['case'],r['budget'])for r in failures}),'failures':failures,'runs':records,'formal_metrics':None,'tool_sha256':sha(__file__)}
(out/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');print(json.dumps({'queries':837,'failures':len(failures),'unique':summary['unique_failures']}))
