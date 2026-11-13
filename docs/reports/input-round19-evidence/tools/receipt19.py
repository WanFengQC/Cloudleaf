"""普通推送及最终精确CI完成后，核验并创建Git外回执。"""
from pathlib import Path
from datetime import datetime, timezone
import gzip
import hashlib
import json
import platform
import re
import subprocess
import zipfile

root = Path(__file__).resolve().parents[1]
base = 'cf54c84d400b627dfa9b246b3800a82d65f061b8'
implementation = 'e0d5cb26d9e173f6dd4999ec7628cc30e7394e11'

def git(*args):
    return subprocess.check_output(['git', *args], cwd=root).decode('utf-8').strip()

def read(name):
    return json.loads((root / name).read_text(encoding='utf-8'))

def identity(path):
    path = Path(path)
    raw = path.read_bytes()
    return {'path': str(path), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}

head = git('rev-parse', 'HEAD')
assert git('branch', '--show-current') == 'codex/input-round14'
assert not git('status', '--porcelain')
assert git('merge-base', base, head) == base
assert git('rev-parse', 'origin/codex/input-round14') == head
assert not git('diff', '--name-only', implementation, head, '--', 'crates', 'adapters', 'apps',
               'scripts', 'tests', 'Cargo.toml', 'Cargo.lock', '.github', 'README.md',
               'THIRD_PARTY_NOTICES.md', 'data', 'licenses', 'config', 'docs/input-round19-design.md')
remote = read('.local/round19-remote/final/summary.json')
baseline = read('.local/round19-remote/baseline/summary.json')
assert remote['main'] == git('rev-parse', 'main') == git('rev-parse', 'origin/main') == 'a564fd47b0f054ff360592af3ccb8d744021160f'
assert remote['branch'] == remote['local_head'] == head
assert len(remote['pulls']) == 5 and all(p['draft'] and p['state'] == 'open' and not p['merged'] for p in remote['pulls'])
assert [(p['number'], p['head']) for p in baseline['pulls']] == [(p['number'], p['head']) for p in remote['pulls']]
ci = read('.local/round19-ci/final/summary.json')
downloads = read('.local/round19-ci/final-downloads/downloads.json')
assert ci['verified_pass'] and ci['head_sha'] == head == downloads['head']
assert len(ci['jobs']) == 2 and all(j['steps'] == 22 for j in ci['jobs'])
assert len(downloads['files']) == 4 and all(row['crc'] == 'passed' for row in downloads['files'])
for row in downloads['files']:
    path = root / '.local/round19-ci/final-downloads' / row['file']
    assert identity(path)['sha256'] == row['sha256']
    with zipfile.ZipFile(path) as archive:
        assert archive.testzip() is None
        for member in row['members']:
            raw = archive.read(member['name'])
            assert len(raw) == member['bytes'] and hashlib.sha256(raw).hexdigest() == member['sha256']
artifact = next(row for row in downloads['files'] if row['file'] == 'input-round19-windows.zip')
names = [row['name'] for row in artifact['members']]
for name in ['cloudleaf-cli.exe', 'cloudleaf-ime.exe', 'cloudleaf_windows_ime.dll',
             'input_trace.exe', 'scheme_trace.exe', 'train_language.exe', 'coverage_probe.exe']:
    assert any(value.endswith(name) for value in names), name
assert not any(Path(name).name in ['runtime.tsv', 'combined-runtime.tsv', 'character.tsv',
                                    'model-0.tsv', 'settings.json', 'pytorch_model.bin',
                                    'model.safetensors'] for name in names)

evidence = root / 'docs/reports/input-round19-evidence'
index = read('docs/reports/input-round19-evidence/index.json')
tree_raw = subprocess.check_output(['git', 'ls-tree', '-r', '-z', 'HEAD', '--',
                                    'docs/reports/input-round19-evidence'], cwd=root)
blobs = {}
for value in tree_raw.split(b'\0'):
    if value:
        metadata, name = value.split(b'\t', 1)
        blobs[name.decode('utf-8')] = metadata.split()[2].decode('ascii')
for name, row in index['files'].items():
    path = evidence / name
    raw = path.read_bytes()
    assert len(raw) == row['bytes'] and hashlib.sha256(raw).hexdigest() == row['sha256'], name
    blob = hashlib.sha1(b'blob ' + str(len(raw)).encode('ascii') + b'\0' + raw).hexdigest()
    assert blobs[path.relative_to(root).as_posix()] == blob, name
    if path.suffix == '.gz':
        unpacked = gzip.decompress(raw)
        assert hashlib.sha256(unpacked).hexdigest() == row['raw_sha256'], name
