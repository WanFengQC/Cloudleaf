"""在精确提交上重放旧搜索、机械路径及固定候选的完整结果。"""

import argparse
import gzip
import hashlib
import json
import platform
import re
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / '.local'))

from r21_evidence_common import (
    LOCAL, ROOT, R20, compress, get_json, git, head, identity, now, percentile, put_json, run,
)

parser = argparse.ArgumentParser()
parser.add_argument('--label', default='round24-search-replay')
parser.add_argument('--expect-head', required=True)
parser.add_argument('--expect-source-change', action='append', default=[],
                    help='必须存在且与旧版不同的编译文件，完整结果差分保持严格')
args = parser.parse_args()
if args.expect_head != head():
    raise ValueError('指定提交与本地 HEAD 不一致')
if Path(args.label).name != args.label or args.label in {'', '.', '..'}:
    raise ValueError('报告目录名无效')
output = LOCAL / args.label
output.mkdir(exist_ok=False)
archive = ROOT / 'docs/reports/input-round20-evidence/search'
previous = get_json(archive / 'paired837/summary.json')
source_binding = get_json(archive / 'source-binding.json')
baseline = Path(previous['binaries']['r20']['path'])
assert identity(baseline)['sha256'] == previous['binaries']['r20']['sha256']
runtime = Path(previous['runtime']['path'])
models = {name: Path(value['path']) for name, value in previous['models'].items()}
cases = {name: ROOT / 'docs/reports/input-round18-evidence/performance' / f'{name}-cases.tsv'
         for name in previous['inputs']}
for path, expected in [(runtime, previous['runtime']), *
                       ((p, previous['models'][name]) for name, p in models.items()), *
                       ((p, previous['inputs'][name]) for name, p in cases.items())]:
    assert identity(path)['sha256'] == expected['sha256'], path

mechanical = archive / 'mechanical-lexicon.tsv'
assert identity(mechanical)['sha256'] == '8fafc9fc19ef779b810304ff056e566cb6b46247c22da6e9412a06f76a45bf45'
probe = output / 'source'
probe.mkdir()
shutil.copyfile(archive / 'source/Cargo.toml', probe / 'Cargo.toml')
shutil.copytree(archive / 'probe', probe / 'probe')
bindings = {}
legacy_projection = {}
source_changes = {}
research_only = {
    'crates/cloudleaf-search/src/recall.rs',
    'crates/cloudleaf-search/src/word_context.rs',
}
for name in ('cloudleaf-search', 'cloudleaf-pinyin', 'cloudleaf-types'):
    relative = Path('crates') / name
    for p in [relative / 'Cargo.toml', *((s.relative_to(ROOT)) for s in (ROOT / relative / 'src').rglob('*.rs'))]:
        key = p.as_posix()
        current = (ROOT / p).read_bytes()
        assert current == git('show', f'{args.expect_head}:{key}'), key
        dest = probe / p
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(current)
        bindings[key] = identity(dest)['sha256']
        if key in research_only:
            continue
        # 仅投影声明以逐字节检查旧行为源码，探针仍编译未投影的完整源码。
        projection = current
        if key == 'crates/cloudleaf-search/src/lib.rs':
            projection = projection.replace(b'mod word_context;\n', b'', 1)
            projection = projection.replace(
                b'pub use word_context::{\n'
                b'    MAX_RESEARCH_POOL, ResearchRanker, ResearchScore, WordContextModel, expected_reachability,\n'
                b'};\n\n', b'', 1,
            )
            projection = projection.replace(
                b'    Candidate, Features, RecallCandidate, RecallResult, SearchResult, Statistics, recall_context,\n'
                b'    recall_interleaved, reference_recall_context, search_graph, search_graph_cancellable,\n',
                b'    Candidate, Features, SearchResult, Statistics, search_graph, search_graph_cancellable,\n', 1,
            )
        elif key == 'crates/cloudleaf-search/src/search.rs':
            projection = projection.replace(
                b'#[path = "recall.rs"]\nmod recall;\npub use recall::{\n'
                b'    RecallCandidate, RecallResult, recall_context, recall_interleaved, reference_recall_context,\n'
                b'};\n\n', b'', 1,
            )
        legacy = git('show', f'{R20}:{key}')
        if key in args.expect_source_change:
            assert projection != legacy, key
            source_changes[key] = {'legacy': hashlib.sha256(legacy).hexdigest(),
                                   'current': bindings[key]}
            continue
        assert projection == legacy, key
        legacy_projection[key] = hashlib.sha256(projection).hexdigest()
        if key in source_binding['compiled_sources']:
            assert legacy_projection[key] == source_binding['compiled_sources'][key]['git_sha256']

