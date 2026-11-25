"""在 Windows 运行固定按键、真实离线工作进程和合成 Store 的交替复测。"""

import argparse
import ctypes
import hashlib
import json
import math
import os
import platform
import subprocess
import tempfile
import time
from collections import defaultdict
from ctypes import wintypes
from datetime import UTC, datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TRACE = ROOT / 'tests/fixtures/input-round14-events.tsv'
SOURCE = ROOT / 'tools/performance/round22-worker-probe.rs'
CASES = [('idle', 1), ('ready', 1), ('loading', 1), ('infer', 1), ('infer', 2), ('infer', 4),
         ('full', 1), ('cancel', 1), ('direction-switch', 1), ('store', 1), ('after-exit', 1),
         ('repeat-release', 1)]
TIMING_FIELDS = ('compute_threads', 'resident_models', 'verification_ns', 'load_ns', 'inference_ns', 'encoder_ns',
                 'decoder_ns', 'generated_steps', 'queue_wait_ns', 'infer_start', 'infer_end',
                 'load_start', 'load_end', 'host_request_to_result_ns')
RUNTIME_SHA = '03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
LANGUAGE_SHA = '36dc4d9057bc70e3494fe397da09e8ab6f187c435bd735b66a7734f9d414fce5'
CATALOG_SHA = 'f88a5f57513a0aaae3bbd659950adf042c8c8dde07736084cb951f61f22c7f0f'


class Memory(ctypes.Structure):
    _fields_ = [('size', wintypes.DWORD), ('faults', wintypes.DWORD)] + [
        (field, ctypes.c_size_t) for field in (
            'peak_ws', 'ws', 'peak_paged', 'paged', 'peak_unpaged', 'unpaged',
            'pagefile', 'peak_pagefile', 'private',
        )
    ]


class Io(ctypes.Structure):
    _fields_ = [(field, ctypes.c_uint64) for field in ('reads', 'writes', 'other_ops', 'read_bytes', 'write_bytes', 'other_bytes')]


class ProcessEntry(ctypes.Structure):
    _fields_ = [('size', wintypes.DWORD), ('use', wintypes.DWORD), ('pid', wintypes.DWORD),
                ('heap', ctypes.c_size_t), ('module', wintypes.DWORD), ('threads', wintypes.DWORD),
                ('parent', wintypes.DWORD), ('priority', wintypes.LONG), ('flags', wintypes.DWORD),
                ('name', wintypes.WCHAR * 260)]


def win_api():
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    psapi = ctypes.WinDLL('psapi', use_last_error=True)
    psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(Memory), wintypes.DWORD]
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.GetPriorityClass.argtypes = [wintypes.HANDLE]
    kernel.GetPriorityClass.restype = wintypes.DWORD
    kernel.SetPriorityClass.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    kernel.GetProcessIoCounters.argtypes = [wintypes.HANDLE, ctypes.POINTER(Io)]
    kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel.Process32FirstW.argtypes = kernel.Process32NextW.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)]
    return kernel, psapi


def thread_counts(kernel, pids):
    snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
    if snapshot == ctypes.c_void_p(-1).value:
        return {}
    counts = {}
    try:
        entry = ProcessEntry()
        entry.size = ctypes.sizeof(entry)
        more = kernel.Process32FirstW(snapshot, ctypes.byref(entry))
        while more and set(counts) != pids:
            if entry.pid in pids:
                counts[entry.pid] = (entry.threads, entry.parent)
            more = kernel.Process32NextW(snapshot, ctypes.byref(entry))
    finally:
        kernel.CloseHandle(snapshot)
    return counts


