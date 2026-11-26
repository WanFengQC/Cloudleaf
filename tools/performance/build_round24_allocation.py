"""链接独立分配探针，保留 Cargo 锁定库身份和实际 rustc 返回码。"""
import argparse
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / '.local'))
from r21_evidence_common import identity, put_json, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('cargo_json', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    crates = {'cloudleaf_input', 'cloudleaf_search', 'ring'}
    libs = {}
    for line in args.cargo_json.read_text(encoding='utf-8').splitlines():
        record = json.loads(line)
        target = record.get('target', {})
        if record.get('reason') == 'compiler-artifact' and target['name'] in crates:
            files = [Path(name) for name in record['filenames'] if name.endswith('.rlib')]
            if files:
                libs[target['name']] = files[0]
    assert libs.keys() == crates, libs.keys()
    deps = libs['ring'].parent
    output = args.output.resolve() / ('round24-allocation-probe.exe' if os.name == 'nt'
                                     else 'round24-allocation-probe')
    script = ROOT / 'tools/performance/round24_allocation_probe.rs'
    command = ['rustc', '--edition=2024', '-O', '-C', 'lto=thin', '-C', 'codegen-units=1',
               '-L', f'dependency={deps}']
    for native in (deps.parent / 'build').glob('ring-*/out'):
        command.extend(('-L', f'native={native}'))
    for name, path in sorted(libs.items()):
        command.extend(('--extern', f'{name}={path}'))
    command.extend([script, '-o', output])
    record = run(args.output / 'link', command, cwd=args.output.resolve(), sample=False)
    put_json(args.output / 'summary.json', {'source': identity(script), 'tool': identity(__file__),
                                            'cargo_artifacts': identity(args.cargo_json),
                                            'libraries': {k: identity(p) for k, p in libs.items()},
                                            'binary': identity(output), 'execution': record})
    print(output)


if __name__ == '__main__':
    main()
