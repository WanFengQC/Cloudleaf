"""按 Round 20 已验证的二进制与原始事件重放真实引擎和无窗口宿主。"""

import argparse
import hashlib
import zipfile
from pathlib import Path

from r21_evidence_common import BASE, LOCAL, ROOT, R20, get_json, head, identity, now, percentile, put_json, run

RUNTIME = ROOT.parents[1] / 'cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv'
MODELS = {
    'lexical': BASE / '.local/round15-language/signal.tsv',
    'addone': BASE / '.local/round16-language/character.tsv',
    'witten': BASE / '.local/round16-language/character-witten.tsv',
}
EVENTS = ROOT / 'tests/fixtures/input-round14-events.tsv'
KNOWN = {
    'runtime': '03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431',
    'lexical': '36dc4d9057bc70e3494fe397da09e8ab6f187c435bd735b66a7734f9d414fce5',
    'addone': '5f28834165fcf83b22458e701b3608b403cb9bf7a9be8ba3c686fd158c53d6b9',
    'witten': '516d78a23786470c5ba7c1c7ee45e82c80eaaccd7eb1e3b541faf55d8f93b44b',
}


def baseline_executables(out):
    folder = BASE / '.local/round20-ci/final-downloads'
    manifest = get_json(folder / 'downloads.json')
    assert manifest['head'] == R20
    expected = next(f for f in manifest['files'] if f['file'] == 'input-round20-windows.zip')
    archive = folder / expected['file']
    assert identity(archive)['sha256'] == expected['sha256']
    binaries = {}
    with zipfile.ZipFile(archive) as package:
        assert package.testzip() is None
        for exe in ('input_trace.exe', 'cloudleaf-ime.exe'):
            member = next(m for m in expected['members'] if Path(m['name']).name == exe)
            data = package.read(member['name'])
            assert len(data) == member['bytes'] and hashlib.sha256(data).hexdigest() == member['sha256']
            file = out / f'r20-{exe}'
            file.write_bytes(data)
            binaries[exe] = file
    return binaries, identity(archive)


def semantic(folder, scope):
    lines = (folder / 'stdout.txt').read_text(encoding='utf-8').splitlines()
    timings, stable = [], []
    for line in lines:
        fields = line.split('\t')
        if scope == 'engine' and fields[0] == 'EVENT':
            stable.append('\t'.join(fields[:3] + fields[4:]))
            timings.append(int(fields[3]))
        elif scope == 'host' and fields[0] == 'HOST_BENCH':
            stable.append('\t'.join(fields[:4] + fields[10:]))
            timings.append(int(fields[9].split('=', 1)[1]))
        elif line.startswith(('FRAME\t', 'CANDIDATE\t', 'COMMIT\t', 'SEARCH\t', 'PATH\t', 'SUMMARY\t',
                              'HOST_FRAME\t', 'HOST_SEARCH\t', 'HOST_SUMMARY ')):
            stable.append(line)
    assert len(timings) == 171
    return timings, stable


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('release', type=Path)
    parser.add_argument('--label', default='round21-input-traces')
    parser.add_argument('--rounds', type=int, default=3)
    args = parser.parse_args()
    out = LOCAL / args.label
    out.mkdir(exist_ok=False)
    old, artifact = baseline_executables(out)
    new = {'input_trace.exe': args.release.resolve() / 'examples/input_trace.exe',
           'cloudleaf-ime.exe': args.release.resolve() / 'cloudleaf-ime.exe'}
    dictionaries = {'runtime': identity(RUNTIME), **{k: identity(p) for k, p in MODELS.items()}}
    assert {k: v['sha256'] for k, v in dictionaries.items()} == KNOWN
    from r21_evidence_common import git
    assert EVENTS.read_bytes() == git('show', f'{R20}:tests/fixtures/input-round14-events.tsv')
    report = {'head': head(), 'baseline_head': R20, 'started_utc': now(), 'tool': identity(__file__),
              'baseline_artifact': artifact, 'execution_scope': 'idle translation; fixed Engine K10 and no-window Host K5',
              'inputs': {**dictionaries, 'events': identity(EVENTS)},
              'binaries': {version: {k: identity(p) for k, p in paths.items()}
                           for version, paths in (('r20', old), ('r21', new))},
              'runs': [], 'comparisons': []}
    payload = EVENTS.read_bytes()
    groups = {}
    for i in range(args.rounds):
        for scope, exe in (('engine', 'input_trace.exe'), ('host', 'cloudleaf-ime.exe')):
            for kind, model in MODELS.items():
                stable = {}
                for version, paths in ((('r20', old), ('r21', new)) if i % 2 == 0 else (('r21', new), ('r20', old))):
                    direct = paths[exe]
                    trace = [RUNTIME, KNOWN['runtime'], model, KNOWN[kind]]
                    command = ([direct, *trace, 'lexical' if kind == 'lexical' else 'character_bigram']
                               if scope == 'engine' else [direct, '--benchmark-input-host', *trace, EVENTS])
                    name = f'{scope}-{i+1}-{kind}-{version}'
                    record = run(out / name, command, payload=payload if scope == 'engine' else None)
                    timings, stable[version] = semantic(out / name, scope)
                    groups.setdefault(f'{scope}/{kind}/{version}', []).extend(timings)
                    report['runs'].append(record)
                    put_json(out / 'summary.json', report)
                same = stable['r20'] == stable['r21']
                report['comparisons'].append({'scope': scope, 'model': kind, 'round': i+1,
                    'semantic_lines': len(stable['r20']), 'equal': same})
                if not same:
                    put_json(out / f'difference-{scope}-{i+1}-{kind}.json', stable)
    report['timing_ns'] = {k: percentile(v) for k, v in groups.items()}
    report['resource_max'] = {k: max(r[k] for sample in report['runs'] for r in sample['memory'])
                              for k in ('working_bytes', 'peak_working_bytes', 'private_bytes')}
    report['finished_utc'] = now()
    put_json(out / 'summary.json', report)
    print(f"{len(report['comparisons'])} comparisons, {sum(c['equal'] for c in report['comparisons'])} exact")
    assert all(c['equal'] for c in report['comparisons'])


if __name__ == '__main__':
    main()
