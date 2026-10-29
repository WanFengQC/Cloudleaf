"""显式运行异常占用下的健康会话压力；仅采样自己创建的测试进程，不注册或激活输入法。"""

from __future__ import annotations

import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
import math
import uuid
from pathlib import Path
import platform
import subprocess
import threading
import time

from benchmark import source_sha256

ROOT = Path(__file__).resolve().parents[1]
TEST = ("native::session::render_tests::"
        "round5_abrupt_owner_workers_reclaim_slots_before_healthy_fifo_session")


class ThreadEntry(ctypes.Structure):
    _fields_ = [("size", wintypes.DWORD), ("usage", wintypes.DWORD),
                ("thread_id", wintypes.DWORD), ("process_id", wintypes.DWORD),
                ("base", wintypes.LONG), ("delta", wintypes.LONG), ("flags", wintypes.DWORD)]


class ProcessMemory(ctypes.Structure):
    _fields_ = [("size", wintypes.DWORD), ("faults", wintypes.DWORD)] + [
        (name, ctypes.c_size_t) for name in ["peak_working", "working", "peak_paged", "paged",
                                             "peak_nonpaged", "nonpaged", "pagefile", "peak_pagefile", "private"]]


def collector():
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessMemory), wintypes.DWORD]
    psapi.GetProcessMemoryInfo.restype = wintypes.BOOL
    kernel.GetProcessHandleCount.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
    kernel.GetProcessHandleCount.restype = wintypes.BOOL
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel.Thread32First.argtypes = [wintypes.HANDLE, ctypes.POINTER(ThreadEntry)]
    kernel.Thread32First.restype = wintypes.BOOL
    kernel.Thread32Next.argtypes = [wintypes.HANDLE, ctypes.POINTER(ThreadEntry)]
    kernel.Thread32Next.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL

    def memory(process):
        counters = ProcessMemory()
        counters.size = ctypes.sizeof(counters)
        if not psapi.GetProcessMemoryInfo(int(process._handle), ctypes.byref(counters), counters.size):
            raise ctypes.WinError(ctypes.get_last_error())
        return {"working_set_bytes": counters.working, "os_peak_working_set_bytes": counters.peak_working,
                "private_commit_bytes": counters.private}

    def sample(process):
        handles = wintypes.DWORD()
        if not kernel.GetProcessHandleCount(int(process._handle), ctypes.byref(handles)):
            raise ctypes.WinError(ctypes.get_last_error())
        snapshot = kernel.CreateToolhelp32Snapshot(4, 0)
        if snapshot == ctypes.c_void_p(-1).value:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            entry = ThreadEntry()
            entry.size = ctypes.sizeof(entry)
            okay = kernel.Thread32First(snapshot, ctypes.byref(entry))
            count = 0
            while okay:
                count += entry.process_id == process.pid
                okay = kernel.Thread32Next(snapshot, ctypes.byref(entry))
            if ctypes.get_last_error() != 18:
                raise ctypes.WinError(ctypes.get_last_error())
            return {"handles": handles.value, "threads": count, **memory(process)}
        finally:
            kernel.CloseHandle(snapshot)

    return sample, memory


def configuration(profile, seconds, cycle_ms, checkpoint_seconds, sample_seconds):
    if profile in {'8h', '24h'}:
        expected = 28800 if profile == '8h' else 86400
        if seconds is not None and seconds != expected:
            raise ValueError('长期档不得缩短连续时长')
        seconds = expected
        cycle_ms = 1000 if cycle_ms is None else cycle_ms
        if not 1000 <= cycle_ms <= 60000:
            raise ValueError('长期原始事件档的周期为1000..60000ms')
    elif profile == 'bounded':
        seconds = 180 if seconds is None else seconds
        cycle_ms = 0 if cycle_ms is None else cycle_ms
        if not 180 <= seconds <= 600:
            raise ValueError('旧有界档必须180..600秒')
    elif profile == 'diagnostic':
        seconds = 3 if seconds is None else seconds
        cycle_ms = 10 if cycle_ms is None else cycle_ms
        if not 1 <= seconds <= 60:
            raise ValueError('短档仅1..60秒，不是长期验收')
    else:
        raise ValueError('未知profile')
    if not 0 <= cycle_ms <= 60000 or not .05 <= checkpoint_seconds <= 300 or not .02 <= sample_seconds <= 60:
        raise ValueError('周期/检查点/采样参数超限')
    return {'profile': profile, 'seconds': seconds, 'cycle_ms': cycle_ms,
            'checkpoint_seconds': checkpoint_seconds, 'sample_seconds': sample_seconds,
            'long_profile': profile in {'8h', '24h'}}


