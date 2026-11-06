from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import os
import subprocess
import time

root = Path(__file__).resolve().parent
source = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
files = ['crates/cloudleaf-search/src/search.rs', 'crates/cloudleaf-search/src/model.rs',
         'crates/cloudleaf-search/src/lib.rs', 'crates/cloudleaf-input/src/sentence.rs',
         'crates/cloudleaf-input/src/lib.rs', 'adapters/windows/session.rs']
before = {name: hashlib.sha256((source / name).read_bytes()).hexdigest() for name in files}
env = os.environ.copy()
env['PATH'] = 'C:/Users/EDY/.cargo/bin' + os.pathsep + env['PATH']
env['CARGO_TARGET_DIR'] = str(root / 'target')
command = ['C:/Users/EDY/.cargo/bin/cargo.exe', 'run', '--release', '--offline']
started = time.perf_counter()
result = subprocess.run(command, cwd=root, env=env, capture_output=True)
after = {name: hashlib.sha256((source / name).read_bytes()).hexdigest() for name in files}
(root / 'stdout.txt').write_bytes(result.stdout)
(root / 'stderr.txt').write_bytes(result.stderr)
record = {'at_utc': datetime.now(timezone.utc).isoformat(), 'command': command, 'returncode': result.returncode,
          'seconds': time.perf_counter() - started, 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source).decode().strip(),
          'source_before_sha256': before, 'source_after_sha256': after, 'source_unchanged_during_run': before == after,
          'probe_sha256': hashlib.sha256((root / 'src/main.rs').read_bytes()).hexdigest(),
          'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(), 'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
with (root / 'result.json').open('x', encoding='utf-8', newline='\n') as file:
    json.dump(record, file, ensure_ascii=False, indent=2)
    file.write('\n')
print(result.stdout.decode('utf-8', errors='replace'))
print(result.stderr.decode('utf-8', errors='replace')[-2000:])
print(json.dumps(record, ensure_ascii=False, indent=2))
raise SystemExit(result.returncode)