def sample(kernel, psapi, pid, threads):
    handle = kernel.OpenProcess(0x0410, False, pid)
    if not handle:
        return None
    try:
        memory = Memory()
        memory.size = ctypes.sizeof(memory)
        if not psapi.GetProcessMemoryInfo(handle, ctypes.byref(memory), memory.size):
            return None
        ticks = [wintypes.FILETIME() for _ in range(4)]
        cpu = None
        if kernel.GetProcessTimes(handle, *(ctypes.byref(tick) for tick in ticks)):
            cpu = sum((tick.dwHighDateTime << 32) | tick.dwLowDateTime for tick in ticks[2:]) * 100
        io = Io()
        read_bytes = io.read_bytes if kernel.GetProcessIoCounters(handle, ctypes.byref(io)) else None
        return {'pid': pid, 'rss': memory.ws, 'peak_rss': memory.peak_ws, 'private': memory.private,
                'threads': threads.get(pid, (None, None))[0], 'process_cpu_ns': cpu, 'read_bytes': read_bytes}
    finally:
        kernel.CloseHandle(handle)


def identity(path):
    with path.open('rb') as content:
        sha = hashlib.file_digest(content, 'sha256').hexdigest()
    return {'path': str(path), 'bytes': path.stat().st_size, 'sha256': sha}


def write_json(path, record):
    path.write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')


def percentiles(values):
    xs = sorted(values)
    return {'n': len(xs), **({f'p{p}': xs[math.ceil(p * len(xs) / 100) - 1] for p in (50, 95, 99)} |
                             {'max': xs[-1]} if xs else {})}


def build(release, evidence):
    libs = ('cloudleaf_input', 'cloudleaf_search', 'cloudleaf_types', 'cloudleaf_user', 'cloudleaf_translation_ipc')
    exe = (release / 'round22-worker-probe.exe').resolve()
    cmd = ['rustc', '--edition=2024', '-O', '-C', 'lto=thin', '-C', 'codegen-units=1',
           '-L', f"dependency={release / 'deps'}"]
    for native in (release / 'build').glob('ring-*/out'):
        cmd.extend(('-L', f'native={native}'))
    for library in libs:
        cmd.extend(('--extern', f'{library}={release / ("lib" + library + ".rlib")}'))
    cmd.extend((str(SOURCE), '-o', str(exe)))
    # 在证据目录内运行 rustc，临时链接对象不进入工作区根目录。
    output = subprocess.run(cmd, cwd=evidence, capture_output=True, check=False)
    (evidence / 'build.stdout.txt').write_bytes(output.stdout)
    (evidence / 'build.stderr.txt').write_bytes(output.stderr)
    write_json(evidence / 'build.json', {'command': cmd, 'returncode': output.returncode,
               'libraries': [identity(release / f'lib{x}.rlib') for x in libs]})
    output.check_returncode()
    return exe


def set_worker_priority(kernel, pid, owner_pid, processes):
    if processes.get(pid, (None, None))[1] != owner_pid:
        return None
    handle = kernel.OpenProcess(0x1200, False, pid)
    if not handle:
        return None
    try:
        if not kernel.SetPriorityClass(handle, 0x4000):
            raise ctypes.WinError(ctypes.get_last_error())
        priority = kernel.GetPriorityClass(handle)
        assert priority == 0x4000
        return priority
    finally:
        kernel.CloseHandle(handle)


def execute(directory, command, kernel, psapi, priority):
    directory.mkdir()
    out = directory / 'stdout.txt'
    err = directory / 'stderr.txt'
    samples = []
    started = time.monotonic()
    pids = set()
    confirmed_priority = {}
    with out.open('wb') as stdout, err.open('wb') as stderr, out.open('rb') as monitor:
        child = subprocess.Popen(command, stdout=stdout, stderr=stderr, stdin=subprocess.DEVNULL,
                                 cwd=ROOT, creationflags=subprocess.CREATE_NO_WINDOW)
        while child.poll() is None:
            if time.monotonic() - started > 135:
                child.kill()
                break
            for line in monitor.readlines():
                if line.startswith((b'READY\t', b'WORKER\t')):
                    pids.add(int(line.split(b'\t')[1]))
            counts = thread_counts(kernel, pids | {child.pid})
            if priority == 'below-normal':
                for pid in pids - confirmed_priority.keys():
                    if (actual := set_worker_priority(kernel, pid, child.pid, counts)) is not None:
                        confirmed_priority[pid] = actual
            samples.append({'unix_ns': time.time_ns(), 'host': sample(kernel, psapi, child.pid, counts),
                            'workers': [row for pid in sorted(pids) if (row := sample(kernel, psapi, pid, counts))]})
            time.sleep(.05)
        rc = child.wait(timeout=10)
    row = {'command': command, 'returncode': rc, 'seconds': time.monotonic() - started,
           'host_pid': child.pid, 'worker_pids': sorted(pids), 'samples': samples,
           'priority': priority, 'verified_priority_classes': confirmed_priority,
           'stdout': identity(out), 'stderr': identity(err)}
    write_json(directory / 'result.json', row)
    if priority == 'below-normal':
        assert pids <= confirmed_priority.keys(), '实验未证实所有测得工作进程使用指定优先级'
    if rc:
        raise RuntimeError(f'{directory.name}: returned {rc}, evidence retained')
    return out.read_text(encoding='utf-8').splitlines(), row


