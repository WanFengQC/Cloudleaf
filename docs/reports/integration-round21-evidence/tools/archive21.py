"""归档 Round 21 输出。只收录日志、报告、合成夹具和重放程序。"""
import gzip
import hashlib
import json
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
local = root / '.local'
out = root / 'docs/reports/integration-round21-evidence'
base = '714f8c9439148b23d2ce34dc3a4e672d59598c1d'
translation = 'cca77457ca3943ee5ad90d566a8d9ef6b8795cd2'
sha = lambda data: hashlib.sha256(data).hexdigest()

def git(*args):
    return subprocess.check_output(['git', *args], cwd=root).decode('utf-8').strip()

files = {}
def add(path, relative):
    assert not path.is_symlink(), path
    raw = path.read_bytes()
    stored = raw
    extra = {}
    if len(raw) > 80_000 and path.suffix != '.gz':
        relative += '.gz'
        stored = gzip.compress(raw, compresslevel=5, mtime=0)
        extra = {'original_bytes': len(raw), 'original_sha256': sha(raw)}
    assert relative not in files, relative
    destination = out / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open('xb') as stream:
        stream.write(stored)
    files[relative] = {'bytes': len(stored), 'sha256': sha(stored), **extra}

suffixes = {'.txt', '.md', '.json', '.jsonl', '.log', '.stdout', '.stderr', '.gz', '.py', '.rs', '.tsv', '.toml'}
def add_tree(folder):
    for path in sorted(folder.rglob('*')):
        if not path.is_file() or path.suffix not in suffixes:
            continue
        rel = path.relative_to(folder)
        if set(rel.parts) & {'target', '__pycache__', '.git', 'source', 'binary', 'binaries'}:
            continue
        if path.with_name(path.name + '.gz').is_file():
            continue
        assert path.stat().st_size < 100 * 1024 * 1024 or path.suffix in {'.json', '.txt', '.stdout'}, path
        add(path, f'{folder.name}/{rel.as_posix()}')

head = git('rev-parse', 'HEAD')
assert git('branch', '--show-current') == 'codex/integration-round21'
assert git('merge-base', base, head) == base
assert not git('diff', '--name-only', base, head, '--', 'crates/cloudleaf-search', 'crates/cloudleaf-pinyin',
               'crates/cloudleaf-user', 'crates/cloudleaf-types', 'adapters/windows', 'apps/windows-ime')
previous = set(git('ls-tree', '-r', '--name-only', base, '--', 'docs/reports', 'data', 'licenses').splitlines())
changed = set(git('diff', '--name-only', base, head).splitlines())
assert not previous & changed, sorted(previous & changed)
out.mkdir(exist_ok=False)
for arg in sys.argv[1:]:
    p = local / arg
    assert p.exists(), p
    if p.is_dir():
        add_tree(p)
    else:
        add(p, p.name)
for name in ('run21.py', 'full21.py', 'archive21.py', 'receipt21.py', 'discover21.py', 'collect_ci21.py', 'download_ci21.py', 'r21-original-prs.json'):
    add(local / name, f'tools/{name}')
for path in sorted(local.glob('r21_*.py')):
    add(path, f'tools/{path.name}')
source = {}
for name in git('ls-files', '--', 'crates', 'apps', 'adapters', 'scripts', 'config', 'vendor', 'Cargo.toml', 'Cargo.lock', '.github').splitlines():
    path = root / name
    if path.is_file():
        source[name] = sha(path.read_bytes())
release = local / 'target/release'
binary = {f: sha((release/f).read_bytes()) for f in (
    'cloudleaf-ime.exe', 'cloudleaf-cli.exe', 'cloudleaf_windows_ime.dll',
    'cloudleaf-translation-panel.exe', 'cloudleaf-bridge-headless.exe',
    'examples/offline_translate.exe', 'examples/offline_models.exe', 'examples/offline_lifecycle.exe',
    'examples/input_trace.exe', 'examples/scheme_trace.exe',
)}
index = {'input_baseline': base, 'translation_baseline': translation, 'implementation': head,
         'protected_input_history_paths': len(previous), 'source_sha256': source, 'binary_sha256': binary,
         'files': files}
(out / 'index.json').write_text(json.dumps(index, ensure_ascii=False, indent=2, sort_keys=True) + '\n', encoding='utf-8')
print(json.dumps({'files': len(files), 'bytes': sum(x['bytes'] for x in files.values()), 'implementation': head}))
