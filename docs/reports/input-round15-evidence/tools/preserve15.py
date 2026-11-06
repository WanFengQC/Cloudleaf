"""只读核对原历史blob、数据与测试，保留来源工作区和main。"""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import subprocess
root = Path(__file__).resolve().parents[1]
def git(*args, cwd=root):
    return subprocess.check_output(['git', *args], cwd=cwd).decode('utf-8').strip()
base = '75e0acee88d21647010a56a9d876c136c95e6d5e'
head = git('rev-parse', 'HEAD')
names = git('ls-tree', '-r', '--name-only', base, '--', 'docs/reports', 'tests', 'data',
    'crates/cloudleaf-input/tests', 'crates/cloudleaf-search/tests', 'licenses').splitlines()
changed = sorted(set(git('diff', '--name-only', base, head).splitlines()).intersection(names))
assert not changed, changed
worktrees = []
for name, directory in [('shared', Path('C:/Users/EDY/WebstormProjects/Cloudleaf')),
    ('CL007', root.parents[1] / 'cl007-windows/Cloudleaf'), ('CL008', root.parents[1] / 'cl008-search/Cloudleaf'),
    ('CL009', root.parents[1] / 'cl009-data/Cloudleaf'), ('translation', root.parents[1] / 'translation-round11/Cloudleaf')]:
    assert git('status', '--porcelain', cwd=directory) == ''
    worktrees.append({'name': name, 'path': str(directory), 'head': git('rev-parse', 'HEAD', cwd=directory), 'clean': True})
for name in ['data/licenses/CC-BY-SA-4.0.txt', 'data/licenses/Unicode-3.0.txt']:
    original = subprocess.check_output(['git', 'show', base + ':' + name], cwd=root)
    assert (root / name).read_bytes() == original
record = {'at_utc': datetime.now(timezone.utc).isoformat(), 'base': base, 'head': head,
    'comparison': 'Original Git blob bytes; excludes new paths, not Windows checkout line-end normalization',
    'original_files_checked': len(names), 'all_original_files_unchanged': True, 'changed': changed,
    'source_worktrees': worktrees, 'main': git('rev-parse', 'main'), 'origin_main': git('rev-parse', 'origin/main'),
    'source_licenses_worktree_bytes_unchanged': True}
assert record['main'] == record['origin_main'] == 'a564fd47b0f054ff360592af3ccb8d744021160f'
destination = root / '.local/round15-preservation.json'
with destination.open('x', encoding='utf-8', newline='\n') as file:
    json.dump(record, file, ensure_ascii=False, indent=2); file.write('\n')
print(json.dumps({'head': head, 'original_files': len(names), 'unchanged': True, 'source_worktrees_clean': True}))