def parse(lines):
    keys, semantics, metrics, codec, cancels, stop, stages = [], [], {}, [], [], [], []
    for line in lines:
        parts = line.split('\t')
        match parts[0]:
            case 'KEY':
                _, index, start, end, wall, cpu, cpu0, cpu1, saving = parts
                keys.append({'index': int(index), 'start': int(start), 'end': int(end), 'wall_ns': int(wall),
                             'cpu_ns': int(cpu), 'core_before': int(cpu0), 'core_after': int(cpu1), 'store': saving == 'true'})
            case 'RESULT':
                semantics.append(line)
            case 'MODEL':
                values = parts[2:]
                assert len(values) == len(TIMING_FIELDS) + 2, parts
                metrics[parts[1]] = dict(zip(TIMING_FIELDS, map(int, values[:-2]), strict=True)) | {
                    'model': values[-2], 'raw': values[-1]}
            case 'CODEC':
                codec.append(int(parts[3]))
            case 'CANCEL':
                cancels.append(int(parts[1]))
            case 'STOP':
                stop.append(int(parts[1]))
            case 'STAGE':
                stages.append(parts[1:])
    assert len(keys) == len(semantics) == 171
    assert [k['index'] for k in keys] == list(range(171))
    return keys, semantics, metrics, codec, cancels, stop, stages


