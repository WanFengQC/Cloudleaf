"""固定源码检查并保留实际命令、返回码和原始字节；不覆盖旧目录。"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / '.local/round6-checks-9673c29'
OUT.mkdir(exist_ok=False)
env = os.environ.copy()
env['PATH'] = str(Path.home() / '.cargo/bin') + os.pathsep + env['PATH']
env['PYTHONUTF8'] = '1'
env['PYTHONIOENCODING'] = 'utf-8'
sys.path.insert(0, str(ROOT / 'scripts'))
from benchmark import source_sha256
commands = [
    (['cargo', 'fetch', '--locked'], 0),
    (['cargo', 'build', '--workspace', '--locked', '--offline'], 0),
    (['cargo', 'fmt', '--all', '--check'], 0),
    (['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--offline', '--', '-D', 'warnings'], 0),
    (['cargo', 'test', '--workspace', '--all-targets', '--locked', '--offline'], 0),
    (['cargo', 'test', '--workspace', '--doc', '--locked', '--offline'], 0),
    ([sys.executable, 'scripts/check_workspace.py'], 0),
    ([sys.executable, '-m', 'unittest', 'discover', '-s', 'tests', '-p', 'test_*.py'], 0),
    (['cargo', 'build', '--workspace', '--release', '--locked', '--offline'], 0),
    ([sys.executable, 'scripts/benchmark.py', '--iterations', '1000', '--output', str(OUT / 'prototype-performance.json')], 0),
    ([sys.executable, 'scripts/evaluate_algorithms.py', '--output-dir', str(OUT / 'algorithm-v1')], 0),
    ([sys.executable, 'scripts/evaluate_search.py', '--iterations', '100', '--warmup', '10', '--output', str(OUT / 'mechanical.json')], 0),
    ([sys.executable, 'scripts/check_language_readiness.py', '--plan', 'config/evaluation/language-readiness-v1.json', '--output', str(OUT / 'language-readiness.json')], 2),
    ([sys.executable, 'scripts/check_quality_preparation.py', '--plan', 'config/evaluation/language-readiness-v1.json', '--preparation', 'config/evaluation/quality-preparation-v1.json', '--output', str(OUT / 'quality-preparation.json')], 2),
    (['git', 'diff', '--check'], 0),
]
before = source_sha256()
rows = []
for index, (command, expected) in enumerate(commands, 1):
    command[0] = shutil.which(command[0], path=env['PATH']) or command[0]
    start = time.perf_counter()
    result = subprocess.run(command, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    path = OUT / f'command-{index:02}.log'
    path.write_bytes(result.stdout)
    rows.append({'command': command, 'returncode': result.returncode, 'expected_returncode': expected,
                 'elapsed_seconds': time.perf_counter() - start, 'raw_log': str(path),
                 'raw_log_sha256': hashlib.sha256(result.stdout).hexdigest()})
    print(json.dumps(rows[-1], ensure_ascii=False), flush=True)
    if result.returncode != expected:
        break
after = source_sha256()
(OUT / 'validation.json').write_text(json.dumps({'implementation_commit': subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT).decode().strip(),
    'source_sha256': before, 'source_after_sha256': after, 'checks': rows}, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
sys.exit(0 if before == after and len(rows) == len(commands) and all(row['returncode'] == row['expected_returncode'] for row in rows) else 1)
