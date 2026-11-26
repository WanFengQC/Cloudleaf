"""交替测量固定输入语料在词汇基线与词语实验模式中的时延和线程周期。"""
import argparse
import json
import tempfile
import time
from collections import defaultdict
from pathlib import Path

import round22_worker_replay as previous

ROOT = previous.ROOT
LOCAL = ROOT / '.local'

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('word', type=Path)
    parser.add_argument('out', type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    release = LOCAL / 'target/release'
    fixed = json.loads((LOCAL / 'round22-search-fixed/summary.json').read_text(encoding='utf-8'))
    catalog = json.loads((LOCAL / 'round21-model-identity/summary.json').read_text(encoding='utf-8'))['models']['catalog']
    exe = previous.build(release, args.out)
    word = previous.identity(args.word.resolve())
    windows, psapi = previous.win_api()
    data = {'word': word, 'worker': previous.identity(release/'cloudleaf-translation-worker.exe'),
            'tool': previous.identity(Path(__file__)), 'probe': previous.identity(exe),
            'runtime': fixed['runtime'], 'lexical': fixed['models']['lexical'], 'catalog': catalog,
            'events': previous.identity(previous.TRACE), 'runs': [], 'errors': []}
    old_results = {}
    wall = defaultdict(list)
    cycles = defaultdict(list)
    specs = [('lexical','idle'),('word','idle'),('lexical','infer'),('word','infer')]
    for repeat in range(3):
        for mode, load in (specs if repeat % 2 == 0 else list(reversed(specs))):
            labels = (mode, load)
            name = f'{repeat + 1}-{mode}-{load}'
            try:
                with tempfile.TemporaryDirectory(prefix='cloudleaf-r23-synthetic-') as directory:
                    argv = [exe, fixed['runtime']['path'], fixed['runtime']['sha256'],
                            fixed['models']['lexical']['path'], fixed['models']['lexical']['sha256'],
                            catalog['path'], catalog['sha256'], previous.TRACE, Path(directory)/'store', load, 1]
                    if mode == 'word':
                        argv.extend((word['path'], word['sha256']))
                    lines, run = previous.execute(args.out/name, list(map(str,argv)), windows, psapi, 'normal')
                keys, semantics, worker, *_ = previous.parse(lines)
                if mode in old_results:
                    assert old_results[mode] == semantics
                old_results[mode] = semantics
                cycle_samples = {int(i):int(n) for kind,i,n in (row.split('\t') for row in lines if row.startswith('CYCLE\t'))}
                input_model = [line.split('\t')[1:] for line in lines if line.startswith('INPUT_MODEL\t')]
                assert len(cycle_samples)==len(keys)==171 and len(input_model)==1
                if load == 'infer':
                    trait = worker['main']
                    keys = [row for row in keys if trait['infer_start'] <= row['start'] <= row['end'] <= trait['infer_end']]
                    assert keys, 'inference must actually overlap the key metric'
                wall[labels].extend(row['wall_ns'] for row in keys)
                cycles[labels].extend(cycle_samples[row['index']] for row in keys)
                data['runs'].append({'case':name, 'io_resources':previous.flatten(run), 'input_model':input_model,
                                     'worker_metrics':worker, 'overlap_or_idle':len(keys)})
                print(name, len(keys), flush=True)
            except (OSError, ValueError, AssertionError, RuntimeError) as error:
                data['errors'].append({'case':name,'error':str(error)})
            previous.write_json(args.out/'summary.json', data)
    data['wall_ns'] = {'/'.join(k):previous.percentiles(v) for k,v in wall.items()}
    data['thread_cycles'] = {'/'.join(k):previous.percentiles(v) for k,v in cycles.items()}
    data['semantics_stable_within_each_ranker'] = True
    previous.write_json(args.out/'summary.json', data)
    raise SystemExit(bool(data['errors']))

if __name__ == '__main__':
    main()
