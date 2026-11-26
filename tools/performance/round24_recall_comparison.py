"""固定 Round24 配对召回实验。涉及来源正文的逐例输出写到 Git 外。"""
import argparse
import json
import os
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LOCAL = ROOT / '.local'
sys.path.insert(0, str(LOCAL))
from r21_evidence_common import get_json, head, identity, percentile, put_json, run
from r23_eval import cases

MODES = ('exact-pool', 'finite-word', 'interleaved', 'context-beam')


def read_rows(path):
    rows = {}
    bindings = None
    for original in path.read_text(encoding='utf-8').splitlines():
        kind, *values = original.split('\t')
        if kind == 'SOURCE':
            bindings = values
            continue
        label, *fields = values
        if kind == 'CASE':
            scheme, raw, expected = fields
            rows[label] = {'scheme': scheme, 'raw': raw, 'expected': expected, 'runs': {},
                           'alloc': {}, 'error': {}}
            continue
        row = rows[label]
        if kind == 'REACH':
            row['reach'] = fields[0]
        elif kind == 'LATTICE':
            row['lattice_ns'] = int(fields[0])
        elif kind == 'ALLOC':
            method, nbytes, calls = fields
            row['alloc'][method] = {'bytes': int(nbytes), 'calls': int(calls)}
        elif kind == 'RUN':
            method, ns, budget, exhaustive, edges, expansions, resident, size = fields
            row['runs'][method] = {'ns': int(ns), 'budget': budget, 'exhaustive': exhaustive == 'true',
                                   'edges': int(edges), 'expansions': int(expansions),
                                   'resident': int(resident), 'size': int(size), 'candidates': []}
        elif kind == 'WORK':
            method, pruned, source_work = fields
            row['runs'][method].update(pruned=int(pruned), source_work=source_work)
        elif kind == 'CAND':
            method, rank, text, score, features, entries, edges = fields
            store = row['runs'][method]['candidates']
            assert int(rank) == len(store) + 1
            store.append({'text': text, 'score': int(score), 'features': features,
                          'entries': entries, 'edges': edges})
        elif kind == 'ERROR':
            phase, reason = fields
            row['error'][phase] = reason
        else:
            raise ValueError(f'未知记录类型 {kind}')
    assert bindings
    for row in rows.values():
        for measurement in row['runs'].values():
            assert measurement['expansions'] <= 200_000 and measurement['resident'] <= 50_000
            assert len(measurement['candidates']) == measurement['size'] <= 50
            assert len({p['text'] for p in measurement['candidates']}) == measurement['size']
    return bindings, rows


def rank(result, expected):
    return next((i for i, path in enumerate(result.get('candidates', []), 1)
                 if path['text'] == expected), None)


def grouped(rows, frozen):
    result = defaultdict(dict)
    if frozen:
        result['new_frozen_v2'] = rows
    else:
        for key, row in rows.items():
            family = ('round23' if key.startswith('r23frozen-') else
                      'round' + key[1:3])
            result[family][key] = row
    return result


