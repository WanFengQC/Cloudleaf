"""由实际stdout中的确定性字符串还原合成TSV并固定内容SHA；不保存私人词库。"""
from pathlib import Path
import hashlib,json
p=Path(__file__).resolve().parent;out=p/'user-layer-performance'
identities={}
for folder in sorted(out.iterdir()):
    if not folder.is_dir():continue
    line=next(line for line in(folder/'stdout.txt').read_text(encoding='utf-8').splitlines()if line.startswith('USER_TSV\t'))
    # 本夹具只有普通Han/ASCII/Tab/LF，Rust Debug字符串与JSON转义在此完全一致。
    tsv=json.loads(line.split('\t',1)[1]).encode('utf-8')
    size=int(folder.name.rsplit('-',1)[1])if not folder.name.endswith('--1')else -1
    rows=tsv.decode('utf-8').splitlines();expected=max(0,size)
    assert len(rows)-2==expected
    identities[folder.name]={'rows':expected,'bytes':len(tsv),'sha256':hashlib.sha256(tsv).hexdigest(),'source':'Actual USER_TSV debug field from same process stdout, deterministic JSON-compatible fixture strings only'}
    name=f'synthetic-{max(0,size)}.tsv'
    dst=out/'fixtures'/name;dst.parent.mkdir(exist_ok=True)
    if dst.exists():assert dst.read_bytes()==tsv
    else:dst.write_bytes(tsv)
(out/'fixture-identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'identities':len(identities),'fixedfixtures':4}))
