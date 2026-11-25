"""执行显式研发样本并保留真实输出及独立进程资源采样，不称为正式质量。"""
from pathlib import Path
import argparse
import ctypes
from ctypes import wintypes
from datetime import datetime, timezone
import hashlib
import json
import os
import subprocess
import time

class Memory(ctypes.Structure):
    _fields_=[('cb',wintypes.DWORD),('PageFaultCount',wintypes.DWORD)]+[(name,ctypes.c_size_t) for name in ['PeakWorkingSetSize','WorkingSetSize','QuotaPeakPagedPoolUsage','QuotaPagedPoolUsage','QuotaPeakNonPagedPoolUsage','QuotaNonPagedPoolUsage','PagefileUsage','PeakPagefileUsage','PrivateUsage']]

class ThreadEntry(ctypes.Structure):
    _fields_=[('size',wintypes.DWORD),('usage',wintypes.DWORD),('thread_id',wintypes.DWORD),('owner_pid',wintypes.DWORD),('base',wintypes.LONG),('delta',wintypes.LONG),('flags',wintypes.DWORD)]

kernel=ctypes.WinDLL('kernel32',use_last_error=True)
kernel.CreateToolhelp32Snapshot.argtypes=[wintypes.DWORD,wintypes.DWORD]
kernel.CreateToolhelp32Snapshot.restype=wintypes.HANDLE
kernel.Thread32First.argtypes=[wintypes.HANDLE,ctypes.POINTER(ThreadEntry)]
kernel.Thread32Next.argtypes=[wintypes.HANDLE,ctypes.POINTER(ThreadEntry)]
kernel.CloseHandle.argtypes=[wintypes.HANDLE]
kernel.GetProcessTimes.argtypes=[wintypes.HANDLE]+[ctypes.POINTER(wintypes.FILETIME)]*4
kernel.GetProcessHandleCount.argtypes=[wintypes.HANDLE,ctypes.POINTER(wintypes.DWORD)]

def counters(process):
    handle=wintypes.DWORD()
    handles=handle.value if kernel.GetProcessHandleCount(int(process._handle),ctypes.byref(handle)) else None
    if handles is not None:handles=handle.value
    values=[wintypes.FILETIME() for _ in range(4)]
    cpu=None
    if kernel.GetProcessTimes(int(process._handle),*[ctypes.byref(value) for value in values]):
        cpu=sum((value.dwHighDateTime<<32)+value.dwLowDateTime for value in values[2:])/10000000
    snapshot=kernel.CreateToolhelp32Snapshot(4,0)
    threads=None
    if snapshot!=ctypes.c_void_p(-1).value:
        try:
            row=ThreadEntry();row.size=ctypes.sizeof(row)
            valid=kernel.Thread32First(snapshot,ctypes.byref(row));threads=0
            while valid:
                threads+=row.owner_pid==process.pid
                valid=kernel.Thread32Next(snapshot,ctypes.byref(row))
        finally:kernel.CloseHandle(snapshot)
    return {'cpu_total_seconds':cpu,'threads':threads,'handles':handles}

parser=argparse.ArgumentParser()
parser.add_argument('--label',required=True)
parser.add_argument('--catalog',required=True)
parser.add_argument('--binary',default='.local/round13-build/release/examples/offline_translate.exe')
parser.add_argument('--samples',default='tests/fixtures/translation-offline-smoke.jsonl')
args=parser.parse_args()
root=Path(__file__).resolve().parents[1]
exe=root/args.binary
directory=root/'.local/round13-inference'/args.label
directory.mkdir(parents=True,exist_ok=False)
command=[str(exe),str(Path(args.catalog).resolve())]
started=time.perf_counter()
with (root/args.samples).open('rb') as stdin,(directory/'stdout.jsonl').open('wb') as stdout,(directory/'stderr.txt').open('wb') as stderr:
    process=subprocess.Popen(command,cwd=root,stdin=stdin,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
    psapi=ctypes.WinDLL('psapi',use_last_error=True)
    psapi.GetProcessMemoryInfo.argtypes=[wintypes.HANDLE,ctypes.c_void_p,wintypes.DWORD]
    resource=[]
    while process.poll() is None:
        memory=Memory();memory.cb=ctypes.sizeof(memory)
        if psapi.GetProcessMemoryInfo(int(process._handle),ctypes.byref(memory),memory.cb):
            resource.append({'elapsed_seconds':time.perf_counter()-started,'working_set':memory.WorkingSetSize,'private_commit':memory.PrivateUsage,'peak_working_set':memory.PeakWorkingSetSize,'peak_pagefile':memory.PeakPagefileUsage}|counters(process))
        time.sleep(.1)
    code=process.wait()
record={'at_utc':datetime.now(timezone.utc).isoformat(),'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),
        'command':command,'returncode':code,'seconds':time.perf_counter()-started,'pid':process.pid,
        'executable_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'samples_sha256':hashlib.sha256((root/args.samples).read_bytes()).hexdigest(),
        'catalog_sha256':hashlib.sha256(Path(args.catalog).read_bytes()).hexdigest(),'tool_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'resource_scope':'whole isolated process containing two models, runtime and executable; not individual model memory',
        'network_environment':'host remains connected; no physical disconnect claim',
        'samples':len(resource),'resource_samples':resource,'exit_counters':counters(process),
        'files':{name:hashlib.sha256((directory/name).read_bytes()).hexdigest() for name in ['stdout.jsonl','stderr.txt']}}
(directory/'result.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({key:value for key,value in record.items() if key!='resource_samples'},ensure_ascii=False))
print((directory/'stdout.jsonl').read_text(encoding='utf-8'))
print((directory/'stderr.txt').read_text(encoding='utf-8'))
raise SystemExit(code)
