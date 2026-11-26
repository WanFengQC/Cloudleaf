"""同一份按键前缀、资源和排序配对正常及阶段观测。"""
import argparse
import hashlib
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
LOCAL = ROOT / '.local'
sys.path.insert(0, str(LOCAL))
from r21_evidence_common import get_json, head, identity, percentile, put_json, run


def commands(path):
    for number, line in enumerate(path.read_text(encoding='utf-8').splitlines(), 1):
        if not line or line.startswith('#'):
            continue
        label, sequence = line.split('\t', 1)
        for word, token in enumerate(sequence.split(), 1):
            if not token.startswith('<') and len(token) > 1:
                for offset in range(1, len(token) + 1):
                    yield f'{label}-{number}-{word}-{offset}\t{token[:offset]}\n'


def parse(path):
    rows = {}
    current = None
    for line in path.read_text(encoding='utf-8').splitlines():
        fields = line.split('\t')
        match fields[0]:
            case 'PROFILE':
                current = {'id': fields[1], 'normal_ns': int(fields[2].split('=')[1]), 'phases': {}}
                rows[fields[1]] = current
            case 'PHASE':
                current['phases'][fields[2]] = [int(fields[3]), int(fields[4])]
            case 'RESULT':
                current['result_sha256'] = hashlib.sha256(fields[-1].encode()).hexdigest()
    assert rows and all('result_sha256' in row for row in rows.values())
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--current', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(LOCAL):
        raise ValueError('原始材料只能记录于 .local')
    output.mkdir()
    source = get_json(LOCAL / 'round22-search-fixed/summary.json')
    runtime, lexical = source['runtime'], source['models']['lexical']
    binaries = {'baseline': identity(args.baseline), 'current': identity(args.current)}
    for record in (runtime, lexical):
        assert identity(record['path'])['sha256'] == record['sha256']
    paths = {
        'full': ROOT / 'tests/fixtures/round25-worker-input.tsv',
        'abbreviated': ROOT / 'docs/reports/input-round18-evidence/performance/abbreviated-cases.tsv',
    }
    report = {'head': head(), 'binaries': binaries, 'tool': identity(__file__),
              'runtime': runtime, 'lexical': lexical, 'cases': {}}
    for scheme, fixture in paths.items():
        payload = ''.join(commands(fixture)).encode()
        repeats = {version: [] for version in binaries}
        for turn, order in enumerate([('baseline','current'), ('current','baseline')], 1):
            for variant in order:
                dirname = output / f'{scheme}-{turn}-{variant}'
                cmd = [binaries[variant]['path'], runtime['path'], runtime['sha256'],
                       scheme, 'profile', lexical['path'], lexical['sha256']]
                execution = run(dirname, cmd, payload=payload, deadline_seconds=110, sample=False)
                rows = parse(dirname/'stdout.txt')
                assert len(rows) == payload.count(b'\n')
                repeats[variant].append((execution, rows))
        reference = {key: value['result_sha256'] for key, value in repeats['baseline'][0][1].items()}
        flags = {}
        for variant, recordings in repeats.items():
            gathered = [row for _, run_rows in recordings for row in run_rows.values()]
            mismatches = [key for _, run_rows in recordings for key, value in run_rows.items()
                          if reference.get(key) != value['result_sha256']]
            fields = sorted({field for row in gathered for field in row['phases']})
            flags[variant] = {
                'normal_ns': percentile([row['normal_ns'] for row in gathered]),
                'phases_ns': {field: percentile([row['phases'].get(field, [0])[0] for row in gathered])
                              for field in fields},
                'mismatches': mismatches,
                'runs': [execution for execution, _ in recordings],
            }
        report['cases'][scheme] = {'input': identity(fixture), 'all_outputs': len(reference), 'data': flags}
        put_json(output/'summary.json',report)
        assert all(not values['mismatches'] for values in flags.values())
        print(f'{scheme}: {[(variant, item["normal_ns"]["p95"]) for variant, item in flags.items()]}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
