"""Windows 无窗口固定轨迹，按当前物理拓扑复测离线子进程隔离。"""
import argparse
import ctypes
import hashlib
import json
import os
import platform
import shutil
import struct
import subprocess
import tempfile
import time
from collections import defaultdict
from ctypes import wintypes
from datetime import UTC, datetime
from pathlib import Path

import round22_worker_replay as base

ROOT = base.ROOT
LOCAL = ROOT / '.local'
EXTENDED = ROOT / 'tests/fixtures/round25-worker-input.tsv'
TOKEN = {'<Blur>', '<Left>', '<Right>', '<Space>', '<Enter>', '<Esc>', '<Backspace>', '<Down>', '<PageDown>'}
BASELINE_WORKER_SHA256 = '6c52a4842612b77f53a9efa9e0504b55134c508fdf8a8e7eee041fec92d8cefc'


class ThreadEntry(ctypes.Structure):
    _fields_ = [('size', wintypes.DWORD), ('usage', wintypes.DWORD), ('thread', wintypes.DWORD),
                ('pid', wintypes.DWORD), ('priority', wintypes.LONG), ('delta', wintypes.LONG),
                ('flags', wintypes.DWORD)]


def api():
    kernel, psapi = base.win_api()
    kernel.GetActiveProcessorGroupCount.restype = wintypes.WORD
    kernel.GetLogicalProcessorInformationEx.argtypes = [wintypes.DWORD, ctypes.c_void_p,
                                                        ctypes.POINTER(wintypes.DWORD)]
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    kernel.GetProcessAffinityMask.argtypes = [wintypes.HANDLE, ctypes.POINTER(ctypes.c_size_t),
                                              ctypes.POINTER(ctypes.c_size_t)]
    kernel.SetProcessAffinityMask.argtypes = [wintypes.HANDLE, ctypes.c_size_t]
    kernel.GetThreadTimes.argtypes = [wintypes.HANDLE, *[ctypes.POINTER(wintypes.FILETIME)] * 4]
    kernel.OpenThread.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenThread.restype = wintypes.HANDLE
    kernel.GetThreadDescription.argtypes = [wintypes.HANDLE, ctypes.POINTER(ctypes.c_void_p)]
    kernel.GetThreadDescription.restype = ctypes.c_long
    kernel.LocalFree.argtypes = [ctypes.c_void_p]
    kernel.LocalFree.restype = ctypes.c_void_p
    kernel.Thread32First.argtypes = kernel.Thread32Next.argtypes = [wintypes.HANDLE, ctypes.POINTER(ThreadEntry)]
    return kernel, psapi


def mask_of(kernel, handle):
    own, system = ctypes.c_size_t(), ctypes.c_size_t()
    if not kernel.GetProcessAffinityMask(handle, ctypes.byref(own), ctypes.byref(system)):
        raise ctypes.WinError(ctypes.get_last_error())
    return own.value


def topology(kernel):
    if ctypes.sizeof(ctypes.c_size_t) != 8 or kernel.GetActiveProcessorGroupCount() != 1:
        return {'status': 'unsupported-processor-group-layout'}
    size = wintypes.DWORD()
    kernel.GetLogicalProcessorInformationEx(0, None, ctypes.byref(size))
    if ctypes.get_last_error() != 122 or size.value < 48:
        raise ctypes.WinError(ctypes.get_last_error())
    buffer = ctypes.create_string_buffer(size.value)
    if not kernel.GetLogicalProcessorInformationEx(0, buffer, ctypes.byref(size)):
        raise ctypes.WinError(ctypes.get_last_error())
    allowed = mask_of(kernel, kernel.GetCurrentProcess())
    cores = []
    offset = 0
    while offset < size.value:
        kind, record_bytes = struct.unpack_from('<II', buffer, offset)
        count, mask, group = struct.unpack_from('<HQH', buffer, offset + 30)
        if kind != 0 or count != 1 or group != 0 or record_bytes != 48:
            return {'status': 'unsupported-core-record'}
        if mask & allowed == mask:
            cores.append({'mask': mask, 'efficiency': buffer[offset+9][0], 'smt': mask.bit_count()})
        offset += record_bytes
    cores.sort(key=lambda row: (-row['efficiency'], row['mask']))
    # 拆分完整物理核心，避免把同一核心的 SMT 线程划给双方。
    if len(cores) < 3:
        return {'status': 'insufficient-available-physical-cores', 'allowed': allowed, 'cores': cores}
    selected = cores[:3]
    if len({x['efficiency'] for x in selected}) != 1:
        return {'status': 'no-comparable-core-class', 'allowed': allowed, 'cores': cores}
    return {'status': 'configured', 'allowed': allowed, 'cores': cores,
            'input_mask': selected[0]['mask'], 'worker_mask': selected[1]['mask'] | selected[2]['mask']}