def verify_identity(expected, actual):
    if expected != actual:
        raise ValueError('续跑源码/数据/二进制/工具/配置身份不一致')


class Journal:
    def __init__(self, folder):
        self.folder = folder
        self.lock = threading.Lock()
        self.streams = {name: (folder / (name + '.jsonl')).open('x', encoding='utf-8', newline='\n')
                        for name in ['events', 'samples', 'checkpoints']}

    def append(self, kind, value):
        with self.lock:
            self.streams[kind].write(json.dumps(value, ensure_ascii=False, separators=(',', ':')) + '\n')
            self.streams[kind].flush()

    def checkpoint(self, value):
        with self.lock:
            self.streams['checkpoints'].write(json.dumps(value, ensure_ascii=False) + '\n')
            for stream in self.streams.values():
                stream.flush()
                os.fsync(stream.fileno())
            temporary = self.folder / 'checkpoint.tmp'
            with temporary.open('w', encoding='utf-8', newline='\n') as output:
                json.dump(value, output, ensure_ascii=False, indent=2)
                output.write('\n')
                output.flush()
                os.fsync(output.fileno())
            os.replace(temporary, self.folder / 'checkpoint.json')

    def close(self):
        for stream in self.streams.values():
            stream.flush()
            os.fsync(stream.fileno())
            stream.close()


def _identity_from_handle(handle, pid):
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
    kernel.GetProcessTimes.restype = wintypes.BOOL
    values = [wintypes.FILETIME() for _ in range(4)]
    if not kernel.GetProcessTimes(handle, *(ctypes.byref(value) for value in values)):
        raise ctypes.WinError(ctypes.get_last_error())
    return {'pid': pid, 'created': values[0].dwLowDateTime | (values[0].dwHighDateTime << 32)}


def process_identity(process):
    return _identity_from_handle(int(process._handle), process.pid)


def process_status(identity):
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    kernel.WaitForSingleObject.restype = wintypes.DWORD
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    handle = kernel.OpenProcess(0x100000 | 0x1000, False, identity['pid'])
    if not handle:
        return 'dead' if ctypes.get_last_error() == 87 else 'unknown'
    try:
        current = _identity_from_handle(handle, identity['pid'])
        if current != identity:
            return 'reused'
        status = kernel.WaitForSingleObject(handle, 0)
        return 'dead' if status == 0 else 'alive' if status == 258 else 'unknown'
    except OSError:
        return 'unknown'
    finally:
        kernel.CloseHandle(handle)


def require_dead(identity):
    if identity is None or process_status(identity) not in {'dead', 'reused'}:
        raise ValueError('无法证明先前进程身份已退出，拒绝续跑且不终止该PID')


def latency_summary(path):
    admission, complete, errors = [], [], []
    trace = {'keys': 0, 'cycles': 0, 'finals': 0, 'identity': None}
    with path.open(encoding='utf-8', newline='') as source:
        for number, line in enumerate(source, 1):
            try:
                row = json.loads(line)
                if not isinstance(row, dict):
                    raise ValueError('事件必须为对象')
                if row.get('experiment') == 'r6-owned-key':
                    if any(type(row[name]) is not int or row[name] < 0 for name in ['admission_ns', 'commit_complete_ns']):
                        raise ValueError('延迟必须为非负整数ns')
                    admission.append(row['admission_ns'])
                    complete.append(row['commit_complete_ns'])
                audit_trace_row(row, trace)
            except (ValueError, KeyError, TypeError) as error:
                errors.append({'line': number, 'sha256': hashlib.sha256(line.encode('utf-8')).hexdigest(),
                               'error': str(error)})
    def quantiles(values):
        ordered = sorted(values)
        return {name: ordered[min(len(ordered) - 1, max(0, math.ceil(len(ordered) * fraction) - 1))]
                if ordered else None for name, fraction in [('p50', .5), ('p95', .95), ('p99', .99), ('max', 1)]}
    return {'events': len(admission), 'admission_ns': quantiles(admission),
            'commit_complete_ns': quantiles(complete), 'raw_errors': errors,
            'cycles': trace['cycles'], 'final_records': trace['finals'],
            'trace_complete': not errors and trace['keys'] > 0 and trace['finals'] == 1
                              and trace['keys'] == trace['cycles'] * 6,
            'external_com_reference_count': None,
            'scope': 'own diagnostic native FIFO; commit latency ends at cycle drain, not real TSF latency'}


