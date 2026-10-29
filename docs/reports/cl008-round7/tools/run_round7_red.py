"""保存真正旧实现的失败命令和原文，不改写历史结果。"""
import hashlib
import json
import os
import shutil
from pathlib import Path
import subprocess
import sys

ROOT = Path.cwd()
OUT = ROOT / '.local/round7-red-complete'
OUT.mkdir(exist_ok=False)
env = os.environ.copy()
env['PYTHONUTF8'] = '1'
env['PYTHONIOENCODING'] = 'utf-8'
env['PATH'] = str(Path.home() / '.cargo/bin') + os.pathsep + env['PATH']
commands = [
    ('fallback', [sys.executable, '-m', 'unittest', 'discover', '-s', 'tests', '-p', 'test_language_readiness.py', '-k', 'non_ok', '-v']),
    ('task-handoff', [sys.executable, '-m', 'unittest', 'discover', '-s', 'tests', '-p', 'test_ime_task_handoff.py', '-v']),
    ('scoring', ['cargo', 'test', '--locked', '--offline', '-p', 'cloudleaf-search', '--test', 'profiling', 'scoring_is_an_explicit', '--', '--nocapture']),
]
rows = []
for name, command in commands:
    command[0] = shutil.which(command[0], path=env['PATH']) or command[0]
    result = subprocess.run(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    path = OUT / (name + '.txt')
    path.write_bytes(result.stdout)
    rows.append({'name': name, 'command': command, 'returncode': result.returncode,
                 'log': path.name, 'sha256': hashlib.sha256(result.stdout).hexdigest()})
value = {'head': subprocess.check_output(['git', 'rev-parse', 'HEAD']).decode().strip(), 'checks': rows}
(OUT / 'identity.json').write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps(value))
assert all(row['returncode'] != 0 for row in rows)
