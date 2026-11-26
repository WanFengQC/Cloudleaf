"""固定本轮研发输入和实际 release 排序输出。原文保留在 Git 外。"""
import argparse
import gzip
import json
import platform
from collections import Counter, defaultdict
from pathlib import Path

from r21_evidence_common import ROOT, LOCAL, git, get_json, head, identity, now, percentile, put_json, run

def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument('word', type=Path)
    parser.add_argument('--frozen', type=Path)
    parser.add_argument('--label', default='round23-language')
    parser.add_argument('--quick', action='store_true')
    parser.add_argument('--character-pool', action='store_true')
    parser.add_argument('--supplement', action='store_true')
    return parser.parse_args()

def cases(out, frozen, quick):
    data = []
    inputs = {}
    r16 = ROOT / 'docs/reports/input-round16-evidence/corpus/word-contrast/development-tasks.jsonl'
    rows = [json.loads(line) for line in r16.read_text(encoding='utf-8').splitlines()]
    for row in rows:
        raw = row.get('pinyin', '')
        data.append((row['case'], 'full' if raw else 'unmappable', raw, row['desired_text']))
    inputs['round16_tatoeba'] = identity(r16)
    r17 = ROOT / 'docs/reports/input-round17-evidence/corpus/audit/dev-targets.tsv'
    rows = [line.split('\t') for line in r17.read_text(encoding='utf-8').splitlines()[1:]]
    for name, pinyin, source in rows:
        data.append((name, 'full' if pinyin else 'unmappable', pinyin, source))
    inputs['round17_news'] = identity(r17)
    for scheme in ['full', 'abbreviated', 'xiaohe', 'fuzzy-nl', 'correction']:
        path = ROOT / f'docs/reports/input-round18-evidence/performance/{scheme}-cases.tsv'
        for index, line in enumerate(path.read_text(encoding='utf-8').splitlines(), 1):
            label, raw = line.split('\t')
            data.append((f'r18-{scheme}-{index}', scheme, raw, ''))
        inputs[f'round18_{scheme}'] = identity(path)
    path = LOCAL / 'round22-search-fixed/pairs.json.gz'
    with gzip.open(path, 'rt', encoding='utf-8') as source:
        fixed = json.load(source)
    keys = sorted({(row['scheme'], row['r21']['raw']) for row in fixed})
    for index, (scheme, raw) in enumerate(keys, 1):
        data.append((f'r20-{index}', scheme, raw, ''))
    inputs['round20_fixed_query_projection'] = identity(path) | {'groups': len(keys), 'original_pairs': len(fixed)}
    if frozen is not None:
        for line in frozen.read_text(encoding='utf-8').splitlines():
            if line and not line.startswith('#') and line != 'id\tscheme\traw\texpected':
                row = line.split('\t')
                assert len(row) == 4, 'frozen TSV shape'
                label, scheme, raw, expected = row
                data.append(('r23frozen-' + label, scheme, raw, expected))
        tasks = frozen.with_name('tasks.jsonl')
        known_raw = {row[0] for row in data}
        if tasks.is_file():
            for line in tasks.read_text(encoding='utf-8').splitlines():
                task = json.loads(line)
                name = 'r23frozen-' + task['case']
                if name not in known_raw:
                    assert not task.get('derived_pinyin'), '只允许纳入原冻结包声明不可映射的缺失行'
                    data.append((name, 'unmappable', '', task['source_text']))
            inputs['round23_all_slots'] = identity(tasks)
        inputs['round23_frozen'] = identity(frozen)
    if quick:
        data = [row for row in data if row[0] in {'r16dev003','r16dev004','r16dev009','r17news001','r17news002'}]
    assert len({row[0] for row in data}) == len(data)
    path = out / 'cases.tsv'
    path.write_text(''.join('\t'.join(row) + '\n' for row in data), encoding='utf-8', newline='\n')
    return path, inputs