def audit_trace_row(row, trace):
    kind = row.get('experiment')
    if kind == 'r6-owned-key':
        trace['keys'] += 1
        number = trace['keys']
        identity = (row['instance'], row['epoch'])
        if trace['identity'] is None:
            trace['identity'] = identity
        if (trace['finals'] or identity != trace['identity'] or row['sequence'] != number
                or row['cycle'] != (number - 1) // 6 + 1
                or row['vk'] != [78, 73, 72, 65, 79, 32][(number - 1) % 6]):
            raise ValueError('逐键序号/周期/会话/固定诊断键顺序不一致')
    elif kind == 'r6-cycle':
        trace['cycles'] += 1
        if (trace['finals'] or row['cycle'] != trace['cycles'] or trace['keys'] != trace['cycles'] * 6
                or row['healthy_writes'] != trace['keys'] or row['fifo_and_budget_verified'] is not True):
            raise ValueError('周期缺键/缺周期/FIFO或写入分母不一致')
    elif kind == 'r5-pinned-availability':
        trace['finals'] += 1
        if trace['keys'] or trace['cycles']:
            if (trace['finals'] != 1 or row['healthy_cycles'] != trace['cycles']
                    or row['healthy_writes'] != trace['keys'] or trace['keys'] != trace['cycles'] * 6
                    or row['faulted_owners'] != 32 or row['unknown_replays'] != 0
                    or row['remaining_hosts'] != 32 or row['remaining_batches'] != 32):
                raise ValueError('最终记录与原始逐键/周期/未知恢复分母不一致')


def environment_identity():
    return {'os': platform.platform(), 'architecture': platform.machine(), 'cpu_count': os.cpu_count(),
            'machine_id_sha256': hashlib.sha256(platform.node().encode('utf-8')).hexdigest(),
            'python_version': platform.python_version(),
            'python_executable_sha256': hashlib.sha256(Path(os.sys.executable).read_bytes()).hexdigest()}


def execution_preflight(identity, approval):
    required = identity['configuration']['long_profile']
    issues = []
    if required:
        if not isinstance(approval, dict):
            issues.append('缺少隔离Windows长测环境与执行授权记录')
        else:
            if approval.get('status') != 'approved' or approval.get('isolated_windows_environment') is not True:
                issues.append('隔离环境和执行批准未确认')
            if any(not isinstance(approval.get(name), str) or not approval[name].strip()
                   for name in ['environment_id', 'operator', 'snapshot', 'rollback_plan', 'authorization_reference']):
                issues.append('环境/负责人/快照/回滚/授权引用未齐')
            if approval.get('identity') != identity:
                issues.append('批准记录的源码/数据/工具/二进制/配置/系统身份不一致')
    return {'ready': not issues, 'long_run_approval_required': required, 'issues': issues,
            'identity': identity, 'declarations_independently_verified': False,
            'approval_sha256': hashlib.sha256(json.dumps(approval, sort_keys=True, ensure_ascii=False,
                                                       separators=(',', ':')).encode('utf-8')).hexdigest()
                               if approval is not None else None,
            'scope': 'owned diagnostic process only; no registration or real-host input authorization'}