def flatten(run):
    rows = run['samples']
    host = [r['host'] for r in rows if r['host']]
    workers = [worker for r in rows for worker in r['workers']]
    both = [r['host']['rss'] + sum(w['rss'] for w in r['workers']) for r in rows if r['host'] and r['workers']]
    maximum = lambda items, field: max((r[field] for r in items if r[field] is not None), default=None)
    return {
        'host_samples': len(host), 'worker_samples': len(workers),
        'host': {field: maximum(host, field) for field in ('rss', 'peak_rss', 'private', 'threads', 'process_cpu_ns', 'read_bytes')},
        'worker': {field: maximum(workers, field) for field in ('rss', 'peak_rss', 'private', 'threads', 'process_cpu_ns', 'read_bytes')},
        'paired_max_rss': max(both, default=None),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('release', 'runtime', 'language', 'catalog', 'output'):
        parser.add_argument(f'--{option}', type=Path, required=True)
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--case', action='append', help='只运行指定条件（可重复）；完整复测省略此选项')
    parser.add_argument('--compare-priority', action='store_true',
                        help='用同一分母交替对照默认进程优先级与本次创建 worker 的低于正常优先级')
    args = parser.parse_args()
    if os.name != 'nt':
        parser.error('此探针需要 Windows 线程 CPU 计数和进程资源 API')
    if not 1 <= args.repeats <= 12:
        parser.error('重复轮数必须在 1 到 12 之间')
    args.output = args.output.resolve()
    args.output.mkdir(exist_ok=False, parents=True)
    args.release = args.release.resolve()
    files = {'runtime': identity(args.runtime), 'language': identity(args.language), 'catalog': identity(args.catalog),
             'events': identity(TRACE), 'tool': identity(Path(__file__)), 'probe': identity(SOURCE),
             'worker': identity(args.release / 'cloudleaf-translation-worker.exe')}
    for name, expected in [('runtime', RUNTIME_SHA), ('language', LANGUAGE_SHA), ('catalog', CATALOG_SHA)]:
        assert files[name]['sha256'] == expected, name
    exe = build(args.release, args.output)
    summary = {'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
               'tracked_dirty': bool(subprocess.check_output(['git', 'status', '--porcelain', '-uno'], cwd=ROOT)),
               'utc': datetime.now(UTC).isoformat(), 'environment': {'os': platform.platform(), 'cpu': platform.processor()},
               'identities': files | {'executable': identity(exe)}, 'runs': [], 'errors': []}
    kernel, psapi = win_api()
    results = defaultdict(lambda: defaultdict(list))
    reference = None
    cases = [(scenario, threads, priority) for scenario, threads in CASES
             for priority in (('normal', 'below-normal') if args.compare_priority else ('normal',))]
    for repeat in range(args.repeats):
        ordered = cases if repeat % 2 == 0 else cases[::-1]
        for scenario, threads, priority in ordered:
            base_tag = f'{scenario}-{threads}'
            tag = f'{base_tag}-{priority}' if args.compare_priority else base_tag
            if args.case and scenario not in args.case and base_tag not in args.case:
                continue
            name = f'{repeat + 1}-{tag}'
            try:
                with tempfile.TemporaryDirectory(prefix='cloudleaf-r22-synthetic-') as user:
                    command = list(map(str, [exe, args.runtime.resolve(), RUNTIME_SHA, args.language.resolve(), LANGUAGE_SHA,
                                             args.catalog.resolve(), CATALOG_SHA, TRACE, Path(user) / 'store', scenario, threads]))
                    lines, run = execute(args.output / name, command, kernel, psapi, priority)
                keys, semantics, metrics, codec, cancels, stop, stages = parse(lines)
                if reference is None:
                    reference = semantics
                assert semantics == reference, '按键事件的完整结果偏离配对参考'
                cuts = {'all': keys, 'inference': [], 'loading': [], 'inference_and_store': []}
                if main_result := metrics.get('main'):
                    for group, prefix in [('inference', 'infer'), ('loading', 'load')]:
                        start, end = main_result[prefix + '_start'], main_result[prefix + '_end']
                        cuts[group] = [key for key in keys if start <= key['start'] <= key['end'] <= end]
                    cuts['inference_and_store'] = [key for key in cuts['inference'] if key['store']]
                for group, observations in cuts.items():
                    results[tag][group].extend(observations)
                summary['runs'].append({'case': name, 'tag': tag, 'semantics_equal': True,
                    'samples': {group: len(rows) for group, rows in cuts.items()},
                    'metrics': metrics, 'codec_ns': percentiles(codec), 'cancel_ns': cancels, 'stop_ns': stop,
                    'received_stage_utc_ns': stages, 'resources': flatten(run)})
                print(f"{name}: {len(keys)} events, inference={len(cuts['inference'])}, loading={len(cuts['loading'])}", flush=True)
            except (OSError, RuntimeError, AssertionError, ValueError) as error:
                summary['errors'].append({'case': name, 'error': str(error)})
                print(f'{name}: {error}', flush=True)
            finally:
                write_json(args.output / 'summary.json', summary)
    summary['event_timing_ns'] = {tag: {group: {
        'wall': percentiles([row['wall_ns'] for row in rows]),
        'cpu': percentiles([row['cpu_ns'] for row in rows]),
        'wall_minus_accounted_cpu': percentiles([max(0, row['wall_ns'] - row['cpu_ns']) for row in rows]),
        'observed_core_changes': sum(row['core_before'] != row['core_after'] for row in rows),
    } for group, rows in groups.items()} for tag, groups in results.items()}
    summary['finished_utc'] = datetime.now(UTC).isoformat()
    write_json(args.output / 'summary.json', summary)
    if summary['errors'] or not summary['runs']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
