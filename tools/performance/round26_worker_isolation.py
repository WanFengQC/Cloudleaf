"""复用无窗口输入重放，交替比较固定 Worker 与当前代码。"""

import argparse
import ctypes
import hashlib
import json
import os
import platform
import shutil
import subprocess
import tempfile
import zipfile
from collections import defaultdict
from ctypes import wintypes
from datetime import UTC, datetime
from pathlib import Path

import round25_worker_isolation as previous
from round22_worker_replay import CATALOG_SHA, LANGUAGE_SHA, RUNTIME_SHA, ROOT, SOURCE, TRACE
from round22_worker_replay import build, flatten, identity, parse, percentiles, write_json

LOCAL = ROOT / '.local'
BASELINE = 'd56ce7167b10d7ee96ff341bb27573cdfcc7a8bc'
ROUND26 = 'a6e48cbe28d48a010629a65537cfb6743522356a'
ROUND26_REPLAY_HEAD = 'bb58b28b59a36f7167f750f8516a559cade8ef4a'
WORD_SHA = 'f8062a4a8e456135840b9df51857c84e3ea3f16f59ced4f9dc85a5bd19fade3a'
ARCHIVED_PROBE = '2e6db901532cdf0f961b0fdf3324bfa16ed88b6680fd439a612300a4070c2091'
WORKER_NAME = 'cloudleaf-translation-worker.exe'
STRATEGIES = {'normal': '1', 'one-below': '1-below', 'auto': 'auto',
              'auto-below': 'auto-below', 'pinned': '1'}
TRACES = {'171': TRACE, '243': previous.EXTENDED}


def read_json(path):
    with path.open(encoding='utf-8') as file:
        return json.load(file)


def freeze_executables(out, release, build_receipt, exact_baseline_release=None, reused=None):
    archived = LOCAL / 'round25-worker-paired-verified'
    prior = read_json(archived / 'summary.json')
    source = subprocess.check_output(['git', 'show', f'{BASELINE}:tools/performance/round22-worker-probe.rs'],
                                     cwd=ROOT)
    assert hashlib.sha256(source).hexdigest() == prior['files']['probe']['sha256']
    archive = LOCAL / 'round25-ci/final-downloads'
    receipt = read_json(archive / 'contents-verification.json')
    assert receipt['head'] == BASELINE
    built = receipt['artifacts']['integration-round25-windows']['members']
    entry = f'target/release/{WORKER_NAME}'
    folder = out / 'round25-bin'
    folder.mkdir()
    baseline_probe = folder / 'probe.exe'
    shutil.copyfile(archived / 'bin/probe.exe', baseline_probe)
    assert identity(baseline_probe)['sha256'] == prior['files']['executable']['sha256'] == ARCHIVED_PROBE
    baseline_worker = folder / WORKER_NAME
    with zipfile.ZipFile(archive / 'integration-round25-windows.zip') as bundle:
        baseline_worker.write_bytes(bundle.read(entry))
    assert identity(baseline_worker)['sha256'] == built[entry]['sha256']
    if exact_baseline_release is not None:
        assert identity(SOURCE)['sha256'] == prior['files']['probe']['sha256']
        baseline_probe = build(exact_baseline_release, folder, executable=folder / 'exact-probe.exe')
    result = {'round25': {'probe': identity(baseline_probe), 'worker': identity(baseline_worker),
                          'exact_release': exact_baseline_release is not None,
                          'exact_probe_build': identity(folder / 'build.json') if exact_baseline_release else None,
                          'archived_probe_head': prior['head'],
                          'probe_source_sha256': prior['files']['probe']['sha256'],
                          'archived_build': identity(archived / 'build.json'),
                          'ci_contents': identity(archive / 'contents-verification.json')}}
    if reused is not None:
        current = read_json(reused / 'summary.json')['executables']['round26']
        folder = out / 'round26-bin'
        folder.mkdir()
        for name, leaf in (('probe', 'probe.exe'), ('worker', WORKER_NAME)):
            copy = folder / leaf
            shutil.copyfile(Path(current[name]['path']), copy)
            record = identity(copy)
            assert record['sha256'] == current[name]['sha256']
            current[name] = record
        current['reused_summary'] = identity(reused / 'summary.json')
        result['round26'] = current
    elif release is not None:
        receipt = read_json(build_receipt)
        if receipt['returncode'] != 0:
            raise ValueError('Release 构建未成功')
        folder = out / 'round26-bin'
        folder.mkdir()
        worker = folder / WORKER_NAME
        shutil.copyfile(release / WORKER_NAME, worker)
        assert identity(worker)['sha256'] == identity(release / WORKER_NAME)['sha256']
        probe = build(release, out, executable=folder / 'probe.exe')
        result['round26'] = {'probe': identity(probe), 'worker': identity(worker),
                             'build_head': receipt['head'], 'build_receipt': identity(build_receipt),
                             'probe_source_sha256': identity(SOURCE)['sha256'],
                             'local_build': identity(out / 'build.json')}
    return result