assert set(source_changes) == set(args.expect_source_change)
flags = ['--manifest-path', probe / 'Cargo.toml']
run(output / 'toolchain', ['rustc', '-Vv'], sample=False)
run(output / 'resolve', ['cargo', 'generate-lockfile', '--offline', *flags], sample=False)
run(output / 'build', ['cargo', 'build', '--release', '--locked', '--offline', '-j2', '--target-dir', probe / 'target', *flags],
    deadline_seconds=240, sample=False)
bins = {'r20': baseline, 'r24': probe / 'target/release/r19-search-probe.exe'}
record = {
    'started_utc': now(), 'baseline': R20, 'head': head(), 'machine': platform.platform(),
    'runtime': identity(runtime), 'models': {k: identity(v) for k, v in models.items()},
    'fixtures': {k: identity(v) for k, v in cases.items()},
    'mechanical_fixture': identity(mechanical),
    'binaries': {k: identity(v) for k, v in bins.items()},
    'tool': identity(__file__), 'common': identity(LOCAL / 'r21_evidence_common.py'),
    'compiled_sources': bindings,
    'legacy_source_projection': legacy_projection,
    'reviewed_source_changes': source_changes,
    'probe_source': {p.relative_to(probe).as_posix(): identity(p)['sha256']
                     for p in (probe/'probe').rglob('*') if p.is_file()},
    'probe_lock': identity(probe / 'Cargo.lock'),
    'limits': {'top_k': 10, 'expansions': 200000, 'resident_paths': 50000},
    'scope': 'same source-bound search-only probes; isolated from TSF, HWND and actual input host',
    'comparison': 'full SearchResult debug representation including provenance, scores, output order, fallback, completeness, and budget diagnostics; excludes two timing columns only',
    'formal_language_metrics': {'top1': None, 'top5': None, 'mrr': None},
    'strict': {},
}

for name, expected in [('diff', 83700), ('preference_diff', 104625)]:
    folder = output / f'strict-{name}'
    exe = probe / f'target/release/{name}.exe'
    meta = run(folder, [exe, mechanical, '7'*64], cwd=folder.parent, sample=False)
    with (folder / 'stdout.txt').open('r', encoding='utf-8') as log:
        result = [line.rstrip('\n') for line in log if line.startswith('SUMMARY\t')]
    assert len(result) == 1
    fields = result[0].split('\t')
    totals = json.loads(fields[4])
    assert fields[3] == 'failures=0'
    assert totals['candidate_comparisons_attempted'] == totals['complete_candidate_comparisons'] == expected
    record['strict'][name] = {'totals': totals, 'binary': identity(exe), 'returncode': meta['returncode'],
                              'stdout': compress(folder / 'stdout.txt')}
    put_json(output / 'summary.json', record)

def collect(folder, kind, scheme, version):
    meta = run(folder, [bins[version], runtime, previous['runtime']['sha256'], models[kind],
                       cases[scheme], scheme])
    rows = []
    for line in (folder / 'stdout.txt').read_text(encoding='utf-8').splitlines():
        _, label, raw, build, search, result = line.split('\t', 5)
        rows.append({'case': label, 'raw': raw,
                     'lattice_ns': int(build.removeprefix('lattice_ns=')),
                     'search_ns': int(search.removeprefix('search_ns=')),
                     'budget': re.search(r'budget: (None|Some\(\w+\))', result)[1],
                     'result': result})
    assert len(rows) == len(cases[scheme].read_text(encoding='utf-8').splitlines())
    return rows, meta


