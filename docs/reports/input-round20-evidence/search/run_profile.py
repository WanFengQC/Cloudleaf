from pathlib import Path
import subprocess,json,hashlib,time
p=Path(__file__).resolve().parent;r=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
runtime=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
sha=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
out=p/'profile';out.mkdir(exist_ok=False)
records=[]
for kind,file in [('addone','character.tsv'),('witten','character-witten.tsv')]:
    model=r/'.local/round16-language'/file
    command=[str(p/'source/target/release/r19-search-probe.exe'),str(runtime),sha(runtime),str(model),str(r/'docs/reports/input-round18-evidence/performance/abbreviated-cases.tsv'),'abbreviated']
    now=time.perf_counter();result=subprocess.run(command,capture_output=True,cwd=p)
    for name,raw in [('stdout',result.stdout),('stderr',result.stderr)]: (out/(kind+'.'+name)).write_bytes(raw)
    records.append({'kind':kind,'returncode':result.returncode,'seconds':time.perf_counter()-now,'command':command,'runtime_sha':sha(runtime),'model_sha':sha(model),'binary_sha':sha(command[0]),'source_sha':sha(p/'source/crates/cloudleaf-search/src/search.rs')})
(out/'summary.json').write_text(json.dumps(records,indent=2),encoding='utf-8')
print(result.stderr.decode('utf-8'))