def prepare_segment(out, identity, resume):
    manifest = out / 'manifest.json'
    if resume:
        verify_identity(json.loads(manifest.read_text(encoding='utf-8'))['identity'], identity)
        segments = sorted(out.glob('segment-*'))
        previous = json.loads((segments[-1] / 'checkpoint.json').read_text(encoding='utf-8'))
        require_dead(previous['runner'])
        require_dead(previous['child'])
        number = len(segments) + 1
    else:
        out.mkdir(parents=True, exist_ok=False)
        manifest.write_text(json.dumps({'schema_version': 2, 'run_id': str(uuid.uuid4()), 'identity': identity,
                                       'scope': 'separate segments never combine to claim contiguous duration'},
                                      ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
        number = 1
    segment = out / f'segment-{number:04}'
    segment.mkdir()
    return segment


def run_segment(segment, binary, config, interrupt_after, fail_cycle):
    command = [str(binary), TEST, '--exact', '--nocapture', '--test-threads=1']
    env = os.environ.copy()
    for key in ['CLOUDLEAF_RECOVERY_REVIEW_CHILD', 'CLOUDLEAF_REVIEW_PINNED_SECONDS',
                'CLOUDLEAF_REVIEW_SOAK_PROFILE', 'CLOUDLEAF_REVIEW_CYCLE_MS', 'CLOUDLEAF_REVIEW_FAIL_CYCLE']:
        env.pop(key, None)
    env.update(CLOUDLEAF_RECOVERY_REVIEW_CHILD=TEST, CLOUDLEAF_REVIEW_PINNED_SECONDS=str(config['seconds']),
               CLOUDLEAF_REVIEW_SOAK_PROFILE=config['profile'], CLOUDLEAF_REVIEW_CYCLE_MS=str(config['cycle_ms']))
    if fail_cycle:
        env['CLOUDLEAF_REVIEW_FAIL_CYCLE'] = str(fail_cycle)
    kernel = ctypes.WinDLL('kernel32')
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    owner = _identity_from_handle(kernel.GetCurrentProcess(), os.getpid())
    journal = Journal(segment)
    started = time.perf_counter()
    state = {'state': 'prepared', 'runner': owner, 'child': None, 'elapsed_seconds': 0}
    journal.checkpoint(state)
    sample, memory = collector()
    latest, errors, peaks = {}, [], {}
    samples, events = [], []
    process = None
    interrupted = timed_out = False
    reader_errors = []
    with (segment / 'stdout.txt').open('xb') as log:
        process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        state['child'] = process_identity(process)
        state['state'] = 'running'
        journal.checkpoint(state)

        def read_output():
            for raw in process.stdout:
                log.write(raw)
                log.flush()
                try:
                    line = raw.decode('utf-8')
                    offset = line.find('{"experiment":')
                    if offset >= 0:
                        event = json.loads(line[offset:])
                        journal.append('events', event)
                        latest[event['experiment']] = event
                        if config['profile'] == 'bounded':
                            events.append(event)
                except (UnicodeError, json.JSONDecodeError) as error:
                    reader_errors.append(str(error))

        reader = threading.Thread(target=read_output)
        reader.start()
        checkpoint_at = 0
        try:
            while process.poll() is None:
                elapsed = time.perf_counter() - started
                if interrupt_after and elapsed >= interrupt_after:
                    raise KeyboardInterrupt
                if elapsed > config['seconds'] + 60:
                    timed_out = True
                    process.kill()
                    break
                try:
                    row = {'elapsed_seconds': elapsed, **sample(process)}
                    journal.append('samples', row)
                    if config['profile'] == 'bounded':
                        samples.append(row)
                    for name in ['handles', 'threads', 'working_set_bytes', 'private_commit_bytes']:
                        peaks[name] = max(peaks.get(name, 0), row[name])
                except OSError as error:
                    errors.append({'elapsed_seconds': elapsed, 'error': str(error)})
                if elapsed >= checkpoint_at:
                    state.update(elapsed_seconds=elapsed, latest=dict(latest), sampling_errors=len(errors))
                    journal.checkpoint(state)
                    log.flush()
                    os.fsync(log.fileno())
                    checkpoint_at = elapsed + config['checkpoint_seconds']
                time.sleep(config['sample_seconds'])
        except KeyboardInterrupt:
            interrupted = True
            process.kill()
        except BaseException as error:
            errors.append({'stage': 'controller', 'error': repr(error)})
            process.kill()
        finally:
            code = process.wait()
            reader.join(timeout=10)
            if reader.is_alive():
                raise RuntimeError('自有子进程输出读取未结束，原始文件保留')
            try:
                final_memory = memory(process)
            except OSError as error:
                final_memory = None
                errors.append({'stage': 'retained handle after exit', 'error': str(error)})
            log.flush()
            os.fsync(log.fileno())
            state.update(state='interrupted' if interrupted else 'finished', exit_code=code,
                         elapsed_seconds=time.perf_counter() - started, latest=dict(latest))
            journal.checkpoint(state)
            journal.close()
    final = latest.get('r5-pinned-availability')
    latencies = latency_summary(segment / 'events.jsonl')
    passed = bool(code == 0 and not interrupted and not timed_out and not errors and not reader_errors
                  and not latencies['raw_errors'] and final and final['pinned_budget_unchanged']
                  and (config['profile'] == 'bounded' or latencies['trace_complete'])
                  and final['steady_elapsed_ns'] >= config['seconds'] * 1_000_000_000)
    summary = {'schema_version': 2, 'command': command, 'explicit_configuration': config,
               'runner': owner, 'child': state['child'], 'exit_code': code, 'interrupted': interrupted,
               'timeout': timed_out, 'wall_seconds': state['elapsed_seconds'], 'mechanism_passed': passed,
               'continuous_long_profile_completed': passed and config['long_profile'],
               'samples': samples if config['profile'] == 'bounded' else None,
               'events': events if config['profile'] == 'bounded' else None,
               'latest_events': latest, 'sampling_errors': errors, 'output_errors': reader_errors,
               'sampled_peak': peaks, 'final_memory': final_memory, 'latency': latencies,
               'recovery': {'expected_unknown_materials': 32, 'verified': 32 if passed else None,
                            'success_rate': 1.0 if passed else None, 'automatic_replays': 0 if passed else None},
               'external_com_reference_count': None, 'real_application_acceptance': False,
               'system_registration_executed': False, 'long_term_production_reliability_accepted': False,
               'files_sha256': {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                                for path in segment.iterdir() if path.is_file()},
               'limitations': ['only owned diagnostic process; raw sampling peaks are not exact RSS bounds',
                               'new segments restart independent sessions; elapsed time never joins across segments',
                               'Rc/Lease counts and paired own-IUnknown returns are separate from global COM reference count']}
    (segment / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps({'segment': str(segment), 'exit_code': code, 'interrupted': interrupted,
                      'mechanism_passed': passed, 'continuous_long_profile_completed': summary['continuous_long_profile_completed']}), flush=True)
    return 130 if interrupted else 0 if passed else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--test-binary', type=Path, required=True)
    parser.add_argument('--profile', choices=['bounded', 'diagnostic', '8h', '24h'], default='bounded')
    parser.add_argument('--seconds', type=int)
    parser.add_argument('--cycle-ms', type=int)
    parser.add_argument('--sample-seconds', type=float, default=.2)
    parser.add_argument('--checkpoint-seconds', type=float, default=5)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--resume', action='store_true')
    parser.add_argument('--interrupt-after', type=float)
    parser.add_argument('--inject-failure-cycle', type=int)
    parser.add_argument('--environment-approval', type=Path)
    parser.add_argument('--preflight-output', type=Path)
    args = parser.parse_args()
    if os.name != 'nt':
        parser.error('仅Windows自有诊断进程，不注册/激活输入法')
    try:
        config = configuration(args.profile, args.seconds, args.cycle_ms, args.checkpoint_seconds, args.sample_seconds)
        if (args.interrupt_after or args.inject_failure_cycle) and args.profile != 'diagnostic':
            raise ValueError('受控中断/失败注入仅允许显式短诊断档')
        binary = args.test_binary.resolve(strict=True)
        identity = {'implementation_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                    'source_sha256': source_sha256(), 'source_fingerprint_schema': 'cloudleaf-source-v3',
                    'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                    'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                    'data_sha256': hashlib.sha256((ROOT / 'data/dictionaries/prototype.tsv').read_bytes()).hexdigest(),
                    'configuration': config, 'os': platform.platform(), 'environment': environment_identity()}
        approval = json.loads(args.environment_approval.read_text(encoding='utf-8')) if args.environment_approval else None
        preflight = execution_preflight(identity, approval)
        if args.preflight_output:
            with args.preflight_output.open('x', encoding='utf-8', newline='\n') as output:
                json.dump(preflight, output, ensure_ascii=False, indent=2)
                output.write('\n')
            print(json.dumps({'ready': preflight['ready'], 'child_started': False,
                              'report': str(args.preflight_output)}), flush=True)
            return 0 if preflight['ready'] else 2
        if not preflight['ready']:
            raise ValueError('; '.join(preflight['issues']))
        identity['execution_approval_sha256'] = preflight['approval_sha256']
        segment = prepare_segment(args.output_dir.resolve(), identity, args.resume)
    except (ValueError, OSError) as error:
        parser.error(str(error))
    return run_segment(segment, binary, config, args.interrupt_after, args.inject_failure_cycle)


if __name__ == '__main__':
    raise SystemExit(main())