pairs, resource = [], {v: [] for v in bins}
for round_number in range(1, 4):
    for i, kind in enumerate(models):
        for j, scheme in enumerate(cases):
            versions = list(bins)
            if (round_number + i + j) % 2 == 0:
                versions.reverse()
            measured = {}
            for version in versions:
                folder = output / 'pairs' / f'{round_number}-{kind}-{scheme}-{version}'
                measured[version], meta = collect(folder, kind, scheme, version)
                resource[version].extend(meta['memory'])
            for a, b in zip(measured['r20'], measured['r24'], strict=True):
                assert (a['case'], a['raw']) == (b['case'], b['raw'])
                pairs.append({'iteration': round_number, 'model': kind, 'scheme': scheme,
                              'r20': a, 'r24': b, 'equal': a['result'] == b['result']})

assert len(pairs) == 837
record['finished_utc'] = now()
record['queries_per_version'] = len(pairs)
record['exact_full_results_equal'] = sum(row['equal'] for row in pairs)
record['budget_failures'] = {v: sum(row[v]['budget'] != 'None' for row in pairs) for v in bins}
record['timing_ns'] = {}
for model in models:
    for scheme in cases:
        observations = [r for r in pairs if r['model'] == model and r['scheme'] == scheme]
        for v in bins:
            record['timing_ns'][f'{model}/{scheme}/{v}'] = {
                'lattice': percentile([r[v]['lattice_ns'] for r in observations]),
                'search': percentile([r[v]['search_ns'] for r in observations]),
                'total': percentile([r[v]['lattice_ns'] + r[v]['search_ns'] for r in observations]),
                'budget_failures': sum(r[v]['budget'] != 'None' for r in observations),
            }
record['whole_process_memory'] = {
    v: {'samples': len(m), 'peak_working': max(r['peak_working_bytes'] for r in m),
        'observed_private': max(r['private_bytes'] for r in m)} for v, m in resource.items()
}


def behavior_hash(rows, current):
    projection = [
        (row['iteration'], row['model'], row['scheme'],
         [(row[version]['case'], row[version]['raw'], row[version]['budget'], row[version]['result'])
          for version in ('r20', current)])
        for row in rows
    ]
    encoded = json.dumps(projection, ensure_ascii=False, separators=(',', ':')).encode()
    return hashlib.sha256(encoded).hexdigest()


prior_path = LOCAL / 'round22-search-fixed/pairs.json.gz'
prior_summary = get_json(prior_path.with_name('summary.json'))
prior_identity = identity(prior_path)
assert prior_identity['sha256'] == prior_summary['pairs']['gzip']['sha256']
with gzip.open(prior_path, 'rt', encoding='utf-8') as data:
    prior_pairs = json.load(data)
record['round22_behavior'] = {
    'raw_archive': prior_identity,
    'head': prior_summary['head'],
    'semantic_sha256': behavior_hash(prior_pairs, 'r21'),
}
record['semantic_sha256'] = behavior_hash(pairs, 'r24')
record['matches_round22_behavior'] = (
    record['semantic_sha256'] == record['round22_behavior']['semantic_sha256']
)
put_json(output / 'pairs.json', pairs)
record['pairs'] = compress(output / 'pairs.json')
put_json(output / 'summary.json', record)
print(json.dumps({'pairs': len(pairs), 'equal': record['exact_full_results_equal'],
                  'budget_failures': record['budget_failures'],
                  'matches_round22_behavior': record['matches_round22_behavior']}), flush=True)
assert record['exact_full_results_equal'] == 837
assert record['budget_failures'] == {'r20': 0, 'r24': 0}
assert record['matches_round22_behavior']
