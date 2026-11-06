"""真实固定词典输入轨迹与整进程采样，保持全部候选失真和失败。"""
from pathlib import Path
import ctypes
from ctypes import wintypes
import hashlib
import json
import math
import subprocess
import sys
import time
root=Path(__file__).resolve().parents[1]
out=root/'.local'/sys.argv[1];out.mkdir(exist_ok=False)
variant=sys.argv[2]
exe=root/('.local/round17-baseline-build/release/examples/input_trace.exe' if variant.startswith('original') else '.local/build/release/examples/input_trace.exe')
receipt=json.loads((root/'.local/round14-lexicon-receipt.json').read_text(encoding='utf-8'))
runtime=receipt['runtime']['path']
command=[str(exe),runtime,receipt['runtime']['sha256']]
model=root/'.local/round15-language/signal.tsv'
if variant in ['lexical','originallexical']:command.extend([str(model),hashlib.sha256(model.read_bytes()).hexdigest(),'lexical'])
if variant in ['character','originalcharacter','witten','originalwitten']:
    model=root/('.local/round16-language/character-witten.tsv' if 'witten' in variant else '.local/round16-language/character.tsv')
    command.extend([str(model),hashlib.sha256(model.read_bytes()).hexdigest(),'character_bigram'])

events=(root/(sys.argv[3] if len(sys.argv)>3 else 'tests/fixtures/input-round14-events.tsv')).read_bytes()
class Memory(ctypes.Structure):
    _fields_=[('cb',wintypes.DWORD),('faults',wintypes.DWORD)]+[(n,ctypes.c_size_t) for n in ['peak','working','peak_paged','paged','peak_nonpaged','nonpaged','pagefile','peak_pagefile','private']]
api=ctypes.WinDLL('psapi',use_last_error=True)
api.GetProcessMemoryInfo.argtypes=[wintypes.HANDLE,ctypes.POINTER(Memory),wintypes.DWORD]
started=time.perf_counter()
with (out/'stdout.tsv').open('wb') as stdout,(out/'stderr.txt').open('wb') as stderr:
    process=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
    process.stdin.write(events);process.stdin.close();samples=[]
    while process.poll() is None:
        value=Memory();value.cb=ctypes.sizeof(value)
        if api.GetProcessMemoryInfo(int(process._handle),ctypes.byref(value),value.cb):
            samples.append({'seconds':time.perf_counter()-started,'working':value.working,'private':value.private,'os_peak_working':value.peak,'os_peak_pagefile':value.peak_pagefile})
        time.sleep(.05)
    code=process.wait()
kernel=ctypes.WinDLL('kernel32',use_last_error=True)
kernel.GetProcessTimes.argtypes=[wintypes.HANDLE]+[ctypes.POINTER(wintypes.FILETIME)]*4
clock=[wintypes.FILETIME() for _ in range(4)]
cpu_ok=kernel.GetProcessTimes(int(process._handle),*[ctypes.byref(value) for value in clock])
cpu_seconds=sum((value.dwHighDateTime<<32)|value.dwLowDateTime for value in clock[2:])/10_000_000 if cpu_ok else None
raw=(out/'stdout.tsv').read_text(encoding='utf-8').splitlines()
times=[int(line.split('\t')[3]) for line in raw if line.startswith('EVENT\t')]
ranked=sorted(times)
result={'command':command,'returncode':code,'seconds':time.perf_counter()-started,
    'variant':variant,'whole_process_cpu_seconds':cpu_seconds,'original_implementation':'521472db18ecd7926f6e5b58b0afd7b5da674a07' if variant.startswith('original') else None,
    'worktree_diff_sha256':hashlib.sha256(subprocess.check_output(['git','diff','--binary','HEAD'],cwd=root)).hexdigest(),
    'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),
    'executable_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'events_sha256':hashlib.sha256(events).hexdigest(),
    'tool_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'runtime':receipt['runtime'],
    'sample_count':len(samples),'resources':samples,'resource_scope':'whole CLI process including fixed lexicon, strings, trie, search and runtime; no single-algorithm memory claim',
    'event_latency_ns':{'n':len(times),'p50':ranked[math.ceil(len(times)*.5)-1],'p95':ranked[math.ceil(len(times)*.95)-1],'p99':ranked[math.ceil(len(times)*.99)-1],'max':max(times)},
    'raw_events_in_latency':True,'timing_scope':'InputEngine.handle including frame clone and deterministic search; excludes stdout write, platform/UI and disk; heterogeneous engineering events, not formal latency acceptance',
    'summary':[line for line in raw if line.startswith('SUMMARY\t')],
    'formal_language_metrics':{'top1':None,'top5':None,'mrr':None},'physical_disconnect':'not-run','TSF':False,
    'files':{n:hashlib.sha256((out/n).read_bytes()).hexdigest() for n in ['stdout.tsv','stderr.txt']}}
(out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in result.items() if k!='resources'},ensure_ascii=False))
assert code==0
assert len(result['summary'])==1
