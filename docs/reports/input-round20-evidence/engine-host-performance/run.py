"""固定R19/R20二进制的默认Engine配对和无窗口Host测量；不写实际仓库。"""
from pathlib import Path
from datetime import datetime, timezone
import ctypes
from ctypes import wintypes
import gzip
import hashlib
import json
import math
import platform
import re
import subprocess
import sys
import time
import zipfile

ROOT=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
OUT=Path(__file__).resolve().parent
RUNS=OUT/'runs';RUNS.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def identity(p):
    p=Path(p);return {'path':str(p),'bytes':p.stat().st_size,'sha256':sha(p)}
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT).decode().strip()
def write(name,value):(OUT/name).write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
def pct(values):
    v=sorted(values)
    return {'n':len(v),**{f'p{p}':v[math.ceil(len(v)*p/100)-1]for p in [50,95,99]},'max':max(v)} if v else {'n':0}

downloads=json.loads((ROOT/'.local/round19-ci/final-downloads/downloads.json').read_text(encoding='utf-8'))
assert downloads['head']=='4d42363091361552ad525f47e3c6b1d581ae3a4c'
artifact=next(v for v in downloads['files']if v['file']=='input-round19-windows.zip')
archive=ROOT/'.local/round19-ci/final-downloads'/artifact['file']
assert sha(archive)==artifact['sha256']
with zipfile.ZipFile(archive) as z:
    assert z.testzip() is None
    members=[v for v in artifact['members']if v['name'].endswith('input_trace.exe')]
    assert len(members)==1
    member=members[0];raw=z.read(member['name']);assert hashlib.sha256(raw).hexdigest()==member['sha256']
    baseline=OUT/'baseline-input_trace.exe';baseline.write_bytes(raw)

runtime=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
assert sha(runtime)=='03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
models={'lexical':ROOT/'.local/round15-language/signal.tsv','addone':ROOT/'.local/round16-language/character.tsv','witten':ROOT/'.local/round16-language/character-witten.tsv'}
binary={'old':baseline,'new':ROOT/'.local/round20-build/release/examples/input_trace.exe','host':ROOT/'.local/round20-build/release/cloudleaf-ime.exe','old-host':ROOT/'.local/round20-baseline/cloudleaf-ime.exe'}
events=ROOT/'tests/fixtures/input-round14-events.tsv';payload=events.read_bytes();(OUT/'events.tsv').write_bytes(payload)
sources=[p for area in ['crates/cloudleaf-search/src','crates/cloudleaf-pinyin/src','crates/cloudleaf-input/src','crates/cloudleaf-user/src']for p in sorted((ROOT/area).glob('*.rs'))]
sources += [ROOT/'crates/cloudleaf-input/examples/input_trace.rs',ROOT/'adapters/windows/isolated_input.rs',ROOT/'adapters/windows/session.rs',ROOT/'Cargo.lock']
def source_identity():return {str(p.relative_to(ROOT)).replace('\\','/'):sha(p)for p in sources}
before=source_identity()
record={'started_utc':datetime.now(timezone.utc).isoformat(),'head':git('rev-parse','HEAD'),'old_head':downloads['head'],'source_before':before,'tool':identity(__file__),'artifact':{'archive':identity(archive),'CRC':'passed','CI_download_record_sha256':sha(ROOT/'.local/round19-ci/final-downloads/downloads.json'),'member':member},'binaries':{k:identity(p)for k,p in binary.items()},'data':{'runtime':identity(runtime),'events':identity(events),'models':{k:identity(p)for k,p in models.items()}},'scope':'Default Engine K10, 171 events per process, 5 fresh processes/model/version, same desktop and inputs; all warmup/tails retained. Timing only Rust engine.handle. Host separately paired old/new K5 two engines no HWND, memory sink. No user store, GUI, TSF or formal quality. Ordinary shared Windows desktop, not controlled dedicated latency acceptance.','formal_metrics':{'top1':None,'top5':None,'mrr':None},'environment':{'platform':platform.platform(),'python':sys.version},'runs':[]}
write('summary.json',record)

