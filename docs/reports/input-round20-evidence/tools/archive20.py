"""归档Round20原始结果与可重放程序，数据、模型和二进制只记录身份。"""
import gzip
import hashlib
import json
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[1]
local = root / '.local'
temp = Path('C:/Users/EDY/.codex/tmp')
out = root / 'docs/reports/input-round20-evidence'
base = '4d42363091361552ad525f47e3c6b1d581ae3a4c'
sha = lambda raw: hashlib.sha256(raw).hexdigest()

def git(*args):
    return subprocess.check_output(['git', *args], cwd=root).decode('utf-8').strip()

files = {}

def add(path, name):
    raw = path.read_bytes()
    packed = raw
    row = {}
    if len(raw) > 100_000 and path.suffix not in {'.gz', '.zip'}:
        name = name + '.gz'
        packed = gzip.compress(raw, mtime=0)
        row = {'original_bytes': len(raw), 'original_sha256': sha(raw)}
    assert name not in files, name
    dest = out / name
    dest.parent.mkdir(parents=True, exist_ok=True)
    with dest.open('xb') as stream:
        stream.write(packed)
    files[name] = {**row, 'bytes': len(packed), 'sha256': sha(packed)}

def tree(folder, prefix):
    for path in sorted(folder.rglob('*')):
        if not path.is_file() or path.suffix.lower() in {'.exe', '.dll', '.pdb', '.pyc'}:
            continue
        if path.with_name(path.name + '.gz').is_file() and gzip.decompress(path.with_name(path.name + '.gz').read_bytes()) == path.read_bytes():
            continue
        add(path, f'{prefix}/{path.relative_to(folder).as_posix()}')

delivery_folders = ((temp / 'r20-search-delivery', 'search'), (temp / 'r20-cache-delivery', 'cache'))
for folder, _ in delivery_folders:
    assert folder.is_dir(), folder
assert (local / 'round20-engine-host-performance/summary.json').is_file()
out.mkdir(exist_ok=False)
for name in ('round20-runs', 'round20-engine-host-performance', 'round20-locked-algorithm-v1'):
    tree(local / name, name.removeprefix('round20-'))
for name in ('round20-full-results.json', 'round20-locked-dependency-audit.json', 'round20-locked-performance.json'):
    add(local / name, name)
add(local / 'round20-baseline/identity.json', 'baseline/identity.json')
add(local / 'round20-remote/baseline/summary.json', 'baseline/remote.json')
for name in ('archive20', 'run20', 'full20', 'baseline20', 'collect_ci20', 'discover20', 'download_ci20', 'remote20', 'receipt20', 'report20_metrics'):
    add(local / f'{name}.py', f'tools/{name}.py')
tree(temp / 'r20-integration', 'integration')
for folder, name in delivery_folders:
    tree(folder, name)

historical = set(git('ls-tree', '-r', '--name-only', base, '--', 'docs/reports', 'data', 'tests', 'licenses').splitlines())
changed = set(git('diff', '--name-only', base, 'HEAD').splitlines())
assert not historical & changed, sorted(historical & changed)
implementation = git('rev-parse', 'HEAD')
source = {}
for name in git('ls-files', '--', 'crates', 'adapters', 'apps', 'scripts', '.github', 'Cargo.toml', 'Cargo.lock').splitlines():
    path = root / name
    if path.suffix in {'.rs', '.py', '.toml', '.yml', '.lock'}:
        source[name] = sha(path.read_bytes())
bins = {}
release = local / 'round20-build/release'
for name in ('cloudleaf-cli.exe', 'cloudleaf-ime.exe', 'cloudleaf_windows_ime.dll', 'examples/input_trace.exe', 'examples/scheme_trace.exe', 'examples/train_language.exe', 'examples/coverage_probe.exe'):
    bins[name] = sha((release / name).read_bytes())
data = json.loads((local / 'round20-engine-host-performance/summary.json').read_text(encoding='utf-8'))['data']
index = {'baseline': base, 'implementation': implementation, 'historical_paths_unchanged': len(historical),
         'files': files, 'source_sha256': source, 'local_release_sha256': bins, 'data': data}
with (out / 'index.json').open('x', encoding='utf-8', newline='\n') as stream:
    json.dump(index, stream, ensure_ascii=False, indent=2, sort_keys=True)
    stream.write('\n')
print(json.dumps({'files': len(files), 'bytes': sum(row['bytes'] for row in files.values()), 'implementation': implementation}))
