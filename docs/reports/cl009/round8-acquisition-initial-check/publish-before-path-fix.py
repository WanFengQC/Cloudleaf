"""保存追加取得的实际检查及原件身份；保留先前阶段目录。"""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path.cwd()
OUT = ROOT / 'docs/reports/cl009/round8-acquisition'
OUT.mkdir(exist_ok=False)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def save(name, value):
    (OUT / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')


acquisition = ROOT / '.local/round8-ngram-acquisition/acquisition.json'
raws = {'acquisition.json': {'original_path': str(acquisition), 'bytes': acquisition.stat().st_size,
         'sha256': sha(acquisition.read_bytes()), 'utf8': acquisition.read_bytes().decode('utf-8')}}
(OUT / 'acquisition.json').write_bytes(acquisition.read_bytes())
env = os.environ.copy()
env['PYTHONUTF8'] = '1'
env['PYTHONIOENCODING'] = 'utf-8'
checks = []
for index, command in enumerate([
    [sys.executable, '-X', 'utf8', '.local/verify_round8_acquisition.py'],
    [sys.executable, '-X', 'utf8', 'scripts/check_workspace.py'],
    [str(Path.home() / '.cargo/bin/cargo.exe'), 'fmt', '--all', '--check'],
    ['git', 'diff', '--check'],
], 1):
    result = subprocess.run(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    row = {'command': command, 'returncode': result.returncode, 'expected_returncode': 0}
    for label, raw in [('stdout', result.stdout), ('stderr', result.stderr)]:
        name = f'check-{index:02}-{label}.txt'
        raws[name] = {'original_path': None, 'capture': 'subprocess.PIPE', 'bytes': len(raw), 'sha256': sha(raw), 'utf8': raw.decode('utf-8')}
        (OUT / name).write_text(raw.decode('utf-8').replace('\r\n', '\n'), encoding='utf-8', newline='\n')
        row[label + '_raw_sha256'] = sha(raw)
    checks.append(row)
    save('checks.json', {'checked_head': subprocess.check_output(['git', 'rev-parse', 'HEAD']).decode().strip(),
                        'recorded_at_utc': datetime.now(timezone.utc).isoformat(), 'checks': checks})
    print(json.dumps({'check': index, 'returncode': result.returncode}), flush=True)
    if result.returncode != 0:
        save('raw-artifacts.json', {'files': raws})
        raise SystemExit(result.returncode)
save('raw-artifacts.json', {'encoding': 'utf8.encode preserves captured original process bytes', 'files': raws})
sources = []
for name in ['acquire_round8_ngram.py', 'verify_round8_acquisition.py', 'publish_round8_acquisition.py']:
    raw = (ROOT / '.local' / name).read_bytes()
    sources.append({'name': name, 'bytes': len(raw), 'sha256': sha(raw), 'utf8': raw.decode('utf-8')})
save('runners.json', {'sources': sources, 'warning': 'Acquisition uses exclusive creation; do not rerun against an existing directory. No production parser or frequency computation.'})
save('checksums.json', {'scope': 'Git LF publication bytes; captured raw original process bytes in raw-artifacts.json',
                       'files': {path.name: sha(path.read_bytes()) for path in sorted(OUT.iterdir()) if path.is_file()}})