class Memory(ctypes.Structure):
    _fields_=[('cb',wintypes.DWORD),('faults',wintypes.DWORD)]+[(n,ctypes.c_size_t)for n in ['peak','working','peak_paged','paged','peak_nonpaged','nonpaged','pagefile','peak_pagefile','private']]
api=ctypes.WinDLL('psapi',use_last_error=True)
api.GetProcessMemoryInfo.argtypes=[wintypes.HANDLE,ctypes.POINTER(Memory),wintypes.DWORD]

def run(label,command,input_bytes):
    folder=RUNS/label;folder.mkdir()
    samples=[];start=time.perf_counter()
    with (folder/'stdout.txt').open('wb')as stdout,(folder/'stderr.txt').open('wb')as stderr:
        p=subprocess.Popen(command,cwd=ROOT,stdin=subprocess.PIPE,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
        if input_bytes is not None:p.stdin.write(input_bytes)
        p.stdin.close()
        while p.poll()is None:
            m=Memory();m.cb=ctypes.sizeof(m)
            if api.GetProcessMemoryInfo(int(p._handle),ctypes.byref(m),m.cb):samples.append({'t':time.perf_counter()-start,'working':m.working,'private':m.private,'OS_peak_working':m.peak})
            time.sleep(.05)
        code=p.wait()
    row={'label':label,'command':command,'returncode':code,'seconds':time.perf_counter()-start,'input_sha256':hashlib.sha256(input_bytes).hexdigest()if input_bytes is not None else None,'stdout':identity(folder/'stdout.txt'),'stderr':identity(folder/'stderr.txt'),'resource_scope':'Entire child process incl preload/raw data/buffers/index/model/diagnostics; not single-algorithm allocation or Host internal budget','resource_samples':samples}
    (folder/'result.json').write_text(json.dumps(row,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    record['runs'].append(row);write('summary.json',record)
    print(json.dumps({'label':label,'returncode':code,'seconds':row['seconds']}),flush=True)
    assert code==0,label
    return (folder/'stdout.txt').read_text(encoding='utf-8').splitlines()

def semantic(lines):
    selected=[]
    for line in lines:
        if line.startswith('EVENT\t'):
            fields=line.split('\t');selected.append('\t'.join(fields[:3]+fields[4:]))
        elif line.startswith(('FRAME\t','CANDIDATE\t','COMMIT\t','SEARCH\t','PATH\t','SUMMARY\t')):
            values=re.findall(r'user_preference_units: (\d+)',line)
            assert all(v=='0'for v in values),'Default OFF produced nonzero user preference'
            selected.append(line)
    return selected

engine_times={f'{version}-{kind}':[]for version in ['old','new']for kind in models};pairs=[]
for iteration in range(1,6):
    order=list(models);order=order[iteration%3:]+order[:iteration%3]
    for kind in order:
        outputs={}
        for variant in (['old','new']if iteration%2 else ['new','old']):
            model=models[kind]
            command=[str(binary[variant]),str(runtime),sha(runtime),str(model),sha(model),'lexical'if kind=='lexical'else'character_bigram']
            lines=run(f'engine-{iteration}-{kind}-{variant}',command,payload)
            timings=[int(l.split('\t')[3])for l in lines if l.startswith('EVENT\t')]
            assert len(timings)==171,(kind,variant,len(timings))
            engine_times[f'{variant}-{kind}'].extend(timings);outputs[variant]=lines
        old=semantic(outputs['old']);new=semantic(outputs['new']);same=old==new
        pair={'iteration':iteration,'model':kind,'same':same,'semantic_rows_old':len(old),'semantic_rows_new':len(new),'normalized_fields':'Only EVENT timing column; candidate, source, preference, and statistics fields compared verbatim'}
        if not same:
            (OUT/f'mismatch-{iteration}-{kind}.json').write_text(json.dumps({'old':old,'new':new},ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
        pairs.append(pair)
record['engine_pairs']=pairs;record['engine_performance_ns']={k:pct(v)for k,v in engine_times.items()};write('summary.json',record)

variants=[('lexical',runtime,models['lexical']),('addone',runtime,models['addone']),('witten',runtime,models['witten']),('supplement-addone',ROOT/'.local/round17-supplement-a/combined-runtime.tsv',ROOT/'.local/round17-supplement-a/model-0.tsv')]
def host_semantics(lines):
    selected=[]
    for line in lines:
        if line.startswith('HOST_BENCH\t'):
            fields=line.split('\t')
            selected.append('\t'.join(fields[:4]+fields[10:]))
        elif line.startswith(('HOST_FRAME\t','HOST_SEARCH\t','HOST_SUMMARY ')):
            selected.append(line)
    return selected

host_times={};host_runs=[];host_pairs=[]
for iteration in range(1,4):
    for kind,lex,model in variants[iteration%4:]+variants[:iteration%4]:
        outputs={}
        for variant in (['old','new'] if iteration%2 else ['new','old']):
            key=variant+'-'+kind
            command=[str(binary['old-host' if variant=='old' else 'host']),'--benchmark-input-host',str(lex),sha(lex),str(model),sha(model),str(events)]
            lines=run(f'host-{iteration}-{key}',command,None)
            outputs[variant]=host_semantics(lines)
            timings=[{f.split('=')[0]:int(f.split('=')[1])for f in l.split('\t')[4:10]}for l in lines if l.startswith('HOST_BENCH\t')]
            assert len(timings)==171,(kind,variant,len(timings))
            host_times.setdefault(key,[]).extend(timings);host_runs.append({'iteration':iteration,'model':kind,'variant':variant,'events':len(timings),'command':command,'dictionary':identity(lex),'model_file':identity(model)})
        host_pairs.append({'iteration':iteration,'model':kind,'same':outputs['old']==outputs['new'],
                           'matched_rows':len(outputs['old']),
                           'scope':'event/revision and emitted case-end frame/diagnostic/final summary; timing removed'})
        if outputs['old']!=outputs['new']:
            (OUT/f'host-mismatch-{iteration}-{kind}.json').write_text(json.dumps(outputs,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
record['host_runs']=host_runs
record['host_pairs']=host_pairs
record['host_performance_ns']={kind:{field:pct([row[field]for row in rows])for field in ['core','projection','compare','transaction','frame','total']}for kind,rows in host_times.items()}
record['source_after']=source_identity();record['source_unchanged']=before==record['source_after']
record['binaries_after']={k:identity(p)for k,p in binary.items()};record['binaries_unchanged']=all(record['binaries'][k]['sha256']==record['binaries_after'][k]['sha256']for k in binary)
record['finished_utc']=datetime.now(timezone.utc).isoformat();write('summary.json',record)
compression={}
for p in RUNS.rglob('stdout.txt'):
    raw=p.read_bytes();packed=gzip.compress(raw,mtime=0);assert gzip.decompress(packed)==raw;p.with_name(p.name+'.gz').write_bytes(packed)
    compression[str(p.relative_to(OUT))]={'raw_bytes':len(raw),'raw_sha256':hashlib.sha256(raw).hexdigest(),'gzip_bytes':len(packed),'gzip_sha256':hashlib.sha256(packed).hexdigest(),'CRC_and_exact_bytes':True}
write('compression.json',compression)
index={str(p.relative_to(OUT)).replace('\\','/'):{'bytes':p.stat().st_size,'sha256':sha(p)}for p in OUT.rglob('*')if p.is_file()and p.name!='index.json'and p.suffix!='.exe'}
write('index.json',index)
print(json.dumps({'runs':len(record['runs']),'pairs_all_equal':all(p['same']for p in pairs),'source_unchanged':record['source_unchanged'],'binaries_unchanged':record['binaries_unchanged'],'engine':record['engine_performance_ns'],'host_total':{k:v['total']for k,v in record['host_performance_ns'].items()}},indent=2))
