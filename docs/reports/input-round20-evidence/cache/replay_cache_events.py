"""同一二进制比较缓存启停，仅移除逐键计时和独立缓存计数。"""
import argparse
import hashlib
import json
import math
import platform
import re
import subprocess
from pathlib import Path

repo = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
runtime = Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
models = {
    'lexical': repo / '.local/round15-language/signal.tsv',
    'addone': repo / '.local/round16-language/character.tsv',
    'witten': repo / '.local/round16-language/character-witten.tsv',
}
schemes = ('full', 'abbreviated', 'xiaohe', 'fuzzy-nl', 'correction')

def identity(path):
    return {'path': str(path), 'bytes': path.stat().st_size,
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}

def quantiles(samples):
    samples = sorted(samples)
    return {'n': len(samples), 'p50': samples[math.ceil(len(samples) * .5) - 1],
            'p95': samples[math.ceil(len(samples) * .95) - 1],
            'p99': samples[math.ceil(len(samples) * .99) - 1], 'max': samples[-1]}

def write(path, row):
    with path.open('x', encoding='utf-8', newline='\n') as stream:
        json.dump(row, stream, ensure_ascii=False, indent=2)
        stream.write('\n')

def run(out, exe, source, payload, model, scheme, mode):
    folder = out / f'{model.stem}-{scheme}-{mode}'
    folder.mkdir()
    argv = [str(exe), str(runtime), source['sha256'], str(model), identity(model)['sha256'], scheme, mode]
    completed = subprocess.run(argv, input=payload, capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
    (folder / 'stdout.txt').write_bytes(completed.stdout)
    (folder / 'stderr.txt').write_bytes(completed.stderr)
    semantic, times, audit = [], [], []
    for line in completed.stdout.decode('utf-8').splitlines():
        if line.startswith('EVENT\t'):
            _, case, event, nanos, body = line.split('\t', 4)
            semantic.append((case, event, body))
            times.append(int(nanos))
        elif line.startswith('CACHE\t'):
            audit.append({k: int(v) for k, v in re.findall(r'(\w+): (\d+)', line)})
    row = {'argv': argv, 'returncode': completed.returncode,
           'stdout': identity(folder / 'stdout.txt'), 'stderr': identity(folder / 'stderr.txt'),
           'events': len(times), 'latency_ns': quantiles(times) if times else None,
           'cache': audit[-1] if audit else None,
           'max_resident_entries': max((entry['resident_entries'] for entry in audit), default=0)}
    write(folder / 'result.json', row)
    assert completed.returncode == 0 and len(times) == len(audit) == 171, row
    return semantic, row

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('exe', type=Path)
    parser.add_argument('out', type=Path)
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir()
    exe = args.exe.resolve()
    fixture = repo / 'tests/fixtures/input-round14-events.tsv'
    payload = fixture.read_bytes()
    (out / 'events.tsv').write_bytes(payload)
    source = identity(runtime)
    assert source['sha256'] == '03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
    pairs = []
    for kind, model in models.items():
        for index, scheme in enumerate(schemes):
            variants = {}
            for mode in (('off', 'on') if index % 2 == 0 else ('on', 'off')):
                variants[mode] = run(out, exe, source, payload, model, scheme, mode)
            off, a = variants['off']
            on, b = variants['on']
            row = {'model': kind, 'scheme': scheme, 'complete_event_equality': off == on, 'off': a, 'on': b}
            pairs.append(row)
            print(json.dumps({'model': kind, 'scheme': scheme, 'equal': off == on,
                              'p95_ns': [a['latency_ns']['p95'], b['latency_ns']['p95']]}), flush=True)
    report = {'platform': platform.platform(), 'binary': identity(exe), 'tool': identity(Path(__file__)),
              'runtime': source, 'fixture': identity(fixture), 'models': {k: identity(v) for k, v in models.items()},
              'pairs': pairs, 'events_compared': 171 * len(pairs), 'observations': 342 * len(pairs),
              'all_equal': all(row['complete_event_equality'] for row in pairs),
              'scope': 'InputEngine explicit synthetic trace, timings exclude stdout. No desktop, TSF or real commit learning.'}
    write(out / 'summary.json', report)
    assert report['all_equal']

if __name__ == '__main__':
    main()
