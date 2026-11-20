"""收集搜索专题证据，原始查询、失败记录和构建标识一并保留。"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess


root = Path(__file__).resolve().parent
out = root.parent / 'r20-search-delivery'
out.mkdir(exist_ok=False)
repo = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
base = '4d42363091361552ad525f47e3c6b1d581ae3a4c'
implementation = '6be40b75bf2312563c7602f5daac174efb651c20'
sha = lambda raw: hashlib.sha256(raw).hexdigest()


def add(path, relative):
    dest = out / relative
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(path, dest)


def git_show(commit, path):
    return subprocess.check_output(['git', 'show', f'{commit}:{path}'], cwd=repo)


for name in ('profile', 'baseline-replay', 'resource-old', 'resource-seed', 'seed-trial', 'runs', 'paired837'):
    shutil.copytree(root / name, out / name)
for name in ('design.md', 'paired837.py', 'deliver_search.py', 'run_trial.py', 'run_resource.py',
             'run_profile.py', 'profile_setup.py', 'setup.py', 'command.py', 'finalize_seed.py',
             'seed_trial.py', 'baseline-source.json', 'baseline-search.rs', 'seed-search.rs',
             'inputs.tsv', 'addone.tsv', 'witten.tsv', 'lexical.tsv', 'bigram.tsv'):
    add(root / name, name)
for path in (root / 'source/probe').rglob('*'):
    if path.is_file():
        add(path, path.relative_to(root / 'source'))
for name in ('Cargo.toml', 'Cargo.lock',
             'crates/cloudleaf-search/tests/incumbent_resources.rs'):
    add(root / 'source' / name, Path('source') / name)
add(root.parent / 'r18-scheme-diff/lexicon.tsv', 'mechanical-lexicon.tsv')
for name in ('full', 'abbreviated', 'xiaohe', 'fuzzy-nl', 'correction'):
    add(repo / f'docs/reports/input-round18-evidence/performance/{name}-cases.tsv', f'fixed-cases/{name}.tsv')

prod = 'crates/cloudleaf-search/src/search.rs'
assert (root / 'baseline-search.rs').read_bytes() == git_show(base, prod)
assert (root / 'source' / prod).read_bytes() == git_show(implementation, prod)
compiled = {}
for crate in ('cloudleaf-types', 'cloudleaf-pinyin', 'cloudleaf-search'):
    manifest = f'crates/{crate}/Cargo.toml'
    add(root / 'source' / manifest, 'source/' + manifest)
    for path in (root / 'source/crates' / crate / 'src').rglob('*.rs'):
        name = path.relative_to(root / 'source').as_posix()
        raw = path.read_bytes()
        expected = git_show(implementation, name)
        assert raw == expected, name
        compiled[name] = {'temporary_sha256': sha(raw), 'git_sha256': sha(expected), 'byte_equal': raw == expected}
        add(path, 'source/' + name)
build_summary = {
    'base': base, 'implementation': implementation, 'search_sha256': sha(git_show(implementation, prod)),
    'old_search_sha256': sha(git_show(base, prod)), 'compiled_sources': compiled,
    'tool_sha256': sha(Path(__file__).read_bytes()),
    'strict_probes': {name: sha((root / 'source/target/release' / (name + '.exe')).read_bytes()) for name in ('diff', 'preference_diff')},
}
(out / 'source-binding.json').write_text(json.dumps(build_summary, ensure_ascii=False, indent=2), encoding='utf-8')

summary = json.loads((root / 'paired837/summary.json').read_text(encoding='utf-8'))
assert len(summary['complete_mismatches']) == 0
assert all(json.loads((root / 'runs' / name / 'result.json').read_text(encoding='utf-8'))['returncode'] == 0
           for name in ('final-diff', 'final-preference-diff', 'seed-final-20261120-quick', 'final-incumbent-tests'))
strict = {}
for name in ('final-diff', 'final-preference-diff'):
    with (root / 'runs' / name / 'stdout.txt').open('rb') as stream:
        stream.seek(-min(4096, stream.seek(0, 2)), 2)
        line = stream.read().splitlines()[-1].decode('utf-8')
    assert line.startswith('SUMMARY\t') and '\tfailures=0\t' in line
    strict[name] = json.loads(line.split('\t')[-2])

compact = {
    'scope': summary['scope'], 'implementation': implementation, 'totals': summary['totals'],
    'strict': strict, 'complete_pairs': summary['complete_pairs'], 'complete_mismatches': 0,
    'timing_p95_ns': {key: r['total']['p95'] for key, r in summary['metrics_ns'].items()},
    'former_budget_cases': [
        {'kind': r['kind'], 'scheme': r['scheme'], 'iteration': r['iteration'],
         'case': r['r19']['case'],
         'r19': {k: v for k, v in r['r19'].items() if k != 'result'},
         'r20': {k: v for k, v in r['r20'].items() if k != 'result'}}
        for r in summary['budget_rows']
    ],
}
(out / 'summary.json').write_text(json.dumps(compact, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(compact, ensure_ascii=False))
