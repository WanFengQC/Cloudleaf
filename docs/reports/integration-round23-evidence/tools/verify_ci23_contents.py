"""核对最终 CI ZIP 中工作流承诺的完整工具、许可和来源锁。"""
import hashlib
import json
import re
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HEAD = sys.argv[1]
OUTPUT = ROOT / '.local/round23-ci/final-downloads'

def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)

def normalized(data):
    return data.replace(b'\r\n', b'\n')

assert git('rev-parse', 'HEAD').decode().strip() == HEAD
workflow = git('show', f'{HEAD}:.github/workflows/ci.yml').decode('utf-8')
lock = hashlib.sha256(git('show', f'{HEAD}:Cargo.lock')).hexdigest()
summary = {'head': HEAD, 'Cargo.lock_sha256': lock, 'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'artifacts': {}}

for platform in ('windows', 'linux'):
    name = f'integration-round23-{platform}'
    upload = workflow.split(f'          name: {name}\n', 1)[1].split('          if-no-files-found:', 1)[0]
    paths = re.findall(r'^ {12}(\S+)$', upload, re.MULTILINE)
    required = {}
    for path in paths:
        if path.startswith('target/'):
            required[path] = 'binary'
            continue
        expanded = git('ls-tree', '-r', '--name-only', '-z', HEAD, '--', path).decode('utf-8').split('\0')
        entries = [entry for entry in expanded if entry]
        assert entries, path
        required.update({entry: 'tracked' for entry in entries})
    records = {}
    with zipfile.ZipFile(OUTPUT / (name + '.zip')) as archive:
        members = [member.filename for member in archive.infolist() if not member.is_dir()]
        assert len(members) == len(set(members))
        assert set(members) == set(required), {'missing': sorted(set(required) - set(members)), 'extra': sorted(set(members) - set(required))}
        for member, kind in sorted(required.items()):
            raw = archive.read(member)
            if kind == 'tracked':
                original = git('show', f'{HEAD}:{member}')
                assert normalized(raw) == normalized(original), member
            records[member] = {'kind': kind, 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}
    summary['artifacts'][name] = {'members': records, 'binary_count': sum(kind == 'binary' for kind in required.values()),
                                  'license_count': sum(member.startswith('licenses/') or member in {'LICENSE', 'THIRD_PARTY_NOTICES.md'} for member in required)}

for platform in ('windows-latest', 'ubuntu-latest'):
    name = f'core-performance-{platform}'
    expected = {
        'm2-performance.json', 'input-dependency-audit.json', 'translation-dependency-audit.json',
        'algorithm-v1/case-comparison.tsv', 'algorithm-v1/raw-results.tsv', 'algorithm-v1/report.md', 'algorithm-v1/summary.json',
    }
    with zipfile.ZipFile(OUTPUT / (name + '.zip')) as archive:
        members = {member.filename.removeprefix('.local/'): member.filename for member in archive.infolist() if not member.is_dir()}
        assert set(members) == expected, {'missing': sorted(expected - set(members)), 'extra': sorted(set(members) - expected)}
        raw = {member: archive.read(original) for member, original in members.items()}
        assert json.loads(raw['m2-performance.json'])['git_head'] == HEAD
        provenance = json.loads(raw['algorithm-v1/summary.json'])['provenance']
        assert provenance['git_head'] == HEAD and not provenance['git_tracked_dirty']
        assert json.loads(raw['input-dependency-audit.json'])['git_head'] == HEAD
        for report, rc in (('input-dependency-audit.json', 'audit_exit_code'), ('translation-dependency-audit.json', 'returncode')):
            data = json.loads(raw[report])
            assert data['lock_sha256'] == lock and data[rc] == 0 and data['licenses'], report
        summary['artifacts'][name] = {'members': {member: hashlib.sha256(data).hexdigest() for member, data in sorted(raw.items())}}
with (OUTPUT / 'contents-verification.json').open('x', encoding='utf-8', newline='\n') as stream:
    json.dump(summary, stream, indent=2, sort_keys=True)
    stream.write('\n')
print(json.dumps({name: len(row['members']) for name, row in summary['artifacts'].items()}))

