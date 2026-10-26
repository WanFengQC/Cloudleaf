"""独立核对本轮工作区、历史证据及发布产物，不修改旧记录。"""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
STARTS = {
    ROOT: 'b97648878bd5e3973b3a1548f0bf20eb8943fcf5',
    Path(r'C:\Users\EDY\.codex\worktrees\cl007-windows\Cloudleaf'): 'bb6aea1fc2d5a60827dcdfc8053fa5ba25f36788',
    Path(r'C:\Users\EDY\.codex\worktrees\cl008-search\Cloudleaf'): '1a7b0065a1c05c075eb93a0d816a564cacdb407b',
    Path(r'C:\Users\EDY\.codex\worktrees\cl009-data\Cloudleaf'): 'e3dc4f92a56a1c005491fcaf0f7b798755dd754e',
}

def git(root, *args):
    return subprocess.run(['git', *args], cwd=root, capture_output=True, check=True).stdout

rows = []
for root, start in STARTS.items():
    old_reports = [p for p in git(root, 'ls-tree', '-r', '--name-only', start, 'docs/reports').decode('utf-8').splitlines()]
    changed = []
    for name in old_reports:
        path = root / name
        # Git检出可能采用CRLF；按各路径clean过滤后的blob核验，另保留磁盘SHA。
        if not path.is_file() or git(root, 'hash-object', f'--path={name}', str(path)).strip() != git(root, 'rev-parse', f'{start}:{name}').strip():
            changed.append(name)
    assert not changed, (root, changed)
    deleted = git(root, 'diff', '--diff-filter=D', '--name-only', start, '--', 'crates', 'tests', 'adapters').decode('utf-8').splitlines()
    assert not deleted, (root, deleted)
    resources = {}
    for name in ['data/dictionaries/prototype.tsv', 'tests/fixtures/input-v1.tsv', 'tests/evaluation/algorithm-v1.tsv']:
        data = (root / name).read_bytes()
        assert git(root, 'hash-object', f'--path={name}', str(root / name)).strip() == git(root, 'rev-parse', f'{start}:{name}').strip()
        resources[name] = hashlib.sha256(data).hexdigest()
    rows.append({'path': str(root), 'start_sha': start, 'head': git(root, 'rev-parse', 'HEAD').decode().strip(),
                 'historical_reports_checked': len(old_reports), 'changed_historical_reports': changed,
                 'deleted_core_tests_adapters': deleted, 'protected_resource_sha256': resources,
                 'scope': 'Git按路径clean后的blob一致与文件删除核对；CRLF检出不误记为修改，不替代测试断言语义审查'})
print(json.dumps({'verified': True, 'directions': rows}, ensure_ascii=False, indent=2))
