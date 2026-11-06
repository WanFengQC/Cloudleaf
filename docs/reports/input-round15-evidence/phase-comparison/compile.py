from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import os
import subprocess
import time

root = Path(__file__).resolve().parent
source = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
env = os.environ.copy()
env['CARGO_TARGET_DIR'] = str(root / 'target')
env['PATH'] = 'C:/Users/EDY/.cargo/bin' + os.pathsep + env['PATH']
files = ['crates/cloudleaf-search/src/search.rs', 'crates/cloudleaf-search/src/model.rs',
         'crates/cloudleaf-search/src/profiling.rs', 'crates/cloudleaf-search/src/lattice.rs']
before = {name: hashlib.sha256((source / name).read_bytes()).hexdigest() for name in files}
command = ['C:/Users/EDY/.cargo/bin/cargo.exe', 'build', '--release', '--offline']
started = time.perf_counter()
result = subprocess.run(command, cwd=root, env=env, capture_output=True)
(root / 'compile.stdout.txt').write_bytes(result.stdout)
(root / 'compile.stderr.txt').write_bytes(result.stderr)
after = {name: hashlib.sha256((source / name).read_bytes()).hexdigest() for name in files}
record = {'command': command, 'returncode': result.returncode, 'seconds': time.perf_counter()-started,
          'at_utc': datetime.now(timezone.utc).isoformat(), 'head': subprocess.check_output(['git','rev-parse','HEAD'],cwd=source).decode().strip(),
          'no_measurement_executed': True, 'source_before': before, 'source_after': after,
          'source_unchanged': before == after, 'probe_sha256': hashlib.sha256((root/'src/main.rs').read_bytes()).hexdigest()}
if result.returncode == 0:
    record['executable_sha256'] = hashlib.sha256((root/'target/release/r15-phase-final.exe').read_bytes()).hexdigest()
with (root/'compile.json').open('x', encoding='utf-8', newline='\n') as file:
    json.dump(record,file,ensure_ascii=False,indent=2);file.write('\n')
print(result.stderr.decode('utf-8',errors='replace')[-3000:])
print(json.dumps(record,ensure_ascii=False,indent=2))
raise SystemExit(result.returncode)
