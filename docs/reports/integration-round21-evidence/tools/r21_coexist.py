"""同一进程测量离线翻译模型和合成用户字典对完整逐键输入的影响。"""

import argparse
import tempfile
from collections import defaultdict
from pathlib import Path

from r21_evidence_common import LOCAL, identity, head, now, percentile, put_json, run
from r21_input_traces import EVENTS, KNOWN, MODELS, RUNTIME
from r21_real_models import DEFAULT_CATALOG, PINNED_CATALOG, approved_catalog


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('release', type=Path)
    parser.add_argument('--label', default='round21-model-coexist')
    args = parser.parse_args()
    release = args.release.resolve()
    out = LOCAL / args.label
    out.mkdir(exist_ok=False)
    source = LOCAL / 'r21_coexist_probe.rs'
    exe = out / 'r21_coexist_probe.exe'
    names = ('cloudleaf_input', 'cloudleaf_offline', 'cloudleaf_search', 'cloudleaf_translation',
             'cloudleaf_types', 'cloudleaf_user')
    libraries = {name: release / f'lib{name}.rlib' for name in names}
    built = {name: identity(path) for name, path in libraries.items()}
    source_before = identity(source)
    report = {'head': head(), 'started_utc': now(), 'tool': identity(__file__), 'source': source_before,
              'libraries': built, 'catalog': approved_catalog(DEFAULT_CATALOG),
              'inputs': {k: identity(p) for k, p in [('runtime', RUNTIME), ('lexical', MODELS['lexical']), ('events', EVENTS)]},
              'scope': 'isolated research executable using production engine, scheduler and synthetic user Store; no Windows GUI, TSF or real user store',
              'timing_scope': 'engine.handle() only; EventResult also retained independently',
              'model': 'real pinned Marian; every scenario uses offline-only, one worker, one queue slot'}
    put_json(out / 'summary.json', report)
    # 统一 workspace 的 release rlib 带 ThinLTO bitcode，直接链接也须执行同种 LTO。
    command = ['rustc', '--edition=2024', '-O', '-C', 'debuginfo=0',
               '-C', 'lto=thin', '-C', 'codegen-units=1', '-L', f'dependency={release / "deps"}']
    for folder in (release / 'build').glob('ring-*/out'):
        command.extend(('-L', 'native=' + str(folder)))
    for name, path in libraries.items():
        command.extend(('--extern', f'{name}={path}'))
    report['build'] = run(out / 'build', [*command, source, '-o', exe], sample=False)
    report['binary'] = identity(exe)
    put_json(out / 'summary.json', report)
    with tempfile.TemporaryDirectory(prefix='cloudleaf-r21-fixture-') as folder:
        report['run'] = run(out / 'probe', [
            exe, RUNTIME, KNOWN['runtime'], MODELS['lexical'], KNOWN['lexical'],
            DEFAULT_CATALOG, PINNED_CATALOG, EVENTS, Path(folder) / 'synthetic-user-store',
        ], deadline_seconds=240)
    put_json(out / 'summary.json', report)
    timing = defaultdict(list)
    overlaps = defaultdict(lambda: defaultdict(list))
    semantics = defaultdict(list)
    stages = []
    for line in (out / 'probe/stdout.txt').read_text(encoding='utf-8').splitlines():
        fields = line.split('\t')
        match fields[0]:
            case 'EVENT':
                _, scope, _, nanos, stage, user = fields
                timing[scope].append(int(nanos))
                overlaps[scope][f'model={stage}, user={user}'].append(int(nanos))
            case 'RESULT':
                semantics[fields[1]].append(tuple(fields[2:]))
            case 'MODEL_LOAD' | 'MODEL_INFER' | 'TASK' | 'END':
                stages.append(fields)
    assert all(len(rows) == 171 for rows in timing.values()) and len(timing) == 7
    assert all(rows == semantics['idle-cold'] for rows in semantics.values()) and len(semantics) == 7
    assert stages[-1] == ['END', 'user_lock_reopened', 'model_released', 'translation_worker_joined']
    assert overlaps['loading']['model=1, user=false']
    assert overlaps['infer']['model=2, user=false']
    assert overlaps['queue-full']['model=2, user=false']
    # 同一进程观察到翻译与磁盘写入同时活跃才登记为双重重叠。
    both_overlap = bool(overlaps['user-store-coexist']['model=2, user=true'])
    report['stages'] = stages
    report['all_171_event_results_equal_per_scenario'] = True
    report['load_infer_user_store_same_process_overlap'] = both_overlap
    report['timing_ns'] = {k: percentile(v) for k, v in timing.items()}
    report['observed_overlap_ns'] = {k: {stage: percentile(v) for stage, v in rows.items()}
                                     for k, rows in overlaps.items()}
    report['resources'] = {key: max(r[key] for r in report['run']['memory'])
                           for key in ('working_bytes', 'peak_working_bytes', 'private_bytes', 'handles')}
    assert {name: identity(path) for name, path in libraries.items()} == built
    assert identity(source) == source_before
    report['finished_utc'] = now()
    put_json(out / 'summary.json', report)
    print(f'7 x 171 matching events, concurrent translation and Store samples: {both_overlap}')


if __name__ == '__main__':
    main()
