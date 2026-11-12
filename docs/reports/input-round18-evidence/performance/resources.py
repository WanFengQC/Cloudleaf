"""50ms采样整进程，不把working/private解释为单算法分配。"""
from pathlib import Path
from ctypes import wintypes
from datetime import datetime,timezone
import ctypes,hashlib,json,subprocess,time
out=Path(__file__).resolve().parent;root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
record=json.loads((out/'summary.json').read_text(encoding='utf-8'));dest=out/'resources';dest.mkdir(exist_ok=False)
class Memory(ctypes.Structure):
    _fields_=[('cb',wintypes.DWORD),('faults',wintypes.DWORD)]+[(name,ctypes.c_size_t)for name in ['peak','working','peak_paged','paged','peak_nonpaged','nonpaged','pagefile','peak_pagefile','private']]
api=ctypes.WinDLL('psapi',use_last_error=True);api.GetProcessMemoryInfo.argtypes=[wintypes.HANDLE,ctypes.POINTER(Memory),wintypes.DWORD]
rows=[]
for scheme in ['full','abbreviated','xiaohe','fuzzy-nl','correction']:
    payload=(out/(scheme+'-cases.tsv')).read_bytes()
    for kind,model in record['models'].items():
        folder=dest/(scheme+'-'+kind);folder.mkdir()
        command=[record['binary']['scheme']['path'],record['runtime']['path'],record['runtime']['sha256'],scheme,'engine',model['path'],model['sha256']]
        start=time.perf_counter();samples=[]
        with (folder/'stdout.txt').open('wb')as stdout,(folder/'stderr.txt').open('wb')as stderr:
            child=subprocess.Popen(command,cwd=root,stdin=subprocess.PIPE,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
            child.stdin.write(payload);child.stdin.close()
            while child.poll()is None:
                memory=Memory();memory.cb=ctypes.sizeof(memory)
                if api.GetProcessMemoryInfo(int(child._handle),ctypes.byref(memory),memory.cb):samples.append({'seconds':time.perf_counter()-start,'working':memory.working,'private':memory.private,'OS_peak_working':memory.peak,'OS_peak_pagefile':memory.peak_pagefile})
                time.sleep(.05)
            code=child.wait()
        row={'scheme':scheme,'model':kind,'command':command,'input_sha256':hashlib.sha256(payload).hexdigest(),'returncode':code,'samples':samples,'sample_count':len(samples),'seconds':time.perf_counter()-start,'sampled_working_max':max(v['working']for v in samples),'sampled_private_max':max(v['private']for v in samples),'OS_peak_working_max':max(v['OS_peak_working']for v in samples),'stdout_sha256':hashlib.sha256((folder/'stdout.txt').read_bytes()).hexdigest(),'stderr_sha256':hashlib.sha256((folder/'stderr.txt').read_bytes()).hexdigest()}
        rows.append(row);(folder/'result.json').write_text(json.dumps(row,indent=2)+'\n',encoding='utf-8');print(json.dumps({k:v for k,v in row.items()if k not in ['samples','command']},ensure_ascii=False),flush=True)
summary={'scope':'Each fresh single-engine K10 scheme_trace process includes immutable lexicon/model/read buffers/trie/search/frame/diagnostic. 50ms sampling and OS peak counters; no single-algorithm/Host/COM memory claim, no allocator-live-byte proof. Separate resource run, not included in original latency sample.','at_utc':datetime.now(timezone.utc).isoformat(),'binary':record['binary']['scheme'],'runtime':record['runtime'],'models':record['models'],'tool_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'runs':rows}
(out/'resources-summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
