"""对原七条驻留终止扩大离线额度，保持展开、词典、模型和 Top-K。"""
from collections import Counter
import argparse
import hashlib
import json
from pathlib import Path

from round24_recall_comparison import LOCAL, get_json, head, identity, put_json, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(LOCAL):
        raise ValueError('逐例材料只写到 .local')
    output.mkdir(parents=True, exist_ok=False)
    old = LOCAL / 'round24-recall-diagnosis'
    resource = get_json(old / 'diagnosis.json')
    results = {'head': head(), 'tool': identity(__file__), 'binary': identity(args.binary),
               'historical_diagnosis': identity(old / 'diagnosis.json'), 'cases': {}}
    total = Counter()
    for version in ['base', 'supplement']:
        folder = output / version
        folder.mkdir()
        selected = [row for row in resource['cases'][version]
                    if row['pools']['character']['budget'] == 'Some(ResidentPaths)'
                    and row['pools']['character']['target_rank'] is None]
        cases = {case['case']: case for case in selected}
        total['historical_cases'] += len(cases)
        raw = (old / version / 'cases.tsv').read_text(encoding='utf-8')
        rows = [line for line in raw.splitlines() if line.split('\t', 1)[0] in cases]
        assert len(rows) == len(cases)
        path = folder / 'cases.tsv'
        path.write_text('\n'.join(rows)+'\n', encoding='utf-8', newline='\n')
        items = resource['sources'][version]
        original = get_json(old / version / 'command.json')
        argv = original['argv']
        files = []
        for position, kind in enumerate(['runtime','word_model','witten_model']):
            index = 2 * position + 1
            file = identity(argv[index])
            assert file['sha256'] == argv[index + 1] == items[kind]['sha256']
            files += [file]
        summary = {'fixture': identity(path), 'resources': files, 'runs': {}}
        for cap in [50_000, 75_000, 100_000, 200_000]:
            command = [args.binary, *argv[1:-1], path, f'--diagnostic-resident={cap}']
            step = folder / str(cap)
            execution = run(step, command, deadline_seconds=120, sample=True)
            checked = []
            content = (step / 'stdout.txt').read_text(encoding='utf-8')
            for line in content.splitlines():
                columns = line.split('\t')
                if columns[0] != 'POOL' or columns[2] != 'character':
                    continue
                _, label, _, size, budget, complete, expansions, resident, pruned, rank = columns
                historical = cases[label]['pools']['character']
                if cap == 50_000:
                    assert budget == historical['budget']
                    assert int(resident) == historical['resident_peak']
                    assert int(expansions) == historical['expansions']
                checked.append({'id': label, 'candidate_count': int(size), 'budget': budget,
                                'complete': complete, 'expansions': int(expansions),
                                'resident': int(resident), 'pruned': int(pruned), 'rank': rank,
                                'expected_sha256': cases[label]['target_sha256']})
                total[f'{cap}_complete'] += complete == 'true'
                total[f'{cap}_recoveries'] += rank != 'None'
            assert len(checked) == len(cases)
            summary['runs'][str(cap)] = {'cases': checked, 'execution': execution}
        results['cases'][version] = summary
    assert total['historical_cases'] == 7
    results['totals'] = dict(total)
    put_json(output / 'summary.json', results)
    print(json.dumps(results['totals']))


if __name__ == '__main__':
    main()