def summarize(path):
    sources = []
    rows = {}
    for line in path.read_text(encoding='utf-8').splitlines():
        key, *values = line.split('\t')
        if key == 'SOURCE':
            sources.append(values)
        elif key == 'CASE':
            name, scheme, raw, expected = values
            rows[name] = {'kind': scheme, 'input': raw, 'expected': expected, 'runs': {}, 'errors': {}}
        elif key == 'REACH':
            rows[values[0]]['reachability'] = values[1]
        elif key == 'LATTICE':
            rows[values[0]]['lattice_ns'] = int(values[1])
        elif key == 'POOL':
            name, size, hit, budget, complete = values
            rows[name]['pool'] = {'size': int(size), 'hit': hit, 'budget': budget, 'complete': complete == 'true'}
        elif key == 'RUN':
            name, mode, ns, budget, complete, edges, expansions, resident, pool = values
            rows[name]['runs'][mode] = {'ns': int(ns), 'budget': budget, 'complete': complete == 'true',
                                       'counts': list(map(int, (edges, expansions, resident, pool))), 'texts': []}
        elif key == 'CAND':
            name, mode, rank, candidate, score, breakdown, entry_ids, edges = values
            outcome = rows[name]['runs'][mode]
            outcome['texts'].append(candidate)
            if 1 <= int(rank) <= 10 and candidate == rows[name]['expected']:
                outcome['rank'] = int(rank)
        elif key == 'ERROR':
            name, mode, error = values
            rows[name]['errors'][mode] = error
    group_ids = defaultdict(list)
    for name in rows:
        family = next((label for prefix, label in [('r16','round16'),('r17','round17'),('r18','round18'),
                                                    ('r20','round20'),('r23frozen','round23_frozen')] if name.startswith(prefix)), 'other')
        group_ids[family].append(name)
    groups = {}
    mods = ('structural', 'lexical', 'character-addone', 'character-witten', 'unigram', 'word', 'finite-character', 'mixed')
    for kind, keys in group_ids.items():
        group = [rows[k] for k in keys]
        expected = [r for r in group if r['expected']]
        modes = {}
        for mode in mods:
            ran = [row['runs'][mode] for row in group if mode in row['runs']]
            ranks = [row['runs'].get(mode, {}).get('rank', 0) if row['runs'].get(mode, {}).get('complete') else 0 for row in expected]
            modes[mode] = {
                'all_cases': len(group), 'with_expected': len(expected), 'executed': len(ran),
                'not_run_or_error': len(group) - len(ran),
                'zero_candidates': sum(not row['texts'] for row in ran),
                'incomplete': sum(not row['complete'] for row in ran),
                'budget': dict(Counter(row['budget'] for row in ran)),
                'source_match_counts': {str(k): sum(0 < rank <= k for rank in ranks) for k in (1, 3, 5, 10)} if expected else None,
                'reciprocal_rank': sum((1 / rank if rank else 0) for rank in ranks) / len(ranks) if ranks else None,
                'query_ns': percentile([row['ns'] for row in ran]),
                'max_resident_paths': max((r['counts'][2] for r in ran), default=None),
            }
        diffs = {}
        for mode in ('word','mixed'):
            bucket = defaultdict(list)
            for key in keys:
                row = rows[key]
                if not row['expected']:
                    continue
                old = row['runs'].get('character-witten', {}).get('rank', 0)
                new = row['runs'].get(mode, {}).get('rank', 0)
                old, new = old or 99, new or 99
                if old > new:
                    change = 'improved'
                elif old < new:
                    change = 'regressed'
                elif new == 99:
                    change = 'missing'
                elif new > 1:
                    change = 'lower'
                else:
                    change = 'equal_first'
                bucket[change].append({'id': key, 'previous_rank': None if old == 99 else old,
                                       'current_rank': None if new == 99 else new})
            diffs[mode] = dict(bucket)
        groups[kind] = {'cases': len(group), 'expected_count': len(expected),
                        'reachability': dict(Counter(row.get('reachability', 'unknown') for row in group)),
                        'pool_recall': sum(row.get('pool',{}).get('hit') == 'yes' for row in expected),
                        'pool_complete': sum(row.get('pool',{}).get('complete',False) for row in group),
                        'modes': modes, 'changes_from_char_witten': diffs}
    return sources, groups

def main():
    args = parse_args()
    output = LOCAL / args.label
    output.mkdir(exist_ok=False)
    fixture, inputs = cases(output, args.frozen, args.quick)
    config = get_json(LOCAL / 'round22-search-fixed/summary.json')
    if args.supplement:
        supplement = ROOT.parents[1] / 'input-round14/Cloudleaf/.local/round17-supplement-a'
        config['runtime'] = identity(supplement / 'combined-runtime.tsv')
        config['models'] = {name:identity(supplement / f'model-{index}.tsv')
                            for name,index in [('lexical',2), ('addone',0), ('witten',1)]}
        assert config['runtime']['sha256'] == 'c9ab8b56b9d960091a1c39c37b7c1df7882ff50de2a64767174af8f4244c219f'
    model = identity(args.word.resolve())
    exe = LOCAL / 'target/release/examples/research_rank.exe'
    parameters = [exe, config['runtime']['path'], config['runtime']['sha256'], args.word.resolve(), model['sha256']]
    for name in ('lexical','addone','witten'):
        parameters.extend((config['models'][name]['path'], config['models'][name]['sha256']))
    parameters.append(fixture)
    if args.character_pool:
        parameters.append('--character-pool')
    record = {'head': head(), 'environment': platform.platform(), 'started': now(), 'evaluator': identity(exe),
              'tool': identity(Path(__file__)), 'word': model, 'frozen_fixture': identity(fixture), 'inputs': inputs,
              'pool_seed': 'character-witten' if args.character_pool else 'structural', 'formal_metrics': {'top1': None,'top5': None,'mrr': None}}
    put_json(output / 'summary.json', record)
    tested = run(output / 'rank', parameters, deadline_seconds=420)
    sources, groups = summarize(output / 'rank/stdout.txt')
    record.update(sources=sources, groups=groups, execution=tested, finished=now())
    put_json(output / 'summary.json', record)
    print(json.dumps({key: row['cases'] for key, row in groups.items()}))

if __name__ == '__main__':
    main()

