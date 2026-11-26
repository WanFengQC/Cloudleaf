"""同资源交替配对精确字符搜索，原始文本在 .local。"""
import argparse
import json
from pathlib import Path

from round24_recall_comparison import LOCAL, get_json, identity, percentile, put_json, read_rows, run

BASELINE = '403ace1c9d95993f6117113f94924b58aee998bb'
EXPECTED_BASELINE_PROBE = 'c057f3da7e4aedd602c04cedfca77b35b3e70fcec2bcc8162fdc97ec498c18a6'


def fixture(provenance, output, mode):
    source = provenance['fixture']
    path = Path(source['path'])
    assert identity(path)['sha256'] == source['sha256']
    if mode == 'ordinary':
        return path
    destination = output / 'interpreted.tsv'
    with destination.open('x', encoding='utf-8', newline='\n') as stream:
        for line in path.read_text(encoding='utf-8').splitlines():
            label, scheme, raw, expected = line.split('\t')
            assert scheme == 'full'
            stream.write(f'{label}\tabbreviated\t{raw}\t{expected}\n')
    return destination


def semantic(rows):
    return {
        case: {'reach': row.get('reach'), 'error': row['error'],
               'exact': {k: v for k, v in row['runs'].get('exact-pool', {}).items() if k != 'ns'}}
        for case, row in rows.items()
    }


def profiling(live, probe):
    samples = [row for batch in live for row in batch['memory']]
    sizes = {k: [row[k] for row in samples] for k in ('peak_working_bytes', 'private_bytes')}
    return {k: max(values, default=None) for k, values in sizes.items()} | {'probe': probe}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--implementation', required=True)
    parser.add_argument('--only-interpreted', action='store_true')
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(LOCAL):
        raise ValueError('配对研发样本只能写到 .local')
    output.mkdir(parents=True, exist_ok=False)
    old = LOCAL / 'round24-profiler/round24-allocation-probe.exe'
    binaries = {'old': identity(old), 'new': identity(args.probe)}
    assert binaries['old']['sha256'] == EXPECTED_BASELINE_PROBE
    summary = {'baseline': BASELINE, 'implementation': args.implementation, 'tool': identity(__file__),
               'probes': binaries, 'groups': {}}
    cases_to_run = [('base','ordinary'), ('supplement','ordinary'), ('base','interpreted')]
    if args.only_interpreted:
        cases_to_run = [('base', 'interpreted')]
    for dictionary, mode in cases_to_run:
        folder = output / f'{dictionary}-{mode}'
        folder.mkdir()
        resource = get_json(LOCAL / f'round24-frozen-{dictionary}-minimum/summary.json')['resources']
        cases = fixture(resource, folder, mode)
        files = [resource['runtime'], resource['word'],
                 *(resource['models'][name] for name in ('lexical','addone','witten'))]
        for item in files:
            assert identity(Path(item['path']))['sha256'] == item['sha256']
        same = None
        measurements = {label: [] for label in binaries}
        executions = {label: [] for label in binaries}
        failures = []
        for trial, ordering in enumerate([('old','new'), ('new','old')], 1):
            for variant in ordering:
                base = folder / f'{trial}-{variant}'
                command = [binaries[variant]['path']]
                for item in files:
                    command += [item['path'], item['sha256']]
                command.extend([cases, '--recall-round24'])
                result = run(base, command, deadline_seconds=180)
                executions[variant].append(result)
                _, rows = read_rows(base / 'stdout.txt')
                assert len(rows) == 40
                if same is None:
                    same = semantic(rows)
                comparison = semantic(rows)
                failures += [{'case': current, 'trial': trial, 'variant': variant}
                             for current in comparison if comparison[current] != same[current]]
                for row in rows.values():
                    query = row['runs'].get('exact-pool')
                    if query:
                        measurements[variant].append((query, row['alloc']['exact-pool']))
        pair = {}
        for variant, samples in measurements.items():
            pair[variant] = {
                'search_ns': percentile([r['ns'] for r, _ in samples]),
                'allocated_bytes': percentile([a['bytes'] for _, a in samples]),
                'alloc_calls': percentile([a['calls'] for _, a in samples]),
                'peak_paths': max(r['resident'] for r, _ in samples),
                'peak_expansions': max(r['expansions'] for r, _ in samples),
                'budget_failures': sum(r['budget'] != 'None' for r, _ in samples),
                'resources': profiling(executions[variant], binaries[variant]),
            }
        summary['groups'][f'{dictionary}-{mode}'] = {
            'resources': files, 'fixture': identity(cases), 'pair': pair,
            'semantic_differences': failures,
            'raw_runs': {version: [r['stdout'] for r in data] for version, data in executions.items()},
        }
        put_json(output / 'summary.json', summary)
        assert not failures, failures
    print(json.dumps({g: {v: (r['search_ns']['p95'],r['allocated_bytes']['p95'])
                          for v,r in rows['pair'].items()} for g,rows in summary['groups'].items()}))


if __name__ == '__main__':
    main()