def freeze_round27(out, release, build_receipt, frozen):
    prior = read_json(frozen)
    assert prior['head'] == ROUND26_REPLAY_HEAD and not prior['errors']
    changed = subprocess.check_output(
        ['git', 'diff', '--name-only', ROUND26_REPLAY_HEAD, ROUND26, '--',
         'Cargo.toml', 'Cargo.lock', 'crates', 'apps/cloudleaf-translation-worker', str(SOURCE.relative_to(ROOT))],
        cwd=ROOT,
    )
    assert not changed, '已冻结的 Round 26 测量与最终产品源码有差异'
    identity_before = prior['executables']['round26']
    base, current = (out / label for label in ('round26-bin', 'round27-bin'))
    base.mkdir()
    current.mkdir()
    fixed = {'prior_measurement': identity(frozen), 'build_head': identity_before['build_head']}
    for role, filename in [('probe', 'probe.exe'), ('worker', WORKER_NAME)]:
        target = base / filename
        shutil.copyfile(identity_before[role]['path'], target)
        fixed[role] = identity(target)
        assert fixed[role]['sha256'] == identity_before[role]['sha256']
    receipt = read_json(build_receipt)
    assert receipt['returncode'] == 0
    worker = current / WORKER_NAME
    shutil.copyfile(release / WORKER_NAME, worker)
    probe = build(release, out, executable=current / 'probe.exe')
    return {'round26': fixed, 'round27': {'probe': identity(probe), 'worker': identity(worker),
             'build_head': receipt['head'], 'build_receipt': identity(build_receipt),
             'local_build': identity(out / 'build.json'), 'probe_source_sha256': identity(SOURCE)}}


def system_cpu(kernel):
    ticks = [wintypes.FILETIME() for _ in range(3)]
    if not kernel.GetSystemTimes(*(ctypes.byref(t) for t in ticks)):
        return None
    return [(t.dwHighDateTime << 32) | t.dwLowDateTime for t in ticks]


def cpu_usage(start, end):
    if start is None or end is None:
        return None
    idle, kernel, user = (a - b for a, b in zip(end, start, strict=True))
    if kernel + user == 0:
        return None
    return 1 - idle / (kernel + user)


def matrix(variants, policies, repeats, lifecycle):
    comparable = [('idle', 'normal'), ('infer', 'normal')]
    diagnostics = ('ready', 'loading', 'warm', 'full', 'cancel-next', 'direction-switch',
                   'store', 'after-exit', 'repeat-release')
    for iteration in range(repeats):
        cases = [(size, mode, stage, policy, version) for size in TRACES for mode in ('lexical', 'word')
                 for stage, policy in comparable for version in variants]
        if 'round26' in variants:
            cases.extend((size, mode, 'infer', policy, 'round26') for size in TRACES
                         for mode in ('lexical', 'word') for policy in policies
                         if policy != 'normal')
            if 'pinned' in policies:
                cases.extend((size, mode, 'idle', 'pinned', 'round26') for size in TRACES
                             for mode in ('lexical', 'word'))
        for size, mode, stage, policy, version in cases[::(-1 if iteration % 2 else 1)]:
            yield iteration + 1, size, mode, stage, policy, version
    if lifecycle:
        for mode in ('lexical', 'word'):
            for stage in diagnostics:
                yield 1, '243', mode, stage, 'normal', variants[-1]


def latency(rows):
    return {field: percentiles([r[field] for r in rows]) for field in ('wall_ns', 'cpu_ns', 'cycles')} | {
        'observed_migrations': sum(row['core_before'] != row['core_after'] for row in rows),
        'store_overlap': sum(row['store'] for row in rows),
    }


def attach_windows(keys, model):
    return {phase: [key for key in keys if model.get(f'{phase}_start', 0) <= key['start']
                    <= key['end'] <= model.get(f'{phase}_end', 0)] for phase in ('infer', 'load')}


def compare_outputs(semantics, mode, size, reference, models, translation):
    behavior = hashlib.sha256('\n'.join(semantics).encode()).hexdigest()
    if reference.setdefault((size, mode), behavior) != behavior:
        raise ValueError('候选、事务或输出摘要与配对样本不同')
    for row in models.values():
        raw = row.pop('raw')
        row['raw_sha256'] = hashlib.sha256(raw.encode()).hexdigest()
        proof = (row['raw_sha256'], row['generated_steps'])
        if translation.setdefault(row['model'], proof) != proof:
            raise ValueError('同一固定模型和同一输入的译文或生成步出现差异')
    return behavior


