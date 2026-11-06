from pathlib import Path
import gzip,hashlib,json,re,shutil,subprocess
root=Path(__file__).resolve().parents[1];out=root/'docs/reports/input-round17-evidence';out.mkdir(exist_ok=True)
tmp=Path('C:/Users/EDY/.codex/tmp')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def copy(p,name):
    p=Path(p);dest=out/name;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,dest)
def pack(p,name):
    p=Path(p);raw=p.read_bytes();z=gzip.compress(raw,mtime=0);assert gzip.decompress(z)==raw
    dest=out/(str(name)+'.gz');dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(z)
def tree(p,prefix,filter=lambda p:True,compress=True):
    p=Path(p)
    for file in sorted(p.rglob('*')):
        if file.is_file() and filter(file):
            name=Path(prefix)/file.relative_to(p)
            if compress and file.stat().st_size>100000 and file.suffix!='.gz':pack(file,name)
            else:copy(file,name)
tree(root/'.local/round17-runs','runs')
tree(root/'.local/round17-quality','quality')
tree(root/'.local/round17-quality-diagnosis','quality-diagnosis')
tree(root/'.local/round17-final-paired-traces','paired-traces')
for p in sorted((root/'.local').glob('round17-final-*')):
    if p.is_dir() and (p/'stdout.tsv').exists():tree(p,Path('traces')/p.name)
tree(root/'.local/round17-host-benchmark','host-benchmark')
for name in ['round17-ui-before-focus-fix','round17-ui-final','round17-ui-focus-target']:
    tree(root/'.local'/name,name)
for name in ['round17-delivery-results.json','round17-verified-results.json','round17-verified-dependency-audit.json','round17-verified-performance.json','round17-lexicon-receipt.json','round17-baseline-source.json','round17-baseline-build.stdout','round17-baseline-build.stderr','round17-baseline-remote.json','round17-final-prs.json']:
    copy(root/'.local'/name,name)
tree(root/'.local/round17-verified-algorithm-v1','algorithm-baseline')
for p in sorted((root/'.local').glob('*17*.py')):copy(p,Path('tools')/p.name)
copy(root/'.local/round17-operational-errors.md','tools/operational-errors.md')
for name in ['manifest.json','MODIFICATIONS.md','NOTICE.txt']:
    p=root/'.local/round17-supplement-a'/name
    if p.exists():copy(p,Path('supplement')/name)
supp=root/'.local/round17-supplement-a';other=root/'.local/round17-supplement-b'
identities={p.name:{'bytes':p.stat().st_size,'sha256':sha(p),'double_build_identical':p.read_bytes()==(other/p.name).read_bytes()} for p in supp.iterdir() if p.is_file()}
assert all(v['double_build_identical'] for k,v in identities.items() if k!='manifest.json')
(out/'supplement/identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8',newline='\n')
data=tmp/'r17-data'
tree(data/'final-audit-v2','corpus/audit')
for name in ['prepare_traditional_layer.py','prepare_wikinews_tasks.py','acquire_wikinews.py','acquire_wikinews-initial.py','resume_wikinews.py','finish_wikinews_audit.py','check_layer_replay.py','audit-execution-record.json','wikinews-initial-failure.json','traditional-reproducibility.json']:
    copy(data/name,Path('corpus/tools')/name)
for name in ['summary.json','leakage-report.json','frozen-source-slots.jsonl']:
    if (data/'wikinews-tasks-a'/name).exists():copy(data/'wikinews-tasks-a'/name,Path('corpus/frozen')/name)
copy(data/'traditional-c/r16-all32-diagnosis.jsonl','corpus/r16-all32-diagnosis.jsonl')
copy(data/'traditional-b/r16-combined-targets.tsv','corpus/r16-combined-targets.tsv')
for p in sorted((data/'wikinews-originals').iterdir()):
    if p.is_file():pack(p,Path('corpus/primary')/p.name)
diff=tmp/'r17-actual-diff'
for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','cases.tsv','run.py']:
    copy(diff/name,Path('differential')/name)
