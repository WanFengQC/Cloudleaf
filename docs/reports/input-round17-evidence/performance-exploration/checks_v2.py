from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
import gzip
import hashlib
import json
import os
import subprocess
import time

root=Path(__file__).resolve().parent
cargo='C:/Users/EDY/.cargo/bin/cargo.exe'
def run(label):
    directory=root/label
    out=directory/'checks-v2';out.mkdir(exist_ok=False)
    commands=[];env=os.environ.copy();env['CARGO_TARGET_DIR']=str(directory/'target')
    for name,args in [('lock',['generate-lockfile','--offline']),('unit',['test','-p','cloudleaf-search','--locked','--offline']),('differential',['run','--release','--locked','--offline','-p','r17-diff'])]:
        command=[cargo,*args];start=time.perf_counter()
        with (out/(name+'.stdout')).open('wb') as stdout,(out/(name+'.stderr')).open('wb') as stderr:
            result=subprocess.run(command,cwd=directory,env=env,stdout=stdout,stderr=stderr)
        commands.append({'command':command,'returncode':result.returncode,'seconds':time.perf_counter()-start})
        if result.returncode:break
    raw=(out/'differential.stdout').read_bytes() if (out/'differential.stdout').exists() else b''
    packed=gzip.compress(raw,mtime=0);(out/'differential.stdout.gz').write_bytes(packed)
    report={'variant':label,'commands':commands,'summary_lines':[line for line in raw.decode('utf-8').splitlines() if line.startswith('SUMMARY\t')],
        'source_sha256':{str(p.relative_to(directory)):hashlib.sha256(p.read_bytes()).hexdigest() for p in (directory/'crates/cloudleaf-search/src').glob('*.rs')},
        'raw_sha256':hashlib.sha256(raw).hexdigest(),'raw_bytes':len(raw),'gzip_sha256':hashlib.sha256(packed).hexdigest(),'gzip_bytes':len(packed),'gzip_validated':gzip.decompress(packed)==raw}
    (out/'checks.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    return report
with ThreadPoolExecutor(max_workers=2) as pool:reports=list(pool.map(run,['one-allocation','prefilter']))
print(json.dumps(reports,indent=2))
raise SystemExit(any(any(c['returncode'] for c in r['commands']) for r in reports))
