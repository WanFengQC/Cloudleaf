"""运行两条产品线的绑定提交验证，各检查独立记录。"""
import json
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
label = sys.argv[1]
audit = 'C:/Users/EDY/WebstormProjects/Cloudleaf/.local/audit-tool/bin/cargo-audit.exe'
commands = [
    ['git', 'diff', '--check'],
    ['cargo', 'fmt', '--all', '--check'],
    ['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--offline', '--', '-D', 'warnings'],
    ['cargo', 'test', '--workspace', '--all-targets', '--locked', '--offline'],
    ['cargo', 'test', '--workspace', '--doc', '--locked', '--offline'],
    ['cargo', 'build', '--workspace', '--release', '--locked', '--offline'],
    ['python', 'scripts/check_workspace.py'],
    ['python', '-m', 'unittest', 'discover', '-s', 'tests', '-p', 'test_*.py', '-v'],
    ['python', 'scripts/audit_windows_dependencies.py', '--audit-binary', audit, '--output', f'.local/{label}-windows-audit.json'],
    ['python', 'scripts/benchmark.py', '--iterations', '100', '--output', f'.local/{label}-input-performance.json'],
    ['python', 'scripts/evaluate_algorithms.py', '--output-dir', f'.local/{label}-algorithm-v1'],
    ['cargo', 'build', '-p', 'cloudleaf-input', '--examples', '--release', '--locked', '--offline'],
    ['cargo', 'build', '-p', 'cloudleaf-search', '--examples', '--release', '--locked', '--offline'],
    ['cargo', 'build', '-p', 'cloudleaf-offline', '--examples', '--release', '--locked', '--offline'],
    ['python', 'scripts/check_offline_vendor.py', '.'],
    ['python', 'scripts/audit_translation_dependencies.py', '--audit-binary', audit, '--output', f'.local/{label}-translation-audit.json'],
]
rows = []
for i, command in enumerate(commands, 1):
    step = f'{label}-{i:02}'
    process = subprocess.run([sys.executable, '-X', 'utf8', '.local/run21.py', step, *command], cwd=root, capture_output=True)
    record = json.loads((root / '.local/round21-runs' / step / 'result.json').read_text(encoding='utf-8'))
    rows.append({'label': step, **{k: v for k, v in record.items() if k != 'source_sha256'}})
    (root / '.local' / f'{label}-results.json').write_text(json.dumps(rows, indent=2) + '\n', encoding='utf-8')
    print(f"{i:02} {record['returncode']} {record['seconds']:.2f}s", flush=True)
    if process.returncode:
        print(process.stdout.decode('utf-8', 'replace')[-5500:] + process.stderr.decode('utf-8', 'replace')[-1000:], flush=True)
raise SystemExit(any(row['returncode'] for row in rows))
