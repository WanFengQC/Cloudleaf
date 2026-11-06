from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
import hashlib,json,os,subprocess,time

root=Path(__file__).resolve().parent
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
def compile(label):
    directory=root/label; p=directory/'probe/src/bin';p.mkdir(exist_ok=True)
    (p/'allocations.rs').write_bytes((root/'allocations.rs').read_bytes())
    env=os.environ.copy();env['CARGO_TARGET_DIR']=str(directory/'target')
    command=['C:/Users/EDY/.cargo/bin/cargo.exe','build','--release','--locked','--offline','--bin','allocations']
    proc=subprocess.run(command,cwd=directory,env=env,capture_output=True)
    (directory/'allocations-build.stdout').write_bytes(proc.stdout);(directory/'allocations-build.stderr').write_bytes(proc.stderr)
    return {'variant':label,'command':command,'returncode':proc.returncode}
with ThreadPoolExecutor(max_workers=3) as pool:compiled=list(pool.map(compile,['baseline','one-allocation','prefilter']))
assert all(r['returncode']==0 for r in compiled),compiled
records=[]
for label in ['baseline','one-allocation','prefilter']:
    for kind,file in [('addone','character.tsv'),('witten','character-witten.tsv')]:
        exe=root/label/'target/release/allocations.exe'
        command=[str(exe),'C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv',str(repo/'.local/round16-language'/file),'03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431',str(root/'cases.tsv')]
        result=subprocess.run(command,capture_output=True)
        (root/f'allocations-{label}-{kind}.tsv').write_bytes(result.stdout);(root/f'allocations-{label}-{kind}.stderr').write_bytes(result.stderr)
        assert result.returncode==0
        records.append({'variant':label,'kind':kind,'command':command,'returncode':result.returncode,'probe_sha256':hashlib.sha256((root/'allocations.rs').read_bytes()).hexdigest(),'stdout_sha256':hashlib.sha256(result.stdout).hexdigest()})
(root/'allocations.json').write_text(json.dumps({'compile':compiled,'runs':records,'scope':'Temporary System allocator wrapper counts allocations/reallocations during search on prebuilt lattice. Cumulative requested bytes are not live/peak allocated memory. No allocator or unsafe code added to product.'},indent=2)+'\n',encoding='utf-8')
print(json.dumps(compiled,indent=2))