def configure(kernel, pid, mask):
    handle = kernel.OpenProcess(0x0600 if mask is not None else 0x0400, False, pid)
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        previous = mask_of(kernel, handle)
        if mask is not None and not kernel.SetProcessAffinityMask(handle, mask):
            raise ctypes.WinError(ctypes.get_last_error())
        actual = mask_of(kernel, handle)
        if mask is not None and actual != mask:
            raise RuntimeError('CPU 亲和性回读与请求不同')
        return {'pid': pid, 'before': previous, 'actual': actual, 'priority': kernel.GetPriorityClass(handle)}
    finally:
        kernel.CloseHandle(handle)


def capture_worker_threads(kernel, pid):
    handle = kernel.CreateToolhelp32Snapshot(4, 0)
    if handle == ctypes.c_void_p(-1).value:
        return []
    rows = []
    try:
        entry = ThreadEntry(size=ctypes.sizeof(ThreadEntry))
        more = kernel.Thread32First(handle, ctypes.byref(entry))
        while more:
            if entry.pid == pid:
                thread = kernel.OpenThread(0x0800, False, entry.thread)
                if thread:
                    try:
                        ticks = [wintypes.FILETIME() for _ in range(4)]
                        name = ctypes.c_void_p()
                        if kernel.GetThreadTimes(thread, *(ctypes.byref(tick) for tick in ticks)):
                            names = None
                            if kernel.GetThreadDescription(thread, ctypes.byref(name)) == 0 and name.value:
                                try:
                                    names = ctypes.wstring_at(name) or None
                                finally:
                                    kernel.LocalFree(name)
                            values = [(tick.dwHighDateTime << 32) | tick.dwLowDateTime for tick in ticks]
                            rows.append({'tid': entry.thread, 'name': names,
                                         'creation_ticks': values[0], 'cpu_ns': (values[2] + values[3]) * 100})
                    finally:
                        kernel.CloseHandle(thread)
            entry.size = ctypes.sizeof(ThreadEntry)
            more = kernel.Thread32Next(handle, ctypes.byref(entry))
    finally:
        kernel.CloseHandle(handle)
    return rows


