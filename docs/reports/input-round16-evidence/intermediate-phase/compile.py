"""独立目标目录编译阶段探针，记录编译前后源码身份。"""
from datetime import datetime, timezone
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys
import time

root = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
tool = Path(__file__).parent
label = sys.argv[1]
out = tool / label
out.mkdir(exist_ok=False)
def source():
    paths = [root / 'Cargo.toml', root / 'Cargo.lock']
    for name in ('cloudleaf-search', 'cloudleaf-pinyin', 'cloudleaf-types'):
        paths += [root / 'crates' / name / 'Cargo.toml', *sorted((root / 'crates' / name / 'src').glob('*.rs'))]
    return {str(path.relative_to(root)).replace('\\', '/'): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths}

before = source()
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip()
diff = subprocess.check_output(['git', 'diff', '--binary', 'HEAD', '--', 'crates/cloudleaf-search', 'crates/cloudleaf-pinyin', 'crates/cloudleaf-types', 'Cargo.toml', 'Cargo.lock'], cwd=root)
(out / 'worktree.diff').write_bytes(diff)
command = ['C:/Users/EDY/.cargo/bin/cargo.exe', 'build', '--release', '--offline', '--manifest-path', str(tool / 'Cargo.toml')]
env = os.environ.copy()
env['CARGO_TARGET_DIR'] = str(tool / 'target')
start = time.perf_counter()
result = subprocess.run(command, cwd=tool, env=env, capture_output=True)
seconds = time.perf_counter() - start
after = source()
exe = tool / 'target/release/r16-phase-profile.exe'
record = {'at_utc': datetime.now(timezone.utc).isoformat(), 'head': head, 'command': command, 'returncode': result.returncode, 'seconds': seconds, 'source_before': before, 'source_after': after, 'source_unchanged': before == after, 'worktree_diff_sha256': hashlib.sha256(diff).hexdigest(), 'executable_sha256': hashlib.sha256(exe.read_bytes()).hexdigest() if exe.exists() else None, 'probe_sha256': hashlib.sha256((tool / 'src/main.rs').read_bytes()).hexdigest(), 'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'no_measurement_executed': True}
for name, raw in [('stdout', result.stdout), ('stderr', result.stderr)]:
    (out / (name + '.txt')).write_bytes(raw)
    record[name + '_sha256'] = hashlib.sha256(raw).hexdigest()
(out / 'compile.json').write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps({key: value for key, value in record.items() if key not in ('source_before', 'source_after')}, indent=2))
print(result.stderr.decode('utf-8', errors='replace'))
raise SystemExit(result.returncode)
