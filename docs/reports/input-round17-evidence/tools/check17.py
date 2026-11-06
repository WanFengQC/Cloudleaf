from pathlib import Path
import gzip,hashlib,json,re,subprocess,zipfile
root=Path(__file__).resolve().parents[1];out=root/'docs/reports/input-round17-evidence'
index=json.loads((out/'index.json').read_text(encoding='utf-8'))['files']
staged={}
for raw in subprocess.check_output(['git','ls-files','-s','-z'],cwd=root).split(b'\0'):
    if raw:
        meta,path=raw.split(b'\t',1);staged[path.decode()]=meta.split()[1].decode()
paths=[(str((out/name).relative_to(root)).replace('\\','/'),row['sha256'])for name,row in index.items()]
data=subprocess.check_output(['git','cat-file','--batch'],cwd=root,input=('\n'.join(staged[name]for name,_ in paths)+'\n').encode())
pos=0
for name,sha in paths:
    end=data.index(b'\n',pos);size=int(data[pos:end].split()[2]);start=end+1;raw=data[start:start+size]
    assert hashlib.sha256(raw).hexdigest()==sha,name
    assert raw==(root/name).read_bytes(),name
    pos=start+size+1
gz=0;zips=0
for p in out.rglob('*.gz'):gzip.decompress(p.read_bytes());gz+=1
for p in out.rglob('*.zip'):
    with zipfile.ZipFile(p)as z:assert z.testzip()is None
    zips+=1
report=root/'docs/reports/input-round17-status.md'
for target in re.findall(r'\]\(([^)]+)\)',report.read_text(encoding='utf-8')):
    if '://'not in target:assert (report.parent/target).exists(),target
checks=json.loads((out/'round17-verified-results.json').read_text(encoding='utf-8'));assert len(checks)==13 and all(r['head']=='cfedc80f14e4ea7773ffed335ffe5fccfa5c2fcc' and r['returncode']==0 for r in checks)
summary=json.loads((out/'summary.json').read_text(encoding='utf-8'));assert summary['rust']=={'passed':228,'failed':0,'ignored':2}
assert all(sum(r['tasks']for r in summary['quality']if r['corpus']==c)==n for c,n in [('r16-all32',192),('news-dev',240)])
assert summary['performance_ns']['character']['p95']==6201700 and summary['performance_ns']['witten']['p95']==6055200
assert len(json.loads((out/'round17-final-prs.json').read_text(encoding='utf-8')))==5
print(json.dumps({'staged_evidence_SHA_all_match':len(paths),'gzip_full_CRC':gz,'zip_full_CRC':zips,'relative_report_links':'all exist','checks':13,'rust':summary['rust'],'formal_language_metrics':summary['formal_language_metrics']}))
