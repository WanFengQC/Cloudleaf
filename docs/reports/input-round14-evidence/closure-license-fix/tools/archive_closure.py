"""追加收尾证据，原始R14证据与历史报告不覆盖。"""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import re
import shutil
import subprocess

root = Path(__file__).resolve().parents[1]
evidence = root / 'docs/reports/input-round14-evidence'
old_index = json.loads((evidence / 'index.json').read_text(encoding='utf-8'))
for row in old_index['files']:
    assert hashlib.sha256((evidence / row['path']).read_bytes()).hexdigest() == row['sha256']
out = evidence / 'closure-license-fix'
out.mkdir(exist_ok=False)

def copy(source, relative):
    destination = out / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)

for path in (root / '.local/round14-runs').iterdir():
    if path.name.startswith(('closure-', 'data-license-original-')):
        for file in path.iterdir():
            if file.is_file():
                copy(file, Path('runs') / path.name / file.name)
for name in ['round14-closure-results.json', 'round14-closure-dependency-audit.json',
             'round14-closure-lexicon-receipt.json', 'round14-closure-performance.json']:
    copy(root / '.local' / name, name)
for name in ['closure_acceptance.py', 'run_command.py', 'build_receipt.py', 'collect_ci.py', 'download_ci.py']:
    copy(root / '.local' / name, Path('tools') / name)
for path in (root / '.local/round14-closure-algorithm-v1').rglob('*'):
    if path.is_file():
        copy(path, Path('prototype-algorithm-v1') / path.relative_to(root / '.local/round14-closure-algorithm-v1'))
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip()
rows = json.loads((root / '.local/round14-closure-results.json').read_text(encoding='utf-8'))
assert len(rows) == 13 and all(row['returncode'] == 0 and row['head'] == head for row in rows)
rust = (root / '.local/round14-runs/closure-04/stdout.txt').read_text(encoding='utf-8')
python = (root / '.local/round14-runs/closure-08/stderr.txt').read_text(encoding='utf-8')
licenses = {}
for name in ['CC-BY-SA-4.0.txt', 'Unicode-3.0.txt']:
    relative = 'data/licenses/' + name
    data = (root / relative).read_bytes()
    original = Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf') / relative
    blob = subprocess.check_output(['git', 'show', 'HEAD:' + relative], cwd=root)
    assert data == blob == original.read_bytes()
    licenses[name] = {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(),
                      'worktree_equals_git_blob_and_C_original': True}
diff = subprocess.check_output(['git', 'diff', '8147194', head], cwd=root)
(out / 'fix.diff').write_bytes(diff)
summary = {'at_utc': datetime.now(timezone.utc).isoformat(), 'implementation_head': head,
    'red_test_commit': '81471948c7d38c668b737deedd8cb87ed801f510',
    'red_result': {'returncode': 1, 'failing_original_licenses': 2},
    'targeted_green': {'returncode': 0, 'tests': 2},
    'rust_passed': sum(map(int, re.findall(r'test result: ok\. (\d+) passed', rust))),
    'rust_ignored': sum(map(int, re.findall(r'; (\d+) ignored', rust))),
    'python_passed': int(re.search(r'Ran (\d+) tests', python).group(1)),
    'doc_test_examples': 0, 'full_commands': len(rows), 'all_returncodes_zero': True,
    'licenses': licenses, 'runtime_or_source_lock_changed': False,
    'original_evidence_files_preserved': len(old_index['files']),
    'Rust_product_sources_unchanged_since_a707d20': not bool(subprocess.check_output(
        ['git', 'diff', 'a707d20', head, '--', 'crates', 'apps', 'Cargo.toml', 'Cargo.lock'], cwd=root)),
    'reason_no_new_language_or_performance_claim': 'Only offline audit, source-license attributes and tests changed.',
    'binary_identities': {str(p.relative_to(root / '.local/build/release')).replace('\\', '/'):
        {'bytes': p.stat().st_size, 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in
        [root / '.local/build/release' / name for name in
         ['cloudleaf-cli.exe', 'cloudleaf-ime.exe', 'cloudleaf_windows_ime.dll', 'examples/input_trace.exe']]}}
(out / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps(summary, ensure_ascii=False, indent=2))
