"""以实际Git blob登记受控方向导入，不覆盖已有差异。"""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
root=Path(__file__).resolve().parents[1]
direction,commit,*prefixes=sys.argv[1:]
output=root/'docs/reports/input-round14-imports'/f'{direction}.json'
output.parent.mkdir(parents=True,exist_ok=True)
assert not output.exists()
names=subprocess.check_output(['git','ls-tree','-r','--name-only',commit,'--',*prefixes],cwd=root).decode().splitlines()
rows=[]
for name in names:
    original=subprocess.check_output(['git','show',commit+':'+name],cwd=root)
    path=root/name
    assert path.is_file() and path.read_bytes().replace(b'\r\n',b'\n')==original.replace(b'\r\n',b'\n'),name
    rows.append({'path':name,'source_blob':subprocess.check_output(['git','rev-parse',commit+':'+name],cwd=root).decode().strip(),
                 'source_sha256':hashlib.sha256(original).hexdigest(),'working_sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
output.write_text(json.dumps({'direction':direction,'source_commit':commit,'base':'83a2666fa748b4ae9cba41a5880ba5353e71c9a3','files':rows},ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'direction':direction,'files':len(rows)}))
