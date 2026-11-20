"""只读比较固定远程分支；merge-tree只写悬空Git对象，不移动引用或工作树。"""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import subprocess

ROOT = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
OUT = Path(__file__).resolve().parent
OUT.mkdir(parents=True, exist_ok=True)
records = []

def run(label, *args, expected=(0,)):
    command = ['git', *args]
    result = subprocess.run(command, cwd=ROOT, capture_output=True)
    for name, value in [('stdout', result.stdout), ('stderr', result.stderr)]:
        (OUT / f'{label}.{name}.txt').write_bytes(value)
    records.append({'label': label, 'command': command, 'returncode': result.returncode,
                    'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
                    'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()})
    assert result.returncode in expected, (label, result.returncode, result.stderr.decode('utf-8', errors='replace'))
    return result.stdout.decode('utf-8')

names = {
    'main': 'origin/main',
    'input': 'origin/codex/input-round14',
    'shared': 'origin/codex/cl007-009-contracts',
    'search': 'origin/codex/cl008-search',
    'data': 'origin/codex/cl009-data',
    'windows': 'origin/codex/cl007-windows',
    'translation': 'origin/codex/translation-round11',
}
refs = {name: run('head-' + name, 'rev-parse', ref).strip() for name, ref in names.items()}
run('status-before', 'status', '--porcelain=v1')
run('refs-before', 'show-ref')
summary = {'at_utc': datetime.now(timezone.utc).isoformat(), 'refs': refs, 'branches': {}}
for name in ['shared', 'search', 'data', 'windows', 'translation']:
    head = refs[name]
    base = run(name + '-merge-base', 'merge-base', refs['input'], head).strip()
    counts = run(name + '-ahead-behind', 'rev-list', '--left-right', '--count', refs['input'] + '...' + head).strip().split()
    own = run(name + '-own-files', 'diff', '--name-only', base, head).splitlines()
    input_files = run(name + '-input-files', 'diff', '--name-only', base, refs['input']).splitlines()
    overlap = sorted(set(own).intersection(input_files))
    same = []
    different = []
    for path in overlap:
        result = subprocess.run(['git', 'diff', '--quiet', refs['input'], head, '--', path], cwd=ROOT)
        (same if result.returncode == 0 else different).append(path)
    patch_lines = run(name + '-patch-equivalence', 'cherry', refs['input'], head).splitlines()
    run(name + '-commits-input', 'log', '--format=%H%x09%s', base + '..' + refs['input'])
    run(name + '-commits-branch', 'log', '--format=%H%x09%s', base + '..' + head)
    run(name + '-diffstat', 'diff', '--stat', base, head)
    preview = run(name + '-merge-preview', 'merge-tree', '--write-tree', '--messages', refs['input'], head, expected=(0, 1))
    conflict_messages = [line for line in preview.splitlines() if line.startswith('CONFLICT')]
    summary['branches'][name] = {'head': head, 'merge_base': base,
        'input_only_commits': int(counts[0]), 'branch_only_commits': int(counts[1]),
        'is_ancestor': int(counts[1]) == 0,
        'own_changed_files': len(own), 'input_changed_files': len(input_files),
        'overlap': overlap, 'overlap_identical': same, 'overlap_different': different,
        'patch_equal_commits': sum(line.startswith('-') for line in patch_lines),
        'patch_unique_commits': sum(line.startswith('+') for line in patch_lines),
        'merge_conflicts': conflict_messages}
    for path in ['Cargo.toml', 'Cargo.lock', '.github/workflows/ci.yml', 'README.md',
                 'crates/cloudleaf-input/src/lib.rs', 'crates/cloudleaf-types/src/lib.rs']:
        run(name + '-file-' + path.replace('/', '_'), 'show', head + ':' + path)
run('refs-after', 'show-ref')
run('status-after', 'status', '--porcelain=v1')
assert (OUT / 'refs-before.stdout.txt').read_bytes() == (OUT / 'refs-after.stdout.txt').read_bytes()
summary['refs_moved'] = False
summary['worktree_modified'] = False
(OUT / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
(OUT / 'commands.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps({'refs': refs, 'branches': {name: {k: row[k] for k in ['input_only_commits', 'branch_only_commits', 'merge_base', 'patch_equal_commits', 'patch_unique_commits', 'merge_conflicts']} for name, row in summary['branches'].items()}}, ensure_ascii=False, indent=2))
