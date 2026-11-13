"""同一固定输入、词典与模型的实际Rust搜索探针。"""
from pathlib import Path
from datetime import datetime,timezone
import hashlib,json,subprocess,sys,time
out=Path(__file__).resolve().parent
root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
label=sys.argv[1];folder=out/label;folder.mkdir(exist_ok=False)
runtime=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
models={'addone':root/'.local/round16-language/character.tsv','witten':root/'.local/round16-language/character-witten.tsv'}
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
exe=out/'source/target/release/r19-search-probe.exe'
cases=root/'docs/reports/input-round18-evidence/performance/abbreviated-cases.tsv'
records=[]
for kind,model in models.items():
    args=[str(exe),str(runtime),sha(runtime),str(model),str(cases)]
    start=time.perf_counter()
    result=subprocess.run(args,cwd=out,capture_output=True)
    (folder/(kind+'.stdout')).write_bytes(result.stdout);(folder/(kind+'.stderr')).write_bytes(result.stderr)
    lines=result.stdout.decode('utf-8').splitlines()
    failures=[line.split('\t')[1]for line in lines if'budget: Some('in line]
    records.append({'kind':kind,'command':args,'returncode':result.returncode,'seconds':time.perf_counter()-start,'cases':len(lines),'budget_cases':failures,'model_sha256':sha(model),'binary_sha256':sha(exe),'runtime_sha256':sha(runtime),'case_file_sha256':sha(cases),'source_sha256':sha(out/'source/crates/cloudleaf-search/src/search.rs'),'stdout_sha256':sha(folder/(kind+'.stdout')),'stderr_sha256':sha(folder/(kind+'.stderr'))})
record={'label':label,'created_utc':datetime.now(timezone.utc).isoformat(),'runs':records,'tool_sha256':sha(__file__),'formal_metrics':None}
(folder/'summary.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'label':label,'runs':[{k:r[k]for k in ['kind','returncode','cases','budget_cases']}for r in records]}))
