from pathlib import Path
import ctypes,hashlib,json,math,subprocess,time
from ctypes import wintypes
root=Path(__file__).resolve().parents[1]
out=root/'.local/round17-host-benchmark';out.mkdir(exist_ok=False)
base=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
variants=[('lexical',base,root/'.local/round15-language/signal.tsv'),('addone',base,root/'.local/round16-language/character.tsv'),('witten',base,root/'.local/round16-language/character-witten.tsv'),('supplement-addone',root/'.local/round17-supplement-a/combined-runtime.tsv',root/'.local/round17-supplement-a/model-0.tsv')]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
class Memory(ctypes.Structure):
    _fields_=[('cb',wintypes.DWORD),('faults',wintypes.DWORD)]+[(n,ctypes.c_size_t) for n in ['peak','working','peak_paged','paged','peak_nonpaged','nonpaged','pagefile','peak_pagefile','private']]
api=ctypes.WinDLL('psapi',use_last_error=True);api.GetProcessMemoryInfo.argtypes=[wintypes.HANDLE,ctypes.POINTER(Memory),wintypes.DWORD]
exe=root/'.local/build/release/cloudleaf-ime.exe';events=root/'tests/fixtures/input-round14-events.tsv'
records=[]
for iteration in range(1,4):
    for kind,lex,model in variants[iteration%4:]+variants[:iteration%4]:
        folder=out/f'{iteration}-{kind}';folder.mkdir()
        command=[str(exe),'--benchmark-input-host',str(lex),sha(lex),str(model),sha(model),str(events)]
        samples=[];start=time.perf_counter()
        with (folder/'stdout.txt').open('wb') as stdout,(folder/'stderr.txt').open('wb') as stderr:
            p=subprocess.Popen(command,cwd=root,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
            while p.poll() is None:
                value=Memory();value.cb=ctypes.sizeof(value)
                if api.GetProcessMemoryInfo(int(p._handle),ctypes.byref(value),value.cb):samples.append({'t':time.perf_counter()-start,'working':value.working,'private':value.private,'peak_working':value.peak})
                time.sleep(.05)
            code=p.wait()
        timing=[]
        for line in (folder/'stdout.txt').read_text(encoding='utf-8').splitlines():
            if line.startswith('HOST_BENCH\t'):
                fields=line.split('\t');timing.append({f.split('=')[0]:int(f.split('=')[1]) for f in fields[4:10]})
        record={'iteration':iteration,'kind':kind,'command':command,'returncode':code,'seconds':time.perf_counter()-start,'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),'binary_sha256':sha(exe),'events_sha256':sha(events),'resource_scope':'whole host CLI process, not per-algorithm allocation','samples':samples,'timings_ns':timing,'stdout_sha256':sha(folder/'stdout.txt'),'stderr_sha256':sha(folder/'stderr.txt')}
        (folder/'result.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8');records.append(record)
        print(json.dumps({'kind':kind,'iteration':iteration,'returncode':code,'events':len(timing)}),flush=True)
summary={'scope':'Actual Host with two engines, K5, all171 events/run; no HWND, memory-only sink. No discarded warmup. Same process budget, no nativeCOM/TSF.','records':records,'formal_language_metrics':None,'variants':{}}
for kind,_,_ in variants:
    selected=[r for r in records if r['kind']==kind]; times=[t for r in selected for t in r['timings_ns']]
    result={'runs':len(selected),'returncodes':[r['returncode'] for r in selected],'n':len(times),'metrics_ns':{}}
    for field in ['core','projection','compare','transaction','frame','total']:
        values=sorted(t[field] for t in times)
        if values:result['metrics_ns'][field]={**{f'p{p}':values[math.ceil(len(values)*p/100)-1] for p in [50,95,99]},'max':max(values)}
    summary['variants'][kind]=result
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print(json.dumps(summary['variants'],indent=2))
