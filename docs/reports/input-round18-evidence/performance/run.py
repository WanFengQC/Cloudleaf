"""固定二进制、词典、输入与模型的只读研发观测；不修改实际仓库。"""
from pathlib import Path
from datetime import datetime,timezone
import gzip,hashlib,json,math,re,subprocess,time

ROOT=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
OUT=Path(__file__).resolve().parent
RUNS=OUT/'runs';RUNS.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
runtime=Path(json.loads((ROOT/'.local/round17-lexicon-receipt.json').read_text(encoding='utf-8'))['runtime']['path'])
models={'lexical':ROOT/'.local/round15-language/signal.tsv','addone':ROOT/'.local/round16-language/character.tsv','witten':ROOT/'.local/round16-language/character-witten.tsv'}
binary={'old':ROOT/'.local/round18-baseline-build/release/examples/input_trace.exe','new':ROOT/'.local/build/release/examples/input_trace.exe','scheme':ROOT/'.local/build/release/examples/scheme_trace.exe'}
source_paths=[*sorted((ROOT/'crates/cloudleaf-search/src').glob('*.rs')),*sorted((ROOT/'crates/cloudleaf-pinyin/src').glob('*.rs')),*sorted((ROOT/'crates/cloudleaf-input/src').glob('*.rs')),ROOT/'crates/cloudleaf-input/examples/scheme_trace.rs',ROOT/'crates/cloudleaf-input/examples/input_trace.rs',ROOT/'Cargo.lock']
def hashes():return {str(p.relative_to(ROOT)).replace('\\','/'):sha(p) for p in source_paths}
source_before=hashes()
record={'started_utc':datetime.now(timezone.utc).isoformat(),'implementation_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT).decode().strip(),'source_before':source_before,'binary':{k:{'path':str(p),'bytes':p.stat().st_size,'sha256':sha(p)}for k,p in binary.items()},'runtime':{'path':str(runtime),'sha256':sha(runtime),'bytes':runtime.stat().st_size},'models':{k:{'path':str(p),'sha256':sha(p),'bytes':p.stat().st_size}for k,p in models.items()},'formal_metrics':{'top1':None,'top5':None,'mrr':None},'scope':'Engineering-only K10. Default171mixed events/run, no warmup discarded, process launch/loading/stdout excluded from Rust event timing. Scheme profile overhead measured separately from plain query. Ordinary Windows desktop, root may perform light owned-HWND tests concurrently; not controlled dedicated-latency acceptance. No TSF/heldout/user history. No per-algorithm memory claim.','runs':[]}
def save():
    (OUT/'summary.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
def run(label,command,input_bytes):
    directory=RUNS/label;directory.mkdir()
    start=time.perf_counter();result=subprocess.run(command,cwd=ROOT,input=input_bytes,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
    raw=result.stdout
    (directory/'stdout.txt').write_bytes(raw);(directory/'stderr.txt').write_bytes(result.stderr)
    row={'label':label,'command':command,'returncode':result.returncode,'seconds':time.perf_counter()-start,'input_sha256':hashlib.sha256(input_bytes).hexdigest(),'stdout_bytes':len(raw),'stdout_sha256':hashlib.sha256(raw).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()}
    (directory/'result.json').write_text(json.dumps(row,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n');record['runs'].append(row);save()
    print(json.dumps({'label':label,'returncode':result.returncode,'seconds':row['seconds']},ensure_ascii=False),flush=True)
    return raw.decode('utf-8',errors='strict').splitlines()
def pct(values):
    values=sorted(values)
    return {'n':len(values),**{f'p{p}':values[math.ceil(len(values)*p/100)-1]for p in [50,95,99]},'max':max(values)}if values else {'n':0}
def semantic(lines):
    selected=[line for line in lines if line.startswith(('FRAME\t','CANDIDATE\t','COMMIT\t','SEARCH\t','PATH\t'))]
    return [line.replace('interpretation_cost: InterpretationCost { edits: 0, fuzzy: 0, abbreviations: 0, prefixes: 0 }, ','').replace(', interpretation: None','')for line in selected]
default=(ROOT/'tests/fixtures/input-round14-events.tsv').read_bytes();(OUT/'default-events.tsv').write_bytes(default)
times={f'{v}-{m}':[]for v in ['old','new']for m in models};pairs=[]
for iteration in range(1,6):
    order=list(models);order=order[iteration%3:]+order[:iteration%3]
    for kind in order:
        outputs={}
        for variant in (['old','new']if iteration%2 else ['new','old']):
            model=models[kind];command=[str(binary[variant]),str(runtime),sha(runtime),str(model),sha(model),'lexical'if kind=='lexical'else'character_bigram']
            lines=run(f'default-{iteration}-{kind}-{variant}',command,default)
            values=[int(line.split('\t')[3])for line in lines if line.startswith('EVENT\t')]
            if len(values)!=171:raise RuntimeError(f'{kind}/{variant} retained wrong event denominator {len(values)}')
            times[f'{variant}-{kind}'].extend(values);outputs[variant]=lines
        same=semantic(outputs['old'])==semantic(outputs['new'])
        pairs.append({'iteration':iteration,'kind':kind,'full_retained_output_equal_after_only_zero_new_fields':same})
        if not same:
            a=semantic(outputs['old']);b=semantic(outputs['new']);(OUT/f'default-mismatch-{iteration}-{kind}.json').write_text(json.dumps({'old':a,'new':b},ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
record['default_pairs']=pairs;record['default_performance_ns']={k:pct(v)for k,v in times.items()};save()

cases=[('zgr','zgr'),('bjdx','bjdx'),('zg','zg'),('zhg','zhg'),('zhonggr','zhonggr'),('wzgs','wzgs'),('nihao','nihao'),('nh','nh'),('full-zhongguoren','zhongguoren'),('full-beijingdaxue','beijingdaxue'),('fuzzy-lin','lin'),('fuzzy-lilan','lilan'),('typo-omission','zhnogguo'),('typo-adjacent','nihso'),('typo-transpose','nihoa'),('typo-long','jintiantiqnqihenhao'),('raw-unknown','qz'),('separated',"xi'an"),('waiting',"ni'"),('limit-budget','n'*32)]
double=[('xiaohe-nihao','nihc'),('xiaohe-zhongguoren','vsgorf'),('xiaohe-beijingdaxue','bw jkd axt'.replace(' ','')),('xiaohe-woshi','woui'),('zero-a','aa'),('zero-ang','ah'),('zero-eng','eg'),('v-nv','nv'),('v-lve','lt'),('collision-lo','lo'),('incomplete-n','n'),('separated',"ni'hc"),('raw-unknown','zz')]
# 小鹤bei=bw、jing=jk、da=da、xue=xt，直接按已核实事实表计算。
double[2]=('xiaohe-beijingdaxue','bwjkdaxt')
scheme_times={};phase_times={};interpret_times={};candidate_rows=[];budget_rows=[]
for scheme in ['full','abbreviated','xiaohe','fuzzy-nl','correction']:
    selected=double if scheme=='xiaohe'else cases
    payload=('\n'.join(case+'\t'+raw for case,raw in selected)+'\n').encode('utf-8');(OUT/(scheme+'-cases.tsv')).write_bytes(payload)
    for kind,model in models.items():
        base=[str(binary['scheme']),str(runtime),sha(runtime),scheme]
        for iteration in range(1,4):
            for mode in ['engine','profile']:
                lines=run(f'scheme-{scheme}-{kind}-{iteration}-{mode}',[*base,mode,str(model),sha(model)],payload)
                key=scheme+'/'+kind
                for line in lines:
                    f=line.split('\t')
                    if f[0]=='EVENT':scheme_times.setdefault(key,[]).append(int(f[3]))
                    elif f[0]=='PROFILE':
                        for name,value in re.findall(r'(normalize_ns|base_ns|abbreviation_ns|fuzzy_ns|correction_ns|finalize_ns|total_ns): (\d+)',line):interpret_times.setdefault(key+'/'+name,[]).append(int(value))
                        match=re.search(r'plain_ns=(\d+)',line)
                        if match:phase_times.setdefault(key+'/plain_query',[]).append(int(match[1]))
                    elif f[0]=='PHASE':phase_times.setdefault(key+'/'+f[2],[]).append(int(f[3]))
                    elif f[0]=='RESULT'and iteration==1:
                        rawresult=f[2];texts=[json.loads(v)for v in re.findall(r'Candidate \{ text: ("(?:[^"\\]|\\.)*")',rawresult)]
                        stats=rawresult.split('statistics: ',1)[-1]
                        candidate_rows.append({'scheme':scheme,'model':kind,'case':f[1],'texts_top10':texts,'statistics':stats,'formal_gold':None})
                        if 'budget: Some' in stats:budget_rows.append({'scheme':scheme,'model':kind,'case':f[1],'statistics':stats})
        record['scheme_event_performance_ns']={k:pct(v)for k,v in scheme_times.items()};record['scheme_phase_ns']={k:pct(v)for k,v in phase_times.items()};record['scheme_interpretation_phase_ns']={k:pct(v)for k,v in interpret_times.items()};record['candidate_observations']=candidate_rows;record['budget_observations']=budget_rows;save()
record['source_after']=hashes();record['source_unchanged']=source_before==record['source_after'];record['finished_utc']=datetime.now(timezone.utc).isoformat();record['tool_sha256']=sha(Path(__file__));save()
compressed={}
for p in RUNS.rglob('stdout.txt'):
    raw=p.read_bytes();packed=gzip.compress(raw,mtime=0);assert gzip.decompress(packed)==raw;p.with_suffix('.txt.gz').write_bytes(packed)
    compressed[str(p.relative_to(OUT))]={'raw_bytes':len(raw),'raw_sha256':hashlib.sha256(raw).hexdigest(),'gzip_bytes':len(packed),'gzip_sha256':hashlib.sha256(packed).hexdigest(),'CRC_and_exact_bytes':True}
(OUT/'compression.json').write_text(json.dumps(compressed,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'source_unchanged':record['source_unchanged'],'default_pairs_equal':all(p['full_retained_output_equal_after_only_zero_new_fields']for p in pairs),'default_performance_ns':record['default_performance_ns'],'runs':len(record['runs']),'nonzero_returncodes':[r['label']for r in record['runs']if r['returncode']],'budget_observations':len(budget_rows)},ensure_ascii=False,indent=2))
