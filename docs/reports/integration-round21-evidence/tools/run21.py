"""保存单条 Round 21 命令的两路输出、返回码与源码身份。"""
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

root = Path(__file__).resolve().parents[1]
label, *command = sys.argv[1:]
out = root / '.local' / 'round21-runs' / label
out.mkdir(parents=True, exist_ok=False)
env = os.environ.copy()
env['PATH'] = str(Path.home() / '.cargo/bin') + os.pathsep + env['PATH']
env.setdefault('CARGO_TARGET_DIR', str(root / '.local/target'))
env.setdefault('CARGO_INCREMENTAL', '0')
env.setdefault('CARGO_PROFILE_DEV_DEBUG', '0')
env.setdefault('CARGO_PROFILE_TEST_DEBUG', '0')
env.setdefault('CARGO_BUILD_JOBS', '4')
env.update(PYTHONUTF8='1', PYTHONIOENCODING='utf-8')
command[0] = shutil.which(command[0], path=env['PATH']) or command[0]
paths = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=root)
source = {}
for name in paths.decode('utf-8').strip('\x00').split('\x00'):
    path = root / name
    if path.suffix in {'.rs', '.toml', '.lock', '.py', '.yml'} and path.is_file() and not name.startswith('docs/'):
        source[name] = hashlib.sha256(path.read_bytes()).hexdigest()
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip()
start = time.monotonic()
with (out / 'stdout.txt').open('wb') as stdout, (out / 'stderr.txt').open('wb') as stderr:
    code = subprocess.call(command, cwd=root, env=env, stdout=stdout, stderr=stderr)
record = {'command': command, 'head': head, 'at_utc': datetime.now(timezone.utc).isoformat(),
          'seconds': time.monotonic() - start, 'returncode': code, 'source_sha256': source}
for stream in ('stdout', 'stderr'):
    data = (out / f'{stream}.txt').read_bytes()
    record[stream] = {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}
    if code:
        print(data.decode('utf-8', 'replace')[-6500:])
(out / 'result.json').write_text(json.dumps(record, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
print(json.dumps({k: v for k, v in record.items() if k != 'source_sha256'}, ensure_ascii=False), flush=True)
raise SystemExit(code)
