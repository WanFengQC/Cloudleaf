"""执行冻结Round7实现的完整检查，保留实际拒绝及全部原始日志。"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / '.local/round7-checks-2a2bd9a'
OUT.mkdir(exist_ok=False)
env = os.environ.copy()
env['PATH'] = str(Path.home() / '.cargo/bin') + os.pathsep + env['PATH']
env['PYTHONUTF8'] = '1'
env['PYTHONIOENCODING'] = 'utf-8'
commands = [
    (['rustc', '--version'], 0), (['cargo', 'fetch', '--locked'], 0),
    (['cargo', 'build', '--workspace', '--locked', '--offline'], 0),
    (['cargo', 'fmt', '--all', '--check'], 0),
    (['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--offline', '--', '-D', 'warnings'], 0),
    (['cargo', 'test', '--workspace', '--all-targets', '--locked', '--offline'], 0),
    (['cargo', 'test', '--workspace', '--doc', '--locked', '--offline'], 0),
    (['cargo', 'build', '--workspace', '--release', '--locked', '--offline'], 0),
    ([sys.executable, '-X', 'utf8', 'scripts/check_workspace.py'], 0),
    ([sys.executable, '-X', 'utf8', '-m', 'unittest', 'discover', '-s', 'tests', '-p', 'test_*.py', '-v'], 0),
    ([sys.executable, '-X', 'utf8', 'scripts/evaluate_search.py', '--iterations', '100', '--warmup', '10', '--output', str(OUT/'mechanics.json')], 0),
    ([sys.executable, '-X', 'utf8', 'scripts/benchmark.py', '--iterations', '1000', '--output', str(OUT/'prototype.json')], 0),
    ([sys.executable, '-X', 'utf8', 'scripts/evaluate_algorithms.py', '--output-dir', str(OUT/'algorithm-v1')], 0),
    ([sys.executable, '-X', 'utf8', 'scripts/check_language_readiness.py', '--plan', 'config/evaluation/language-readiness-v1.json', '--output', str(OUT/'language-readiness.json')], 2),
    ([sys.executable, '-X', 'utf8', 'scripts/check_quality_preparation.py', '--plan', 'config/evaluation/language-readiness-v1.json', '--preparation', 'config/evaluation/quality-preparation-v1.json', '--output', str(OUT/'quality-preparation.json')], 2),
    ([sys.executable, '-X', 'utf8', 'scripts/check_ime_task_handoff.py', '--plan', 'config/evaluation/language-readiness-v1.json', '--preparation', 'config/evaluation/quality-preparation-v1.json', '--handoff', 'config/evaluation/ime-task-handoff-v1.json', '--output', str(OUT/'ime-task-handoff.json')], 2),
    (['git', 'diff', '--check'], 0),
]
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT).decode().strip()
assert head == '2a2bd9a7ea9df73b647ccdba609d9aae8f0ad935'
report = {'implementation_commit': head, 'checks': []}
for index, (command, expected) in enumerate(commands, 1):
    command[0] = shutil.which(command[0], path=env['PATH']) or command[0]
    start = time.perf_counter()
    result = subprocess.run(command, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    name = f'command-{index:02}.txt'
    (OUT / name).write_bytes(result.stdout)
    row = {'command': command, 'returncode': result.returncode, 'expected_returncode': expected,
           'seconds': time.perf_counter() - start, 'log': name, 'log_sha256': hashlib.sha256(result.stdout).hexdigest()}
    report['checks'].append(row)
    (OUT/'validation.json').write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n', encoding='utf-8', newline='\n')
    print(json.dumps(row), flush=True)
raise SystemExit(not all(row['returncode'] == row['expected_returncode'] for row in report['checks']))
