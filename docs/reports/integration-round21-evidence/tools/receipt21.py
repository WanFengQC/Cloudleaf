"""验证最终提交、CI 和下载材料后生成本地回执。"""
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
evidence = root / 'docs/reports/integration-round21-evidence'
sha = lambda raw: hashlib.sha256(raw).hexdigest()

def git(*args):
    return subprocess.check_output(['git', *args], cwd=root).decode('utf-8').strip()

def read(path):
    return json.loads(path.read_text(encoding='utf-8'))

head = git('rev-parse', 'HEAD')
assert git('branch', '--show-current') == 'codex/integration-round21'
assert not git('status', '--porcelain')
refs = dict(line.split('\t')[::-1] for line in git('ls-remote', 'origin', 'refs/heads/*', 'refs/pull/[1-5]/head').splitlines())
assert refs['refs/heads/codex/integration-round21'] == head
assert refs['refs/heads/main'] == 'a564fd47b0f054ff360592af3ccb8d744021160f'
assert refs['refs/heads/codex/input-round14'] == '714f8c9439148b23d2ce34dc3a4e672d59598c1d'
old = read(local / 'r21-original-prs.json')
new = read(local / 'r21-final-prs.json')
assert old == new and len(new) == 5
pr_branches = ['cl007-009-contracts', 'cl008-search', 'cl009-data', 'cl007-windows', 'translation-round11']
for row, branch in zip(new, pr_branches, strict=True):
    assert row['state'] == 'open' and row['draft'] is True and not row['merged']
    assert refs[f"refs/pull/{row['number']}/head"] == refs[f'refs/heads/codex/{branch}'] == row['head']

ci = read(local / 'round21-ci/final/summary.json')
downloads = read(local / 'round21-ci/final-downloads/downloads.json')
assert ci['verified_pass'] and ci['head_sha'] == downloads['head'] == head
assert int(ci['run_id']) == int(downloads['run_id'])
assert {p['name'] for p in ci['jobs']} == {'core (ubuntu-latest)', 'core (windows-latest)'}
assert {p['name'] + '.zip': p['digest'] for p in ci['artifacts']} == {
    p['file']: 'sha256:' + p['sha256'] for p in downloads['files'] if p['file'] != 'logs.zip'
}
assert len(downloads['files']) == 5
licenses = 0
for row in downloads['files']:
    path = local / 'round21-ci/final-downloads' / row['file']
    assert sha(path.read_bytes()) == row['sha256'] and row['crc'] == 'passed'
    with zipfile.ZipFile(path) as archive:
        assert archive.testzip() is None
        for member in row['members']:
            name = member['name']
            raw = archive.read(name)
            assert sha(raw) == member['sha256']
            if name in {'LICENSE', 'THIRD_PARTY_NOTICES.md'} or name.startswith('licenses/'):
                original = subprocess.check_output(['git', 'show', f'HEAD:{name}'], cwd=root)
                assert raw.replace(b'\r\n', b'\n') == original.replace(b'\r\n', b'\n'), name
                licenses += 1
assert licenses > 0

index = read(evidence / 'index.json')
checks = read(local / 'round21-final-results.json')
assert len(checks) == 16 and all(r['returncode'] == 0 and r['head'] == index['implementation'] for r in checks)
for name, digest in index['source_sha256'].items():
    assert sha((root / name).read_bytes()) == digest, name
for name, digest in index['binary_sha256'].items():
    assert sha((local / 'target/release' / name).read_bytes()) == digest, name
prefix = evidence.relative_to(root).as_posix()
tracked = {line.split('\t',1)[1]: line.split()[2] for line in git('ls-tree','-r','HEAD','--',prefix).splitlines()}
assert set(tracked) == {f'{prefix}/{n}' for n in [*index['files'], 'index.json']}
git_hash = git('rev-parse', '--show-object-format')
for name, row in index['files'].items():
    raw = (evidence / name).read_bytes()
    assert sha(raw) == row['sha256'] and len(raw) == row['bytes'], name
    assert hashlib.new(git_hash, f'blob {len(raw)}\0'.encode('ascii') + raw).hexdigest() == tracked[f'{prefix}/{name}'], name
    if 'original_sha256' in row:
        assert sha(gzip.decompress(raw)) == row['original_sha256'], name
test = (local / 'round21-runs/round21-final-04/stdout.txt').read_text(encoding='utf-8')
rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', test)
totals = {n: sum(int(row[i]) for row in rows) for i,n in enumerate(['rust_passed','rust_failed','rust_ignored'])}
totals['python_passed'] = int(re.search(r'Ran (\d+) tests', (local / 'round21-runs/round21-final-08/stderr.txt').read_text(encoding='utf-8'))[1])
reports = {p.name: sha(p.read_bytes()) for p in (root / 'docs/reports').glob('integration-round21-*.md')}
assert len(reports) == 3
record = {'round': 21, 'status': 'partial', 'head': head, 'branch': 'codex/integration-round21',
          'implementation': index['implementation'], 'input_baseline': index['input_baseline'],
          'translation_baseline': index['translation_baseline'], 'checked_at_utc': datetime.now(timezone.utc).isoformat(),
          'refs': refs, 'original_drafts': new, 'ci': ci, 'downloads': downloads, 'tests': totals,
          'license_members_verified_against_git': licenses, 'reports_sha256': reports,
          'evidence_index_sha256': sha((evidence / 'index.json').read_bytes()), 'evidence_files': len(index['files']),
          'window_acceptance': 'not-run/user-paused', 'TSF': 'not-run', 'long_soak': 'not-run',
          'formal_quality': {'top1': None, 'top5': None, 'mrr': None}, 'CL010': 'closed'}
path = local / 'round21-delivery-receipt.json'
with path.open('x', encoding='utf-8', newline='\n') as stream:
    json.dump(record, stream, ensure_ascii=False, indent=2)
    stream.write('\n')
print(json.dumps({'head': head, 'tests': totals, 'ci': ci['url'], 'license_members': licenses, 'receipt': str(path)}))
