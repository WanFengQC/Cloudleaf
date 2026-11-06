from pathlib import Path
import gzip,hashlib,json
root=Path(__file__).resolve().parent
compressed=[]
for p in sorted(root.rglob('*')):
    if not p.is_file() or 'target' in p.parts or p.suffix=='.gz':continue
    if (p.suffix=='.json' and p.stat().st_size>100000) or p.name=='differential.stdout':
        target=p.with_name(p.name+'.gz')
        if not target.exists():target.write_bytes(gzip.compress(p.read_bytes(),mtime=0))
        raw=p.read_bytes();zipped=target.read_bytes();assert gzip.decompress(zipped)==raw
        compressed.append({'source':str(p.relative_to(root)),'source_bytes':len(raw),'source_sha256':hashlib.sha256(raw).hexdigest(),'gzip':str(target.relative_to(root)),'gzip_bytes':len(zipped),'gzip_sha256':hashlib.sha256(zipped).hexdigest()})
(root/'compression.json').write_text(json.dumps(compressed,indent=2)+'\n',encoding='utf-8')
files={}
for p in sorted(root.rglob('*')):
    if not p.is_file() or 'target' in p.parts or p.name=='evidence-index.json':continue
    if p.stat().st_size>100000 and p.with_name(p.name+'.gz').exists():continue
    files[str(p.relative_to(root)).replace('\\','/')]={'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
(root/'evidence-index.json').write_text(json.dumps(files,indent=2)+'\n',encoding='utf-8')
print({'files':len(files),'bytes':sum(row['bytes'] for row in files.values()),'compressed':len(compressed)})
