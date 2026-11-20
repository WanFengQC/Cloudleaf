"""固定输入与模型交替重放搜索，保留全部结果及整进程资源样本。"""
import ctypes
from ctypes import wintypes
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import platform
import re
import subprocess
import time


root = Path(__file__).resolve().parent
repo = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
runtime = Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
output = root / 'paired837'
output.mkdir(exist_ok=False)
models = {
    'lexical': repo / '.local/round15-language/signal.tsv',
    'addone': repo / '.local/round16-language/character.tsv',
    'witten': repo / '.local/round16-language/character-witten.tsv',
}
schemes = {
    name: repo / f'docs/reports/input-round18-evidence/performance/{name}-cases.tsv'
    for name in ('full', 'abbreviated', 'xiaohe', 'fuzzy-nl', 'correction')
}
binaries = {'r19': root / 'baseline.exe', 'r20': root / 'final-search.exe'}


def identity(path):
    raw = path.read_bytes()
    return {'path': str(path), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}


class ProcessMemory(ctypes.Structure):
    _fields_ = [('cb', wintypes.DWORD), ('faults', wintypes.DWORD)] + [
        (name, ctypes.c_size_t) for name in (
            'peak', 'working', 'peak_paged', 'paged', 'peak_nonpaged', 'nonpaged',
            'pagefile', 'peak_pagefile', 'private',
        )
    ]


get_memory = ctypes.WinDLL('psapi', use_last_error=True).GetProcessMemoryInfo
get_memory.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessMemory), wintypes.DWORD]
budget_re = re.compile(r'budget: (None|Some\(\w+\))')


def run(kind, scheme, iteration, version):
    label = f'{iteration}-{kind}-{scheme}-{version}'
    command = [str(binaries[version]), str(runtime), runtime_id['sha256'],
               str(models[kind]), str(schemes[scheme]), scheme]
    stdout = output / (label + '.stdout')
    stderr = output / (label + '.stderr')
    memory = []
    start = time.perf_counter()
    with stdout.open('xb') as out, stderr.open('xb') as err:
        child = subprocess.Popen(command, cwd=root, stdout=out, stderr=err,
                                 creationflags=subprocess.CREATE_NO_WINDOW)
        while child.poll() is None:
            sample = ProcessMemory()
            sample.cb = ctypes.sizeof(sample)
            if get_memory(int(child._handle), ctypes.byref(sample), sample.cb):
                memory.append({'offset_s': time.perf_counter() - start,
                               'working_bytes': sample.working, 'peak_working_bytes': sample.peak,
                               'private_bytes': sample.private})
            time.sleep(.05)
    meta = {'label': label, 'kind': kind, 'scheme': scheme, 'iteration': iteration,
            'version': version, 'command': command, 'returncode': child.returncode,
            'elapsed_s': time.perf_counter() - start, 'stdout': identity(stdout),
            'stderr': identity(stderr), 'memory': memory}
    (output / (label + '.json')).write_text(json.dumps(meta, indent=2), encoding='utf-8')
    if child.returncode:
        raise RuntimeError(meta)
    result = []
    for line in stdout.read_text(encoding='utf-8').splitlines():
        _, case, raw, lattice, search, payload = line.split('\t', 5)
        result.append({
            'case': case, 'raw': raw, 'lattice_ns': int(lattice.split('=')[1]),
            'search_ns': int(search.split('=')[1]), 'budget': budget_re.search(payload)[1],
            'result': payload.split(', statistics: Statistics {')[0],
            'candidate_count': payload.count('Candidate { text:'),
            'graph_edges': int(re.search(r'graph_edges: (\d+)', payload)[1]),
            'beam_pruned': int(re.search(r'beam_pruned: (\d+)', payload)[1]),
            'expansions': int(re.search(r'expansions: (\d+)', payload)[1]),
            'resident': int(re.search(r'resident_high_water: (\d+)', payload)[1]),
        })
    assert len(result) == len(schemes[scheme].read_text(encoding='utf-8').splitlines())
    return result


def percentiles(observations):
    values = sorted(observations)
    if not values:
        return {'n': 0}
    return {'n': len(values), 'p50': values[math.ceil(len(values) * .50) - 1],
            'p95': values[math.ceil(len(values) * .95) - 1],
            'p99': values[math.ceil(len(values) * .99) - 1], 'max': values[-1]}


