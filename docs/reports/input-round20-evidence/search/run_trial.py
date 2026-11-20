from pathlib import Path
from datetime import datetime,timezone
import subprocess,json,hashlib,time,sys,re
p=Path(__file__).resolve().parent;r=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
runtime=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
sha=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
out=p/sys.argv[1];out.mkdir(exist_ok=False)
records=[]
for kind,file in [('lexical','../round15-language/signal.tsv'),('addone','character.tsv'),('witten','character-witten.tsv')]:
    model=r/'.local/round16-language'/file
    command=[str(p/(sys.argv[2] if len(sys.argv)>2 else 'source/target/release/r19-search-probe.exe')),str(runtime),sha(runtime),str(model),str(r/'docs/reports/input-round18-evidence/performance/abbreviated-cases.tsv'),'abbreviated']
    now=time.perf_counter();result=subprocess.run(command,capture_output=True,cwd=p)
    for name,raw in [('stdout',result.stdout),('stderr',result.stderr)]: (out/(kind+'.'+name)).write_bytes(raw)
    rows=[]
    for line in result.stdout.decode('utf-8').splitlines():
        a=line.split('\t'); rows.append({'case':a[1],'raw':a[2],'lattice_ns':int(a[3].split('=')[1]),'search_ns':int(a[4].split('=')[1]),'budget':re.search(r'budget: (None|Some\(\w+\))',line)[1]})
    records.append({'kind':kind,'returncode':result.returncode,'seconds':time.perf_counter()-now,'command':command,'queries':rows,'runtime_sha':sha(runtime),'model_sha':sha(model),'binary_sha':sha(command[0]),'source_sha':sha(p/'source/crates/cloudleaf-search/src/search.rs')})
    print(kind,[(a['case'],round((a['search_ns']+a['lattice_ns'])/1e6,3),a['budget'])for a in rows])
(out/'summary.json').write_text(json.dumps({'created_utc':datetime.now(timezone.utc).isoformat(),'runs':records},indent=2),encoding='utf-8')