commands = read('.local/round19-locked-results.json')
assert len(commands) == 13 and all(row['returncode'] == 0 and row['head'] == implementation for row in commands)
for number in range(1, 14):
    result = read(f'.local/round19-runs/locked-{number:02}/result.json')
    assert result['returncode'] == 0 and result['head'] == implementation
    for name, digest in result['source_files_sha256'].items():
        assert identity(root / name)['sha256'] == digest, name
raw = (root / '.local/round19-runs/locked-04/stdout.txt').read_text(encoding='utf-8')
rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', raw)
assert [sum(int(row[i]) for row in rows) for i in range(3)] == [281, 0, 2]
assert 'Ran 71 tests' in (root / '.local/round19-runs/locked-08/stderr.txt').read_text(encoding='utf-8')
preservation = read('docs/reports/input-round19-evidence/preservation.json')
assert preservation['historical_paths_unchanged'] == 3767 and preservation['old_rust_tests_retained'] == 256
pause = read('.local/round19-ui-final/completion.json')
assert pause['paused_by_user'] and not pause['closed_owned_window_observed'] and not pause['TSF']
source = read('docs/reports/input-round19-evidence/source-identities.json')
for name, row in source['files'].items():
    assert identity(root / name)['sha256'] == row['sha256'], name
data = read('docs/reports/input-round19-evidence/data-identities.json')
for name, row in data['files'].items():
    assert identity(name)['sha256'] == row['sha256'], name
binaries = read('docs/reports/input-round19-evidence/binary-identities.json')
for name, row in binaries['current_local_release'].items():
    assert identity(root / '.local/round19-final-build/release' / name)['sha256'] == row['sha256'], name
record = {
    'schema_version': 1, 'round': 19, 'status': 'partial',
    'created_utc': datetime.now(timezone.utc).isoformat(), 'branch': 'codex/input-round14',
    'base': base, 'implementation_head': implementation, 'final_head': head,
    'clean_worktree': True, 'main_unchanged': True, 'PRs': remote['pulls'], 'CI': ci,
    'downloaded_CI': downloads, 'local_checks': commands,
    'tests': {'rust_passed': 281, 'rust_ignored': 2, 'rust_failed': 0, 'python_passed': 71},
    'evidence': {'index': identity(evidence / 'index.json'), 'verified_files': len(index['files']),
                 'preservation': preservation}, 'production_sources_match_full_local_checks': True,
    'data': data, 'binaries': binaries,
    'differential': {'strict_complete_candidate_comparisons': 188325, 'differences': 0,
                     'incomplete_reference_comparisons': 0, 'formal_language_accuracy': None},
    'resource_results': {'same_837_queries': True, 'old_budget_failures': 90, 'new_budget_failures': 6,
                         'unique_old_failures': 30, 'unique_fixed': 28,
                         'latency_gate': 'not passed: abbreviation tails/regressions and character P95 above 5ms retained',
                         '128MiB': 'unchanged internal estimated-capacity gate; whole-process RSS is separate'},
    'actual_window_tests': pause, 'formal_metrics': {'top1': None, 'top5': None, 'mrr': None},
    'research_only': True, 'production_approved': False,
    'not_implemented_or_accepted': ['Complete actual Windows user-manager and trustworthy Applied learning acceptance',
                                   'Microsoft/Natural Code layouts and Xiaohe correction',
                                   'formal independent human-gold language quality',
                                   'full TSF system/installation and long-duration acceptance',
                                   'incremental search preemption'],
    'not_run': ['TSF install/register/activate/uninstall', '8h/24h soak',
                'formal Windows application acceptance', 'human-gold quality evaluation'],
    'CL010': 'closed', 'PR6_created': False,
    'report': identity(root / 'docs/reports/input-round19-status.md'), 'tool': identity(__file__),
    'environment': {'platform': platform.platform(), 'python': platform.python_version()}
}
destination = root / '.local/round19-delivery-receipt.json'
with destination.open('x', encoding='utf-8', newline='\n') as stream:
    json.dump(record, stream, ensure_ascii=False, indent=2)
    stream.write('\n')
print(json.dumps({'head': head, 'status': record['status'], 'receipt': str(destination),
                  'CI': ci['url'], 'evidence_files': len(index['files'])}, ensure_ascii=False))