tree(diff/'src','differential/src')
tree(diff/'verification-existing','differential/verification')
tree(diff/'evidence','differential/evidence',lambda p:p.name not in ['differential.stdout','profile-actual-addone.stdout','profile-baseline-addone.stdout'])
for name in ['profile-actual-addone.stdout','profile-baseline-addone.stdout']:pack(diff/'evidence'/name,Path('differential/evidence')/name)
perf=tmp/'r17-perf'
tree(perf,'performance-exploration',lambda p:'target' not in p.parts and not any(v in p.parts for v in ['baseline','one-allocation','prefilter']) and not (p.suffix=='.json' and p.with_suffix(p.suffix+'.gz').exists()))
for variant in ['baseline','one-allocation','prefilter']:
    for rel in ['Cargo.toml','Cargo.lock','src/main.rs','crates/cloudleaf-search/src/search.rs']:
        if (perf/variant/rel).exists():copy(perf/variant/rel,Path('performance-exploration')/variant/rel)
host=tmp/'r17-host'
tree(host/'runs','host-temp/runs')
tree(host/'delivery','host-temp/delivery')
tree(tmp/'r17-focus','host-review',lambda p:p.suffix in ['.py','.patch','.json','.md'])
# 最终实现的历史保全与早期搜索证据的源码对应关系单独计算。
base='521472db18ecd7926f6e5b58b0afd7b5da674a07'
names=subprocess.check_output(['git','ls-tree','-r','--name-only',base,'--','docs/reports','data','tests','licenses'],cwd=root).decode().splitlines()
changed=set(subprocess.check_output(['git','diff','--name-only',base,'HEAD'],cwd=root).decode().splitlines())
assert not changed.intersection(names)
retained=[]
for name in subprocess.check_output(['git','ls-tree','-r','--name-only',base,'--','crates','adapters','apps'],cwd=root).decode().splitlines():
    if not name.endswith('.rs'):continue
    old=subprocess.check_output(['git','show',base+':'+name],cwd=root).decode('utf-8');new=(root/name).read_text(encoding='utf-8')
    for test in re.findall(r'#\[test\][\s\S]*?\bfn\s+(\w+)\s*\(',old):
        assert re.search(r'\bfn\s+'+test+r'\s*\(',new),name+':'+test
        retained.append(name+':'+test)
proof=json.loads((diff/'verification-existing/summary.json').read_text(encoding='utf-8'))
binding={p:sha(root/p)==v for p,v in proof['source_hashes'].items()};assert all(binding.values())
preservation={'base':base,'implementation_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),'old_paths_unchanged':len(names),'old_rust_tests_retained':len(retained),'test_names':retained,'earlier_differential_search_source_same':binding}
(out/'preservation.json').write_text(json.dumps(preservation,indent=2)+'\n',encoding='utf-8',newline='\n')
bins={str(p.relative_to(root)).replace('\\','/'):{'bytes':p.stat().st_size,'sha256':sha(p)} for p in [root/'.local/build/release'/n for n in ['cloudleaf-ime.exe','cloudleaf_windows_ime.dll','cloudleaf-cli.exe','examples/input_trace.exe','examples/train_language.exe','examples/coverage_probe.exe']]}
(out/'binary-identities.json').write_text(json.dumps(bins,indent=2)+'\n',encoding='utf-8',newline='\n')
for p in (root/'.local/round17-ci').iterdir():
    if p.is_dir() and (p/'summary.json').exists():tree(p,Path('CI')/p.name)
    if p.is_dir() and (p/'downloads.json').exists():
        copy(p/'downloads.json',Path('CI')/p.name/'downloads.json')
        copy(p/'logs.zip',Path('CI')/p.name/'logs.zip')
index={str(p.relative_to(out)).replace('\\','/'):{'bytes':p.stat().st_size,'sha256':sha(p)} for p in sorted(out.rglob('*')) if p.is_file() and p!=out/'index.json'}
(out/'index.json').write_text(json.dumps({'implementation_head':preservation['implementation_head'],'files':index},ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'files':len(index),'bytes':sum(v['bytes'] for v in index.values()),'old_paths':len(names),'old_tests':len(retained)}))
