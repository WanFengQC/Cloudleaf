"""逐命令保存Round13精确实现的完整回归与隔离产物目录。"""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[1]
out = root / '.local/round13-final-acceptance'
out.mkdir(exist_ok=False)
env = os.environ.copy()
env['PATH'] = str(Path.home() / '.cargo/bin') + os.pathsep + env['PATH']
env['PYTHONUTF8'] = '1'
env['PYTHONIOENCODING'] = 'utf-8'
env['CARGO_TARGET_DIR'] = str(root / '.local/round13-build')
cargo = shutil.which('cargo', path=env['PATH'])
commands = [
    ['git', 'diff', '--check'], [cargo, 'fmt', '--all', '--check'],
    [cargo, 'clippy', '--workspace', '--all-targets', '--locked', '--offline', '--', '-D', 'warnings'],
    [cargo, 'test', '--workspace', '--all-targets', '--locked', '--offline'],
    [cargo, 'test', '--workspace', '--doc', '--locked', '--offline'],
    [cargo, 'build', '--workspace', '--release', '--locked', '--offline'],
    [sys.executable, '-X', 'utf8', 'scripts/check_workspace.py'],
    [sys.executable, '-X', 'utf8', '-m', 'unittest', 'discover', '-s', 'tests', '-p', 'test_*.py', '-v'],
    [sys.executable, '-X', 'utf8', 'scripts/audit_translation_dependencies.py', '--audit-binary',
     'C:/Users/EDY/WebstormProjects/Cloudleaf/.local/audit-tool/bin/cargo-audit.exe', '--output', str(out / 'dependency-audit.json')],
    [sys.executable, '-X', 'utf8', 'scripts/benchmark.py', '--iterations', '100', '--output', str(out / 'm2-performance.json')],
    [sys.executable, '-X', 'utf8', 'scripts/evaluate_algorithms.py', '--output-dir', str(out / 'algorithm-v1')],
    [cargo, 'build', '-p', 'cloudleaf-translation-panel', '--release', '--locked', '--offline', '--message-format=json'],
    [cargo, 'build', '-p', 'cloudleaf-offline', '--examples', '--release', '--locked', '--offline', '--message-format=json'],
    [sys.executable, '-X', 'utf8', 'scripts/check_offline_vendor.py', str(root)],
]
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip()
files = subprocess.check_output(['git', 'ls-files', '-z'], cwd=root).decode('utf-8').split('\0')
identity = {'head': head, 'at_utc': datetime.now(timezone.utc).isoformat(), 'platform': platform.platform(),
    'python': sys.version, 'CARGO_TARGET_DIR': env['CARGO_TARGET_DIR'], 'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'tracked_worktree_bytes': {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in files if name}}
(out / 'identity.json').write_text(json.dumps(identity, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
rows=[]
for i, command in enumerate(commands, 1):
    start=time.perf_counter()
    result=subprocess.run(command,cwd=root,env=env,capture_output=True)
    row={'command':command,'returncode':result.returncode,'seconds':time.perf_counter()-start,'head':head}
    for stream in ['stdout','stderr']:
        raw=getattr(result,stream);path=out/f'{i:02}-{stream}.txt';path.write_bytes(raw)
        row[stream]={'file':path.name,'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()}
    rows.append(row)
    (out/'results.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'i':i,'returncode':result.returncode,'seconds':row['seconds']}),flush=True)
    if result.returncode:
        print(result.stdout.decode('utf-8',errors='replace')[-2500:]+result.stderr.decode('utf-8',errors='replace')[-4500:],flush=True)
raise SystemExit(any(row['returncode'] for row in rows))
