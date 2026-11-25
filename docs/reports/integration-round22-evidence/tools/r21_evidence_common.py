"""Round 21 本机证据工具。仅运行显式的无窗口程序。"""

import ctypes
import gzip
import hashlib
import json
import math
import os
import shutil
import subprocess
import time
from ctypes import wintypes
from datetime import UTC, datetime
from pathlib import Path

LOCAL = Path(__file__).resolve().parent
ROOT = LOCAL.parent
BASE = ROOT.parents[1] / 'input-round14/Cloudleaf'
TRANSLATION = ROOT.parents[1] / 'translation-round11/Cloudleaf'
R20 = '714f8c9439148b23d2ce34dc3a4e672d59598c1d'
R13 = 'cca77457ca3943ee5ad90d566a8d9ef6b8795cd2'


def now():
    return datetime.now(UTC).isoformat()


def identity(path):
    path = Path(path)
    with path.open('rb') as source:
        sha = hashlib.file_digest(source, 'sha256').hexdigest()
    return {'path': str(path.resolve()), 'bytes': path.stat().st_size, 'sha256': sha}


def get_json(path):
    path = Path(path)
    if not path.is_file():
        with gzip.open(path.with_suffix(path.suffix + '.gz'), 'rt', encoding='utf-8') as src:
            return json.load(src)
    return json.loads(path.read_text(encoding='utf-8'))


def put_json(path, record):
    Path(path).write_text(json.dumps(record, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)


def head():
    return git('rev-parse', 'HEAD').decode().strip()


def source_hashes(paths):
    return {str(path.relative_to(ROOT)).replace('\\', '/'): identity(path)['sha256']
            for path in paths if path.is_file()}


def env_for_tool():
    env = os.environ.copy()
    env.update(PYTHONUTF8='1', PYTHONIOENCODING='utf-8')
    env['PATH'] = str(Path.home() / '.cargo/bin') + os.pathsep + env['PATH']
    return env


def compress(path):
    packed = path.with_suffix(path.suffix + '.gz')
    with path.open('rb') as src, packed.open('xb') as raw:
        with gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0, compresslevel=5) as out:
            while data := src.read(1_048_576):
                out.write(data)
    with gzip.open(packed, 'rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    orig = identity(path)
    assert digest == orig['sha256']
    return {'original': orig, 'gzip': identity(packed)}


def percentile(values):
    values = sorted(values)
    if not values:
        return {'n': 0}
    return {'n': len(values), **{f'p{p}': values[math.ceil(p * len(values) / 100) - 1]
                                for p in [50, 95, 99]}, 'max': values[-1]}


class ProcessMemory(ctypes.Structure):
    _fields_ = [('cb', wintypes.DWORD), ('faults', wintypes.DWORD)] + [
        (name, ctypes.c_size_t) for name in (
            'peak', 'working', 'peak_paged', 'paged', 'peak_nonpaged', 'nonpaged',
            'pagefile', 'peak_pagefile', 'private',
        )
    ]


get_memory = ctypes.WinDLL('psapi', use_last_error=True).GetProcessMemoryInfo
get_memory.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessMemory), wintypes.DWORD]
get_times = ctypes.WinDLL('kernel32', use_last_error=True).GetProcessTimes
get_times.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
get_handles = ctypes.WinDLL('kernel32', use_last_error=True).GetProcessHandleCount
get_handles.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]


def run(folder, command, *, payload=None, cwd=ROOT, deadline_seconds=180,
        sample=True, expected=0):
    folder.mkdir(parents=True, exist_ok=False)
    stdout, stderr = folder / 'stdout.txt', folder / 'stderr.txt'
    resource = []
    started_utc = now()
    start = time.perf_counter()
    timed_out = False
    args = [str(s) for s in command]
    env = env_for_tool()
    args[0] = shutil.which(args[0], path=env['PATH']) or args[0]
    with stdout.open('xb') as out, stderr.open('xb') as err:
        process = subprocess.Popen(args, cwd=cwd, stdin=subprocess.PIPE,
                                   stdout=out, stderr=err, env=env,
                                   creationflags=subprocess.CREATE_NO_WINDOW)
        if payload:
            process.stdin.write(payload)
        process.stdin.close()
        while process.poll() is None:
            elapsed = time.perf_counter() - start
            if elapsed >= deadline_seconds:
                timed_out = True
                process.kill()
                break
            row = ProcessMemory()
            row.cb = ctypes.sizeof(row)
            handle = int(process._handle)
            if sample and get_memory(handle, ctypes.byref(row), row.cb):
                times = [wintypes.FILETIME() for _ in range(4)]
                cpu = None
                if get_times(handle, *(ctypes.byref(t) for t in times)):
                    cpu = sum((t.dwHighDateTime << 32) + t.dwLowDateTime for t in times[2:]) / 1e7
                handles = wintypes.DWORD()
                resource.append({'offset_s': elapsed, 'working_bytes': row.working,
                                 'peak_working_bytes': row.peak, 'private_bytes': row.private,
                                 'cpu_seconds': cpu,
                                 'handles': handles.value if get_handles(handle, ctypes.byref(handles)) else None})
            time.sleep(.05 if sample else .2)
        code = process.wait(timeout=15)
    record = {'started_utc': started_utc, 'seconds': time.perf_counter()-start, 'command': args,
              'cwd': str(cwd), 'head': head(), 'harness_timeout': timed_out, 'pid': process.pid,
              'returncode': code, 'expected_returncode': expected,
              'stdout': identity(stdout), 'stderr': identity(stderr), 'memory': resource,
              'tool': identity(__file__),
              'input_sha256': hashlib.sha256(payload).hexdigest() if payload is not None else None}
    put_json(folder / 'result.json', record)
    print(json.dumps({'run': folder.name, 'code': code, 'expected': expected,
                      'seconds': record['seconds'], 'samples': len(resource)}), flush=True)
    if timed_out or (expected is not None and code != expected):
        raise RuntimeError(f'{folder}: expected {expected}, got {code}')
    return record
