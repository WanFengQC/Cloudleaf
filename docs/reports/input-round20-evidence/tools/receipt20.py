"""仅在最终push的Windows和Linux结果、日志和产物全部验证后生成回执。"""
import gzip
import hashlib
import json
import re
import subprocess
import zipfile
from datetime import datetime, timezone
from pathlib import Path

root = Path(__file__).resolve().parents[1]
local = root / '.local'
evidence = root / 'docs/reports/input-round20-evidence'
base = '4d42363091361552ad525f47e3c6b1d581ae3a4c'

def git(*args):
    return subprocess.check_output(['git', *args], cwd=root).decode('utf-8').strip()

def load(path):
    return json.loads(path.read_text(encoding='utf-8'))

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

head = git('rev-parse', 'HEAD')
assert git('branch', '--show-current') == 'codex/input-round14'
assert not git('status', '--porcelain')
remote = load(local / 'round20-remote/final/summary.json')
prior = load(local / 'round20-remote/baseline/summary.json')
assert remote['branch'] == remote['local_head'] == git('rev-parse', 'origin/codex/input-round14') == head
assert remote['main'] == prior['main'] == git('rev-parse', 'main') == git('rev-parse', 'origin/main')
assert remote['pulls'] == prior['pulls'] and len(remote['pulls']) == 5
assert all(p['draft'] and p['state'] == 'open' and not p['merged'] for p in remote['pulls'])
ci = load(local / 'round20-ci/final/summary.json')
downloads = load(local / 'round20-ci/final-downloads/downloads.json')
assert ci['verified_pass'] and ci['head_sha'] == downloads['head'] == head
assert int(ci['run_id']) == int(downloads['run_id'])
assert {j['name'] for j in ci['jobs']} == {'core (ubuntu-latest)', 'core (windows-latest)'}
assert len(downloads['files']) == 4
assert {row['name'] + '.zip': row['digest'] for row in ci['artifacts']} == {
    row['file']: 'sha256:' + row['sha256'] for row in downloads['files'] if row['file'] != 'logs.zip'
}
for row in downloads['files']:
    path = local / 'round20-ci/final-downloads' / row['file']
    assert sha(path.read_bytes()) == row['sha256'] and row['crc'] == 'passed'
    with zipfile.ZipFile(path) as archive:
        assert archive.testzip() is None
        for member in row['members']:
            assert sha(archive.read(member['name'])) == member['sha256']
windows = next(row for row in downloads['files'] if row['file'] == 'input-round20-windows.zip')
members = {Path(member['name']).name for member in windows['members']}
assert {'cloudleaf-ime.exe', 'cloudleaf-cli.exe', 'cloudleaf_windows_ime.dll', 'input_trace.exe', 'scheme_trace.exe', 'train_language.exe', 'coverage_probe.exe', 'LICENSE', 'THIRD_PARTY_NOTICES.md'} <= members
assert not members & {'runtime.tsv', 'user-dictionary.tsv', 'model.safetensors', 'pytorch_model.bin', 'settings.json'}
license_members = [row['name'] for row in windows['members']
                   if row['name'] in {'LICENSE', 'THIRD_PARTY_NOTICES.md'} or row['name'].startswith('licenses/')]
with zipfile.ZipFile(local / 'round20-ci/final-downloads' / windows['file']) as archive:
    for name in license_members:
        original = subprocess.check_output(['git', 'show', f'HEAD:{name}'], cwd=root)
        assert archive.read(name).replace(b'\r\n', b'\n') == original.replace(b'\r\n', b'\n'), name
index = load(evidence / 'index.json')
assert index['baseline'] == base
assert git('merge-base', base, head) == base
assert not git('diff', '--name-only', index['implementation'], head, '--', 'crates', 'adapters', 'apps', 'scripts', 'tests', 'Cargo.toml', 'Cargo.lock', '.github', 'data', 'licenses', 'config')
tracked = {}
for entry in git('ls-tree', '-r', 'HEAD', '--', evidence.relative_to(root).as_posix()).splitlines():
    metadata, name = entry.split('\t', 1)
    tracked[name] = metadata.split()[2]
assert set(tracked) == {f'{evidence.relative_to(root).as_posix()}/{name}' for name in [*index['files'], 'index.json']}
git_hash = git('rev-parse', '--show-object-format')
for name, row in index['files'].items():
    raw = (evidence / name).read_bytes()
    assert len(raw) == row['bytes'] and sha(raw) == row['sha256'], name
    blob = hashlib.new(git_hash, f'blob {len(raw)}\0'.encode('ascii') + raw).hexdigest()
    assert blob == tracked[(evidence / name).relative_to(root).as_posix()], name
    if 'original_sha256' in row:
        assert sha(gzip.decompress(raw)) == row['original_sha256'], name
for name, digest in index['source_sha256'].items():
    assert sha((root / name).read_bytes()) == digest, name
for name, digest in index['local_release_sha256'].items():
    assert sha((local / 'round20-build/release' / name).read_bytes()) == digest, name
checks = load(local / 'round20-full-results.json')
assert len(checks) == 13 and all(c['returncode'] == 0 and c['head'] == index['implementation'] for c in checks)
raw = (local / 'round20-runs/full-04/stdout.txt').read_text(encoding='utf-8')
rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', raw)
tests = {name: sum(int(row[i]) for row in rows) for i,name in enumerate(['rust_passed', 'rust_failed', 'rust_ignored'])}
tests['python_passed'] = int(re.search(r'Ran (\d+) tests', (local / 'round20-runs/full-08/stderr.txt').read_text(encoding='utf-8'))[1])
record = {'round': 20, 'head': head, 'implementation': index['implementation'], 'baseline': base,
          'checked_at_utc': datetime.now(timezone.utc).isoformat(), 'status': 'partial', 'remote': remote,
          'CI': ci, 'downloads': downloads, 'tests': tests, 'local_checks': checks,
          'license_members_verified_against_git_after_crlf_normalization': len(license_members),
          'evidence_index_sha256': sha((evidence / 'index.json').read_bytes()), 'evidence_files': len(index['files']),
          'report_sha256': sha((root / 'docs/reports/input-round20-status.md').read_bytes()),
          'window_acceptance': 'not-run/user-paused', 'TSF': 'not-run', 'long_soak': 'not-run',
          'formal_quality': {'top1': None, 'top5': None, 'mrr': None}, 'CL010': 'closed'}
path = local / 'round20-delivery-receipt.json'
with path.open('x', encoding='utf-8', newline='\n') as stream:
    json.dump(record, stream, ensure_ascii=False, indent=2)
    stream.write('\n')
print(json.dumps({'head': head, 'CI': ci['url'], 'evidence_files': len(index['files']), 'tests': tests, 'receipt': str(path)}))