def evaluate(rows):
    expected = {k: r for k, r in rows.items() if r['expected']}
    report = {'cases': len(rows), 'expected': len(expected),
              'reach': dict(Counter(r.get('reach', 'unknown') for r in rows.values())),
              'lattice_ns': percentile([r['lattice_ns'] for r in rows.values() if 'lattice_ns' in r]),
              'lattice_alloc_bytes': percentile([r['alloc']['lattice']['bytes'] for r in rows.values()
                                                 if 'lattice' in r['alloc']]), 'modes': {}}
    for mode in MODES:
        requested = {k: r['runs'].get(mode, {}) for k, r in rows.items()}
        runs = [r for r in requested.values() if r]
        ranks = {k: rank(requested[k], r['expected']) for k, r in expected.items()}
        strict = {k: value if requested[k].get('budget') == 'None' else None
                  for k, value in ranks.items()}
        old_ranks = {k: rank(r['runs'].get('exact-pool', {}), r['expected']) for k, r in expected.items()}
        gains = [k for k, v in ranks.items() if (100 if v is None else v) < (old_ranks[k] or 100)]
        regressions = [k for k, v in ranks.items() if (100 if v is None else v) > (old_ranks[k] or 100)]
        report['modes'][mode] = {
            'supported': len(runs), 'no_candidates': sum(not r['size'] for r in runs),
            'budgets': dict(Counter(r['budget'] for r in runs)),
            'no_global_guarantee': sum(not r['exhaustive'] for r in runs),
            'budget_failures': sum(r['budget'] != 'None' for r in runs),
            'source_match': {str(k): sum(v is not None and v <= k for v in ranks.values())
                             for k in (1, 3, 5, 10, 50)} if expected else None,
            'without_exhausted_matches': {str(k): sum(v is not None and v <= k for v in strict.values())
                                          for k in (1, 3, 5, 10, 50)} if expected else None,
            'source_mrr_at_50': sum(1/v for v in ranks.values() if v) / len(expected) if expected else None,
            'changed_rank_improved_ids': gains, 'changed_rank_regressed_ids': regressions,
            'gained_candidates': sum(v is not None and old_ranks[k] is None for k, v in ranks.items()),
            'lost_candidates': sum(v is None and old_ranks[k] is not None for k, v in ranks.items()),
            'ns': percentile([r['ns'] for r in runs]),
            'expansions': percentile([r['expansions'] for r in runs]),
            'resident': percentile([r['resident'] for r in runs]),
            'pruned': percentile([r['pruned'] for r in runs if 'pruned' in r]),
            'allocated_bytes': percentile([r['alloc'][mode]['bytes'] for r in rows.values()
                                           if mode in r['alloc']]),
            'allocations': percentile([r['alloc'][mode]['calls'] for r in rows.values()
                                       if mode in r['alloc']]),
        }
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('word', type=Path)
    parser.add_argument('out', type=Path)
    parser.add_argument('--supplement', action='store_true')
    parser.add_argument('--frozen', action='store_true')
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    fixed = get_json(LOCAL / 'round22-search-fixed/summary.json')
    if args.supplement:
        prior = ROOT.parents[1] / 'input-round14/Cloudleaf/.local/round17-supplement-a'
        fixed['runtime'] = identity(prior / 'combined-runtime.tsv')
        assert fixed['runtime']['sha256'] == 'c9ab8b56b9d960091a1c39c37b7c1df7882ff50de2a64767174af8f4244c219f'
        fixed['models'] = {name: identity(prior / f'model-{i}.tsv')
                           for name, i in [('lexical', 2), ('addone', 0), ('witten', 1)]}
    if args.frozen:
        folder = LOCAL / 'round24-corpus/frozen-v2'
        pack = get_json(folder / 'receipt.json')
        ready = folder / 'rank-replay-cases.tsv'
        assert identity(ready)['sha256'] == pack['rank_replay_cases_sha256']
        sources = {'frozen_v2': identity(folder / 'receipt.json'), 'source_cases': identity(ready)}
        tags = [line for line in ready.read_text(encoding='utf-8').splitlines()
                if line and not line.startswith('#') and line != 'id\tscheme\traw\texpected']
        assert len(tags) == 40
        fixture = args.out / 'cases.tsv'
        fixture.write_text('\n'.join(tags) + '\n', encoding='utf-8', newline='\n')
    else:
        old = LOCAL / 'round23-word-context' / ('frozen-supplement' if args.supplement else 'frozen-base')
        fixture, sources = cases(args.out, old / 'rank-replay-cases.tsv', False)
    binary = LOCAL / ('target/release/examples/research_rank.exe' if os.name == 'nt' else
                      'target/release/examples/research_rank')
    word = identity(args.word)
    command = [binary, fixed['runtime']['path'], fixed['runtime']['sha256'], word['path'], word['sha256']]
    for role in ('lexical', 'addone', 'witten'):
        command.extend((fixed['models'][role]['path'], fixed['models'][role]['sha256']))
    command.extend((fixture, '--recall-round24'))
    execution = run(args.out / 'query', command, deadline_seconds=480)
    assert execution['returncode'] == 0, execution
    bindings, rows = read_rows(args.out / 'query/stdout.txt')
    assert len(rows) == len(fixture.read_text(encoding='utf-8').splitlines())
    by_group = {key: evaluate(group) for key, group in grouped(rows, args.frozen).items()}
    put_json(args.out / 'summary.json', {
        'implementation': head(), 'tool': identity(Path(__file__)), 'binary': identity(binary),
        'command': list(map(str, command)), 'code': execution['returncode'], 'execution': execution,
        'resources': {'word': word, 'runtime': fixed['runtime'], 'models': fixed['models'],
                      'fixture': identity(fixture), 'sources': sources, 'decoded': bindings},
        'limits': {'max_expansions': 200_000, 'max_resident_paths': 50_000, 'pool': 50, 'beam_width': 64,
                   'max_input': 64, 'exact_source_max_length': 64},
        'formal_metrics': {'top1': None, 'top5': None, 'mrr': None}, 'groups': by_group,
    })
    print({key: item['cases'] for key, item in by_group.items()})


if __name__ == '__main__':
    main()