def execute(directory, command, gate, policy, scenario, kernel, psapi, layout):
    directory.mkdir()
    out, err = directory / 'stdout.txt', directory / 'stderr.txt'
    started = time.monotonic()
    samples, affinity = [], {}
    pid = None
    selected = None
    failure = None
    with out.open('wb') as stdout, err.open('wb') as stderr, out.open('rb') as monitor:
        child = subprocess.Popen(command, cwd=ROOT, stdout=stdout, stderr=stderr, stdin=subprocess.DEVNULL,
                                 creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            while child.poll() is None:
                if time.monotonic() - started > 60:
                    raise TimeoutError('资源重放超过 60 秒')
                while True:
                    position = monitor.tell()
                    line = monitor.readline()
                    if not line.endswith(b'\n'):
                        monitor.seek(position)
                        break
                    if line.startswith(b'READY\t'):
                        pid = int(line.split(b'\t')[1])
                    elif line.startswith(b'RESOURCE\t'):
                        selected = int(line.split(b'\t')[1])
                pids = {child.pid} if pid is None else {child.pid, pid}
                threads = base.thread_counts(kernel, pids)
                if selected is not None and not affinity:
                    if scenario != 'idle' and (pid is None or threads.get(pid, (None, None))[1] != child.pid):
                        raise RuntimeError('无法核验受测工作进程的父 PID')
                    pinned = policy == 'pinned'
                    if pid is not None:
                        affinity['worker'] = configure(kernel, pid, layout['worker_mask'] if pinned else None)
                        if policy == 'auto-below' and affinity['worker']['priority'] != 0x4000:
                            raise RuntimeError('工作进程优先级回读失败')
                    affinity['input'] = configure(kernel, child.pid, layout['input_mask'] if pinned else None)
                    gate.touch(exist_ok=False)
                samples.append({'unix_ns': time.time_ns(), 'host': base.sample(kernel, psapi, child.pid, threads),
                                'workers': [row] if pid and (row := base.sample(kernel, psapi, pid, threads)) else [],
                                'worker_threads': capture_worker_threads(kernel, pid) if pid else []})
                time.sleep(.05)
        except (OSError, RuntimeError, ValueError) as error:
            failure = str(error)
            child.kill()
        rc = child.wait(timeout=10)
    result = {'command': command, 'returncode': rc, 'seconds': time.monotonic() - started,
              'error': failure, 'host_pid': child.pid, 'worker_pid': pid, 'samples': samples,
              'affinity': affinity, 'compute_threads': selected, 'policy': policy,
              'stdout': base.identity(out), 'stderr': base.identity(err)}
    base.write_json(directory / 'result.json', result)
    if rc or failure or not affinity:
        raise RuntimeError(f'{directory.name}: returned {rc}: {failure}; evidence retained')
    return out.read_text(encoding='utf-8').splitlines(), result


def expanded_events(path):
    rows = (line.split('\t', 1)[1] for line in path.read_text(encoding='utf-8').splitlines()
            if line and not line.startswith('#'))
    return sum(1 if token in TOKEN else len(token) for keys in rows for token in keys.split())


def thread_observations(samples):
    threads = {}
    for sample in samples:
        for row in sample['worker_threads']:
            key = (row['tid'], row['creation_ticks'])
            if key not in threads:
                threads[key] = {'tid': row['tid'], 'creation_ticks': row['creation_ticks'], 'name': row['name'],
                                'min_cpu_ns': row['cpu_ns'], 'max_cpu_ns': row['cpu_ns'], 'samples': 0}
            item = threads[key]
            item['min_cpu_ns'] = min(item['min_cpu_ns'], row['cpu_ns'])
            item['max_cpu_ns'] = max(item['max_cpu_ns'], row['cpu_ns'])
            item['samples'] += 1
    return sorted(threads.values(), key=lambda row: (row['name'] or '', row['tid']))


def compare(release, out, repeats, include_extended, extra_only=False, baseline=None):
    if os.name != 'nt':
        raise RuntimeError('此探针需要 Windows 进程和线程测量 API')
    out.mkdir(exist_ok=False)
    fixed = json.loads((LOCAL / 'round22-search-fixed/summary.json').read_bytes())
    model_identity = json.loads((LOCAL / 'round21-model-identity/summary.json').read_bytes())['models']
    catalog = Path(model_identity['catalog']['path'])
    runtime = Path(fixed['runtime']['path'])
    lexical = Path(fixed['models']['lexical']['path'])
    word = LOCAL / 'round24-corpus/final-minimum-base/word-context.tsv'
    word_id = base.identity(word)
    assert word_id['sha256'] == 'f8062a4a8e456135840b9df51857c84e3ea3f16f59ced4f9dc85a5bd19fade3a'
    files = {name: base.identity(path) for name, path in
             {'runtime': runtime, 'lexical': lexical, 'catalog': catalog, 'word': word,
              'trace': base.TRACE, 'extended': EXTENDED, 'worker': release / 'cloudleaf-translation-worker.exe',
              'tool': Path(__file__), 'probe': base.SOURCE}.items()}
    assert files['runtime']['sha256'] == base.RUNTIME_SHA
    assert files['lexical']['sha256'] == base.LANGUAGE_SHA
    assert files['catalog']['sha256'] == base.CATALOG_SHA
    active = out / 'bin'
    active.mkdir()
    worker = active / 'cloudleaf-translation-worker.exe'
    shutil.copyfile(release / worker.name, worker)
    assert base.identity(worker)['sha256'] == files['worker']['sha256']
    exe = base.build(release, out, executable=active / 'probe.exe')
    files['executable'] = base.identity(exe)
    legacy_exe = None
    if baseline is not None:
        files['baseline_worker'] = base.identity(baseline)
        assert files['baseline_worker']['sha256'] == BASELINE_WORKER_SHA256
        legacy = out / 'legacy-bin'
        legacy.mkdir()
        shutil.copyfile(baseline, legacy / worker.name)
        legacy_exe = legacy / exe.name
        shutil.copyfile(exe, legacy_exe)
        assert base.identity(legacy_exe)['sha256'] == files['executable']['sha256']
    kernel, psapi = api()
    layout = topology(kernel)
    summary = {'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
               'started_utc': datetime.now(UTC).isoformat(), 'environment': platform.platform(),
               'processor': platform.processor(), 'topology': layout, 'files': files, 'runs': [], 'errors': [],
               'notes': {'cpu_residual': 'wall minus quantized accounted CPU, not scheduler ready-time',
                         'memory': 'process working set and private commit, not per-query heap/DRAM attribution'}}
    configurations = [(scenario, policy) for scenario, policy in
                      [('idle', 'normal'), ('infer', 'normal'), ('infer', 'auto'), ('infer', 'auto-below'),
                       ('idle', 'pinned'), ('infer', 'pinned')]
                      if policy != 'pinned' or layout['status'] == 'configured']
    if legacy_exe:
        configurations.insert(2, ('infer', 'round24'))
    matrix = [] if extra_only else [(repeat, mode, scenario, policy, base.TRACE)
              for repeat in range(repeats)
              for mode, scenario, policy in ([(m, s, p) for m in ('lexical', 'word') for s, p in configurations]
                                             [::(1 if repeat % 2 == 0 else -1)])]
    if include_extended:
        matrix.extend((0, mode, scenario, 'pinned' if layout['status'] == 'configured' else 'normal', EXTENDED)
                      for mode in ('lexical', 'word')
                      for scenario in ('idle', 'warm', 'cancel-next', 'direction-switch', 'ready', 'full', 'loading', 'store'))
    assert matrix
    reference = {}
    translation_reference = {}
    timing = defaultdict(list)
    translation = defaultdict(list)
    for repeat, mode, scenario, policy, trace in matrix:
        trace_key = '171' if trace == base.TRACE else 'extended'
        label = f'{trace_key}-{repeat+1}-{mode}-{scenario}-{policy}'
        thread_option = policy if policy.startswith('auto') else '1'
        try:
            with tempfile.TemporaryDirectory(prefix='cloudleaf-r25-worker-') as folder:
                gate = Path(folder) / 'start'
                selected_exe = legacy_exe if policy == 'round24' else exe
                command = list(map(str, [selected_exe, runtime, base.RUNTIME_SHA, lexical, base.LANGUAGE_SHA,
                                         catalog, base.CATALOG_SHA, trace, Path(folder) / 'store', scenario, thread_option]))
                if mode == 'word':
                    command.extend((str(word), word_id['sha256']))
                command.extend(('--replay-gate', str(gate)))
                lines, run = execute(out / label, command, gate, policy, scenario, kernel, psapi, layout)
            keys, behavior, models, codec, cancels, stop, stages = base.parse(lines, expanded_events(trace))
            signature = hashlib.sha256('\n'.join(behavior).encode()).hexdigest()
            assert signature == reference.setdefault((trace_key, mode), signature)
            cycles = {int(i): int(value) for _, i, value in
                      (line.split('\t') for line in lines if line.startswith('CYCLE\t'))}
            assert len(cycles) == len(keys)
            for item in keys:
                item['cycles'] = cycles[item['index']]
            if policy == 'pinned':
                measured_mask = run['affinity']['input']['actual']
                assert all(measured_mask & (1 << key[f'core_{endpoint}'])
                           for key in keys for endpoint in ('before', 'after'))
            main = models.get('main', {})
            overlap = [key for key in keys if main and main['infer_start'] <= key['start'] <= key['end'] <= main['infer_end']]
            if scenario in ('infer', 'warm'):
                assert overlap, '没有真实推理重叠的按键'
            model_caps = {}
            for name, metrics in models.items():
                assert metrics['compute_threads'] == run['compute_threads'] and metrics['resident_models'] == 1
                raw = metrics.pop('raw').encode()
                metrics['raw_sha256'] = hashlib.sha256(raw).hexdigest()
                if (scenario, name) != ('direction-switch', 'main'):
                    proof = (metrics['model'], metrics['raw_sha256'], metrics['generated_steps'])
                    assert proof == translation_reference.setdefault('english', proof), label
                model_caps[name] = metrics
            tag = f'{trace_key}/{mode}/{scenario}/{policy}'
            timing[tag].extend(overlap if scenario in ('infer', 'warm') else keys)
            if main:
                translation[tag].append(main)
            native_threads = thread_observations(run['samples'])
            tensor = [row for row in native_threads if (row['name'] or '').startswith('cloudleaf-tensor-')]
            if models and policy != 'round24' and tensor:
                assert len(tensor) == run['compute_threads'], '计算线程链未保持配置的一份池'
            summary['runs'].append({'label': label, 'input_behavior_sha256': signature, 'events': len(keys),
                 'actual_overlap': len(overlap), 'model': model_caps, 'codec_ns': base.percentiles(codec),
                 'cancel_ns': cancels, 'stop_ns': stop, 'stages': stages, 'resources': base.flatten(run),
                 'config': run['affinity'], 'compute_threads': run['compute_threads'],
                 'worker_threads': native_threads,
                 'raw_run': base.identity(out / label / 'result.json')})
            print(f'{label}: {len(keys)} events, {len(overlap)} real overlap', flush=True)
        except (OSError, RuntimeError, AssertionError, ValueError) as error:
            summary['errors'].append({'label': label, 'error': str(error)})
            print(f'{label}: {error}', flush=True)
        base.write_json(out / 'summary.json', summary)
    summary['timing'] = {tag: {
        'wall_ns': base.percentiles([row['wall_ns'] for row in rows]),
        'cpu_ns': base.percentiles([row['cpu_ns'] for row in rows]),
        'cycles': base.percentiles([row['cycles'] for row in rows]),
        'unaccounted_wall_ns': base.percentiles([max(0, row['wall_ns'] - row['cpu_ns']) for row in rows]),
        'observed_migrations': sum(row['core_before'] != row['core_after'] for row in rows),
    } for tag, rows in timing.items()}
    fields = ('inference_ns', 'load_ns', 'verification_ns', 'queue_wait_ns', 'host_request_to_result_ns',
              'generated_steps', 'encoder_ns', 'decoder_ns')
    summary['translation'] = {tag: {field: base.percentiles([row[field] for row in rows]) for field in fields}
                              | {'aggregate_generated_tokens_per_s': sum(row['generated_steps'] for row in rows)
                                 / (sum(row['inference_ns'] for row in rows) / 1e9)}
                              for tag, rows in translation.items()}
    summary['finished_utc'] = datetime.now(UTC).isoformat()
    base.write_json(out / 'summary.json', summary)
    return bool(summary['errors'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release', type=Path, default=LOCAL / 'target/release')
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--repeats', type=int, choices=range(1, 7), default=3)
    parser.add_argument('--extended', action='store_true')
    parser.add_argument('--extended-only', action='store_true')
    parser.add_argument('--round24-worker', type=Path)
    args = parser.parse_args()
    raise SystemExit(compare(args.release.resolve(), args.out.resolve(), args.repeats,
                             args.extended or args.extended_only, args.extended_only, args.round24_worker))


if __name__ == '__main__':
    main()
