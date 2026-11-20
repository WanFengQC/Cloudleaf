"""只解出固定Round19产物供无窗口对照，不运行可交互入口。"""
from pathlib import Path
import hashlib
import json
import zipfile

root = Path(__file__).resolve().parents[1]
out = root/'.local/round20-baseline'
out.mkdir(exist_ok=False)
record = json.loads((root/'.local/round19-ci/final-downloads/downloads.json').read_text(encoding='utf-8'))
assert record['head'] == '4d42363091361552ad525f47e3c6b1d581ae3a4c'
artifact = next(a for a in record['files'] if a['file'] == 'input-round19-windows.zip')
path = root/'.local/round19-ci/final-downloads'/artifact['file']
assert hashlib.sha256(path.read_bytes()).hexdigest() == artifact['sha256']
names = ['cloudleaf-ime.exe', 'cloudleaf-cli.exe', 'input_trace.exe', 'scheme_trace.exe',
         'train_language.exe', 'coverage_probe.exe', 'cloudleaf_windows_ime.dll']
members = []
with zipfile.ZipFile(path) as archive:
    assert archive.testzip() is None
    for name in names:
        row = next(row for row in artifact['members'] if Path(row['name']).name == name)
        raw = archive.read(row['name'])
        assert hashlib.sha256(raw).hexdigest() == row['sha256']
        (out/name).write_bytes(raw)
        members.append(row)
(out/'identity.json').write_text(json.dumps({'head': record['head'], 'archive': str(path),
    'archive_sha256': artifact['sha256'], 'CRC': 'passed', 'members': members,
    'GUI_launched': False, 'purpose': 'fixed baseline headless benchmarks only'}, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print(json.dumps({'head':record['head'],'CRC':'passed','member_SHA256':'all matched','files':len(members)}))
