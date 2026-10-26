"""独立核对已发布方向证据及C/B冻结包的真实字节。"""
import hashlib
import json
from pathlib import Path
import subprocess

B = Path(r'C:\Users\EDY\.codex\worktrees\cl008-search\Cloudleaf')
C = Path(r'C:\Users\EDY\.codex\worktrees\cl009-data\Cloudleaf')
A = Path(r'C:\Users\EDY\.codex\worktrees\cl007-windows\Cloudleaf')

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def git(root, *args):
    return subprocess.check_output(['git', *args], cwd=root)

root = B / 'docs/reports/cl008-round4'
provenance = json.loads((root / 'provenance.json').read_text(encoding='utf-8'))
for name, expected in provenance['files_sha256'].items():
    blob = git(B, 'show', 'HEAD:' + (root / name).relative_to(B).as_posix())
    assert hashlib.sha256(blob).hexdigest() == expected, name

release_root = C / '.local/round4-clean-0dee2e2'
files = sorted(path.name for path in (release_root / 'release-a').iterdir())
assert len(files) == 9
final_sha = {name: sha(release_root / 'release-a' / name) for name in files}
assert final_sha == {name: sha(release_root / 'release-b' / name) for name in files}
early = C / '.local/round4-clean-2401b16/release-a'
copied = B / '.local/real-lexicon/cccedict-round4-2401b16'
early_sha = {name: sha(early / name) for name in files}
assert early_sha == {name: sha(copied / name) for name in files}
assert [name for name in files if final_sha[name] != early_sha[name]] == ['manifest.json']
assert sha(C / '.local/round4-source-cedict/cccedict-20260604-mirror7019e1c.zip') == 'fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a'
interruption = json.loads((C / 'docs/reports/cl009/round4-final-real-interruption.json').read_text(encoding='utf-8'))
assert len(interruption['cases']) == 7
assert all(row['exit_code'] == 73 and row['recovery']['recovered'] for row in interruption['cases'])
report = {'verified': True, 'B_preserved_published_git_blob_sha256_count': len(provenance['files_sha256']),
          'B_measured_matrix_commit': provenance['measured_implementation_commit'],
          'C_implementation_commit': '0dee2e27fa7749d952df24e44bfaefc9a2331c6d',
          'C_final_nine_files_identical': final_sha, 'B_frozen_early_nine_files_unchanged': early_sha,
          'cross_builder_changed_files': ['manifest.json'], 'C_completed_process_interruption_points': 7,
          'scope': '独立读取发布Git blob及磁盘冻结包；不重写样本或推导语言质量，不代替物理断电/实际应用验收'}
checks = json.loads((A / 'docs/reports/cl007/round4-owner/checksums.json').read_text(encoding='utf-8'))['sha256']
for name, expected in checks.items():
    name = 'docs/reports/cl007/round4-owner/' + name
    assert hashlib.sha256(git(A, 'show', 'HEAD:' + name)).hexdigest() == expected, name
report['A_published_git_blob_sha256_count'] = len(checks)
report['A_implementation_commit'] = '5c0ba9d651038eca5b3fcb53dba3f6835cf13a0c'
report['A_final_commit'] = git(A, 'rev-parse', 'HEAD').decode().strip()
changed = git(A, 'diff', '--name-only', report['A_implementation_commit'], 'HEAD').decode().splitlines()
assert all(name.startswith('docs/reports/cl007/round4-') or name == '.gitattributes' for name in changed)
report['A_measured_code_unchanged'] = True
print(json.dumps(report, ensure_ascii=False, indent=2))
