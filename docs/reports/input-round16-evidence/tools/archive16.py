from pathlib import Path
import gzip
import hashlib
import json
import shutil
import subprocess

root=Path(__file__).resolve().parents[1]
dest=root/'docs/reports/input-round16-evidence'
dest.mkdir(exist_ok=False)
def copy(source, relative):
    source=Path(source); target=dest/relative
    target.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(source,target)
def tree(source, relative, allow=None):
    source=Path(source)
    for p in sorted(source.rglob('*')):
        if p.is_file() and (allow is None or allow(p)):
            copy(p,Path(relative)/p.relative_to(source))
def packed(source, relative):
    source=Path(source); raw=source.read_bytes()
    target=dest/relative;target.parent.mkdir(parents=True,exist_ok=True)
    zipped=gzip.compress(raw,mtime=0)
    assert gzip.decompress(zipped)==raw
    target.write_bytes(zipped)
    return {'original_bytes':len(raw),'original_sha256':hashlib.sha256(raw).hexdigest(),'gzip_bytes':len(zipped),'gzip_sha256':hashlib.sha256(zipped).hexdigest()}

tree(root/'.local/round16-runs','runs')
for p in sorted((root/'.local').glob('round16-*')):
    if p.is_dir() and (p/'result.json').exists() and (p/'stdout.tsv').exists():
        tree(p,Path('traces')/p.name)
for name in ['round16-delivery-paired-traces','round16-paired-traces']:
    tree(root/'.local'/name,name)
for name in ['round16-full-results.json','round16-verified-results.json','round16-delivery-results.json','round16-final-dependency-audit.json','round16-performance.json','round16-lexicon-receipt.json','round16-baseline-source.json','round16-baseline-build.stdout','round16-baseline-build.stderr','round16-witten-dev-summary.json','round16-tool-errors.txt']:
    copy(root/'.local'/name,name)
tree(root/'.local/round16-algorithm-v1','algorithm-baseline')
tree(root/'.local/round16-delivery-summary','summary')
tree(root/'.local/round16-summary','intermediate-summary')
for p in sorted((root/'.local').glob('*16*.py')):copy(p,Path('tools')/p.name)
for name in ['metadata.tsv','covered-metadata.tsv','witten-metadata.tsv']:
    copy(root/'.local/round16-language'/name,Path('models')/name)

corpus=Path('C:/Users/EDY/.codex/tmp/r16-corpus')
for name in ['source-review.md','acquisition.json','extra-acquisition.json','profile.json','actual-char-transitions.json','verify-near.json','prepare-initial-failure.json','word-reproducibility.json','prepare.py','prepare-word.py','verify_near.py','acquire.py','acquire_extra.py','profile.py']:
    copy(corpus/name,Path('corpus')/name)
for name in ['downloads.html','terms.html','faq.html','using-corpus.html','guidelines.html','CC-BY-2.0-FR-legalcode.html','UD-licensing.html']:
    packed(corpus/name,Path('corpus/primary')/(name+'.gz'))
for name in ['summary.json','near-pairs.json','dangling-eng-links.json']:
    copy(root/'.local/round16-corpus-a'/name,Path('corpus/prepared')/name)
for name in ['summary.json','summary-addendum.json','source-binding.json','verification.json','metadata-header.tsv','development-tasks.jsonl','development-events.tsv']:
    copy(corpus/'word-contrast'/name,Path('corpus/word-contrast')/name)
corpus_identity={}
for directory in [corpus,root/'.local/round16-corpus-a',root/'.local/round16-corpus-b',corpus/'word-contrast']:
    for p in directory.iterdir():
        if p.is_file():corpus_identity[str(p)]={'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
(dest/'corpus/git-external-files.json').write_text(json.dumps(corpus_identity,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')

review=Path('C:/Users/EDY/.codex/tmp/r16-search-review')
tree(review,'diagnosis',lambda p: 'target' not in p.parts and p.name not in ['diagnosis.stdout.tsv','diagnosis.stdout.tsv.gz'])
packed(review/'diagnosis.stdout.tsv','diagnosis/diagnosis.stdout.tsv.gz')
baseline=Path('C:/Users/EDY/.codex/tmp/r16-baseline')
tree(baseline,'independent-review',lambda p:'target' not in p.parts)

diff=Path('C:/Users/EDY/.codex/tmp/r16-character-diff')
for name in ['Cargo.toml','Cargo.lock','src/main.rs','run.py','compression-evidence.json']:
    if (diff/name).exists():copy(diff/name,Path('differential')/name)
for p in sorted((diff/'runs').iterdir()):
    if p.is_dir():
        for f in sorted(p.iterdir()):
            if f.is_file() and f.name!='run.stdout.txt':copy(f,Path('differential/runs')/p.name/f.name)
        if not (p/'run.stdout.txt.gz').exists():packed(p/'run.stdout.txt',Path('differential/runs')/p.name/'run.stdout.txt.gz')

for source,label in [('C:/Users/EDY/.codex/tmp/r16-phase-profile','intermediate-phase'),('C:/Users/EDY/.codex/tmp/r16-phase-final-f2','final-phase')]:
    source=Path(source)
    for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','src/main.rs','cases.tsv','compile.py','measure.py']:
        copy(source/name,Path(label)/name)
    for d in source.iterdir():
        if d.is_dir() and ('compile' in d.name or 'final' in d.name or 'trial-measurements'==d.name):
            for p in sorted(d.rglob('*')):
                if p.is_file():
                    relative=Path(label)/d.name/p.relative_to(d)
                    if p.suffix=='.json' and p.stat().st_size>50000:packed(p,str(relative)+'.gz')
                    else:copy(p,relative)
copy(root/'.local/round16-remote/resumed-before-delivery/summary.json','PRs-before-final.json')
for label in ['implementation']:
    tree(root/'.local/round16-ci'/label,Path('CI')/label)

impl='f2b5c5ee53882787c84b29180da6e835ffa98a27'
changed=subprocess.check_output(['git','diff','--name-status','5045e18',impl],cwd=root).decode('utf-8')
(dest/'changed-files.tsv').write_text(changed,encoding='utf-8',newline='\n')
identities={}
for name in ['.local/build/release/cloudleaf-ime.exe','.local/build/release/cloudleaf_windows_ime.dll','.local/build/release/cloudleaf-cli.exe','.local/build/release/examples/input_trace.exe','.local/build/release/examples/train_language.exe','Cargo.lock','licenses/input-context/CC-BY-2.0-FR.txt']:
    p=root/name;identities[name]={'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
for p in (root/'.local/round16-language').glob('*.tsv'):
    identities[str(p)]={'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
(dest/'identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8',newline='\n')
index={str(p.relative_to(dest)).replace('\\','/'):{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(dest.rglob('*')) if p.is_file()}
(dest/'index.json').write_text(json.dumps({'implementation_head':impl,'files':index},ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'files':len(index),'bytes':sum(v['bytes'] for v in index.values()),'output':str(dest)}))