def measure(args):
    if os.name != 'nt' or not args.out.resolve().is_relative_to(LOCAL):
        raise ValueError('无窗口测试需要 Windows 且输出位于 .local')
    out = args.out.resolve()
    out.mkdir(exist_ok=False, parents=True)
    fixed = read_json(LOCAL / 'round22-search-fixed/summary.json')
    models = read_json(LOCAL / 'round21-model-identity/summary.json')['models']
    runtime = identity(Path(fixed['runtime']['path']))
    lexical = identity(Path(fixed['models']['lexical']['path']))
    catalog = identity(Path(models['catalog']['path']))
    word = identity(LOCAL / 'round24-corpus/final-minimum-base/word-context.tsv')
    assert [record['sha256'] for record in (runtime, lexical, catalog, word)] == [
        RUNTIME_SHA, LANGUAGE_SHA, CATALOG_SHA, WORD_SHA]
    frozen = args.baseline_round26_quiet
    if frozen:
        exes = freeze_round27(out, args.release, args.build_receipt, frozen)
    else:
        exes = freeze_executables(out, args.release, args.build_receipt, args.exact_baseline_release, args.reuse_current)
    kernel, psapi = previous.api()
    kernel.GetSystemTimes.argtypes = [ctypes.POINTER(wintypes.FILETIME)] * 3
    layout = previous.topology(kernel)
    policies = list(STRATEGIES)
    if args.normal_only:
        policies = ['normal']
    if 'pinned' in policies and (layout['status'] != 'configured' or args.no_pin):
        policies.remove('pinned')
    event_counts = {name: previous.expanded_events(path) for name, path in TRACES.items()}
    assert event_counts == {'171': 171, '243': 243}
    report = {'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'baseline_head': ROUND26 if frozen else BASELINE, 'started_utc': datetime.now(UTC).isoformat(),
              'environment': platform.platform(), 'processor': platform.processor(), 'topology': layout,
              'resources': {'runtime': runtime, 'lexical': lexical, 'catalog': catalog, 'word': word,
                            'models': models, 'traces': {k: identity(v) for k, v in TRACES.items()}},
              'runner': identity(Path(__file__)), 'executables': exes, 'runs': [], 'errors': [],
              'not_measured': ['ETW', 'scheduler ready-time', 'LLC and DRAM', 'allocator contention'],
              'stated_quiet': args.stated_quiet}
    reference, translation = {}, {}
    groups = defaultdict(lambda: {'all': [], 'window': [], 'loading': [], 'runs': [],
                                  'models': [], 'cancels': [], 'ready': [], 'sample_cpu': []})
    for iteration, size, mode, stage, policy, version in matrix(list(exes), policies, args.repeats, args.lifecycle):
        name = f'{size}-{iteration}-{mode}-{stage}-{policy}-{version}'
        try:
            with tempfile.TemporaryDirectory(prefix='cloudleaf-r26-worker-') as tmp:
                gate = Path(tmp) / 'go'
                command = list(map(str, [exes[version]['probe']['path'], runtime['path'], RUNTIME_SHA,
                                         lexical['path'], LANGUAGE_SHA, catalog['path'], CATALOG_SHA,
                                         TRACES[size], Path(tmp) / 'store', stage, STRATEGIES[policy]]))
                if mode == 'word':
                    command.extend([word['path'], WORD_SHA])
                command.extend(['--replay-gate', str(gate)])
                start = system_cpu(kernel)
                lines, observed = previous.execute(out / name, command, gate, policy, stage, kernel, psapi, layout)
                elapsed_cpu = cpu_usage(start, system_cpu(kernel))
            keys, semantics, measured, codec, cancelled, stop, stages = parse(lines, event_counts[size])
            cycles = {int(i): int(n) for _, i, n in
                      (row.split('\t') for row in lines if row.startswith('CYCLE\t'))}
            assert len(cycles) == len(keys)
            for key in keys:
                key['cycles'] = cycles[key['index']]
            if policy == 'pinned':
                mask = observed['affinity']['input']['actual']
                assert all(mask & (1 << key[f'core_{point}']) for key in keys for point in ('before', 'after'))
            if 'worker' in observed['affinity']:
                assert observed['affinity']['worker']['priority'] == (0x4000 if policy.endswith('-below') else 0x20)
            for model in measured.values():
                assert model['compute_threads'] == observed['compute_threads'] and model['resident_models'] == 1
            core = measured.get('main', {})
            windows = attach_windows(keys, core)
            if stage in {'infer', 'warm', 'loading'}:
                assert windows['load' if stage == 'loading' else 'infer'], '未采到真实事件窗口'
            behavior = compare_outputs(semantics, mode, size, reference, measured, translation)
            memory = flatten(observed)
            ready = next((int(line.split('\t')[2]) for line in lines if line.startswith('READY\t')), None)
            run = {'label': name, 'repetition': iteration, 'version': version, 'trajectory': size, 'mode': mode,
                   'stage': stage, 'policy': policy, 'events': len(keys), 'candidate_sha256': behavior,
                   'infer_window_events': len(windows['infer']), 'load_window_events': len(windows['load']),
                   'models': measured, 'resources': memory, 'system_cpu_fraction': elapsed_cpu,
                   'cancel_ns': cancelled, 'ready_ns': ready, 'shutdown_ns': stop, 'codec_ns': percentiles(codec),
                   'stages': stages, 'actual': observed['affinity'], 'actual_compute': observed['compute_threads'],
                   'record': identity(out / name / 'result.json')}
            report['runs'].append(run)
            group = groups[f'{version}/{size}/{mode}/{stage}/{policy}']
            group['all'].extend(keys)
            group['window'].extend(windows['infer'])
            group['loading'].extend(windows['load'])
            group['models'].append(core or measured.get('recovered'))
            group['cancels'].extend(cancelled)
            group['runs'].append(memory)
            if ready is not None:
                group['ready'].append(ready)
            if elapsed_cpu is not None:
                group['sample_cpu'].append(elapsed_cpu)
            print(f"{name}: {len(keys)} keys, {len(windows['infer'])} infer, {len(windows['load'])} load", flush=True)
        except (OSError, RuntimeError, AssertionError, ValueError) as error:
            report['errors'].append({'label': name, 'error': str(error) or type(error).__name__})
            print(f'{name}: {type(error).__name__}: {error}', flush=True)
        write_json(out / 'summary.json', report)
    report['aggregate'] = {}
    for label, group in groups.items():
        rows = [model for model in group['models'] if model]
        stats = {'all': latency(group['all']), 'inference': latency(group['window']),
                 'loading': latency(group['loading']), 'cancellation_ns': percentiles(group['cancels']),
                 'handshake_ns': percentiles(group['ready']),
                 'system_cpu_fraction': percentiles(group['sample_cpu']),
                 'peak_observed_memory': {
                     kind: {field: max((run[kind][field] for run in group['runs']
                                        if run[kind][field] is not None), default=None)
                            for field in ('rss', 'private')}
                     for kind in ('host', 'worker')
                 }}
        if rows:
            stats['translation'] = {field: percentiles([row[field] for row in rows])
                                    for field in ('load_ns', 'verification_ns', 'inference_ns', 'generated_steps',
                                                  'host_request_to_result_ns', 'queue_wait_ns')}
            elapsed = sum(row['inference_ns'] for row in rows)
            stats['steps_per_second'] = sum(row['generated_steps'] for row in rows) / (elapsed / 1e9)
        report['aggregate'][label] = stats
    report['finished_utc'] = datetime.now(UTC).isoformat()
    write_json(out / 'summary.json', report)
    return bool(report['errors'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--release', type=Path)
    parser.add_argument('--reuse-current', type=Path)
    parser.add_argument('--exact-baseline-release', type=Path)
    parser.add_argument('--baseline-round26-quiet', type=Path)
    parser.add_argument('--normal-only', action='store_true')
    parser.add_argument('--build-receipt', type=Path)
    parser.add_argument('--repeats', type=int, choices=range(1, 7), default=3)
    parser.add_argument('--no-pin', action='store_true')
    parser.add_argument('--lifecycle', action='store_true')
    parser.add_argument('--stated-quiet', action='store_true')
    args = parser.parse_args()
    if args.release is not None and args.reuse_current is not None:
        parser.error("choose fresh or reused Release binaries")
    if (args.release is None) != (args.build_receipt is None):
        parser.error('--release 与 --build-receipt 需同时提供')
    if args.baseline_round26_quiet and (not args.release or not args.normal_only
                                       or args.exact_baseline_release):
        parser.error('Round 26 冻结配对使用新 Release 构建与 --normal-only')
    if args.release is not None:
        args.release = args.release.resolve()
        args.build_receipt = args.build_receipt.resolve()
    if args.reuse_current is not None:
        args.reuse_current = args.reuse_current.resolve()
    if args.exact_baseline_release is not None:
        args.exact_baseline_release = args.exact_baseline_release.resolve()
    raise SystemExit(measure(args))


if __name__ == '__main__':
    main()