runtime_id = identity(runtime)
assert runtime_id['sha256'] == '03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
results = []
summary = {
    'baseline': '4d42363091361552ad525f47e3c6b1d581ae3a4c',
    'machine': platform.platform(), 'start_utc': datetime.now(timezone.utc).isoformat(),
    'runtime': runtime_id, 'binaries': {k: identity(p) for k, p in binaries.items()},
    'models': {k: identity(p) for k, p in models.items()},
    'inputs': {k: identity(p) for k, p in schemes.items()},
    'comparison': 'All SearchResult fields except work counters. Complete Candidate objects include text, ID, score, features and edges.',
    'scope': 'Search-only test processes, whole-process memory samples, no desktop automation or TSF',
    'source': identity(root / 'source/crates/cloudleaf-search/src/search.rs'),
    'config': {'max_expansions': 200000, 'max_resident_paths': 50000, 'k': 10, 'rounds': 3},
    'tool': identity(Path(__file__)),
}
for iteration in range(1, 4):
    for kind in models:
        for scheme in schemes:
            versions = ['r19', 'r20'] if (iteration + list(models).index(kind) + list(schemes).index(scheme)) % 2 else ['r20', 'r19']
            batch = {version: run(kind, scheme, iteration, version) for version in versions}
            for before, after in zip(batch['r19'], batch['r20'], strict=True):
                assert (before['case'], before['raw']) == (after['case'], after['raw'])
                results.append({'iteration': iteration, 'kind': kind, 'scheme': scheme,
                                'r19': before, 'r20': after})
            print(iteration, kind, scheme, len(batch['r20']), flush=True)

assert len(results) == 837
summary['totals'] = {version: {'queries': len(results),
    'budget_failures': sum(r[version]['budget'] != 'None' for r in results)} for version in binaries}
summary['budget_rows'] = [r for r in results if any(r[v]['budget'] != 'None' for v in binaries)]
complete_pairs = [r for r in results if all(r[v]['budget'] == 'None' for v in binaries)]
summary['complete_pairs'] = len(complete_pairs)
summary['complete_mismatches'] = [r for r in complete_pairs if any(
    r['r19'][field] != r['r20'][field] for field in ('result', 'graph_edges', 'beam_pruned')
)]
summary['metrics_ns'] = {}
for kind in models:
    for scheme in schemes:
        condition = [r for r in results if (r['kind'], r['scheme']) == (kind, scheme)]
        paired = [r for r in condition if all(r[v]['budget'] == 'None' for v in binaries)]
        for version in binaries:
            summary['metrics_ns'][f'{kind}/{scheme}/{version}'] = {
                'lattice': percentiles([r[version]['lattice_ns'] for r in condition]),
                'search': percentiles([r[version]['search_ns'] for r in condition]),
                'total': percentiles([r[version]['search_ns'] + r[version]['lattice_ns'] for r in condition]),
                'total_both_complete': percentiles([r[version]['search_ns'] + r[version]['lattice_ns'] for r in paired]),
                'budget_failures': sum(r[version]['budget'] != 'None' for r in condition),
                'max_expansions': max(r[version]['expansions'] for r in condition),
                'max_resident_paths': max(r[version]['resident'] for r in condition),
            }
summary['finish_utc'] = datetime.now(timezone.utc).isoformat()
summary['process_memory'] = {}
for version in binaries:
    samples = [sample for path in output.glob(f'*-{version}.json')
               for sample in json.loads(path.read_text(encoding='utf-8'))['memory']]
    summary['process_memory'][version] = {
        'samples': len(samples),
        'maximum_working_bytes': max((s['peak_working_bytes'] for s in samples), default=None),
        'maximum_observed_private_bytes': max((s['private_bytes'] for s in samples), default=None),
    }
(output / 'rows.json').write_text(json.dumps(results, ensure_ascii=False), encoding='utf-8')
(output / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8')
print(summary['totals'], 'complete pairs', len(complete_pairs), 'mismatches', len(summary['complete_mismatches']))
assert not summary['complete_mismatches']
