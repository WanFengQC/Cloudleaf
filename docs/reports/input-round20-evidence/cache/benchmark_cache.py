"""同一二进制ON/OFF配对；保留完整Candidate与所有尾延迟，不进行语言质量评分。"""
from pathlib import Path
from datetime import datetime,timezone
import hashlib,json,math,re,subprocess,time

root=Path(__file__).resolve().parent
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=root/'cache-measurements';out.mkdir(exist_ok=False)
exe=Path('C:/Users/EDY/.codex/tmp/r20-incremental-target/release/examples/transition_cache_probe.exe')
runtime=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
models={'lexical':repo/'.local/round15-language/signal.tsv','addone':repo/'.local/round16-language/character.tsv','witten':repo/'.local/round16-language/character-witten.tsv'}
cases=repo/'docs/reports/input-round18-evidence/performance'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
records=[]
def quantiles(values):
    values=sorted(values)
    return {'n':len(values),**{f'p{n}':values[math.ceil(len(values)*n/100)-1]for n in [50,95,99]},'max':max(values)}
for kind,model in models.items():
    for size in [-1,0,4,512,4096]:
        modes=['off','on'] if size in [-1,4,4096] else ['on','off']
        for enabled in modes:
            folder=out/f'{kind}-{size}-{enabled}';folder.mkdir()
            command=[str(exe),str(runtime),sha(runtime),str(model),str(size),str(cases),enabled]
            start=time.perf_counter()
            with (folder/'stdout.txt').open('wb') as stdout,(folder/'stderr.txt').open('wb') as stderr:
                child=subprocess.run(command,cwd=root,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
            rows=[];cache=[]
            for line in (folder/'stdout.txt').read_text(encoding='utf-8').splitlines():
                fields=line.split('\t')
                if fields[0]=='CASE':
                    rows.append({'iteration':int(fields[1]),'scheme':fields[2],'case':fields[3],'raw':fields[4],
                                 'lattice_ns':int(fields[5].split('=')[1]),'search_ns':int(fields[6].split('=')[1]),
                                 'budget':re.search(r'budget: (None|Some\(\w+\))',fields[7])[1], 'result':fields[7]})
                elif fields[0]=='CACHE':
                    values={key:int(value) for key,value in re.findall(r'(\w+): (\d+)',fields[4])}
                    cache.append({'iteration':int(fields[1]),'scheme':fields[2],'case':fields[3],**values})
            record={'kind':kind,'size':size,'cache':enabled,'command':command,'returncode':child.returncode,
                    'at_utc':datetime.now(timezone.utc).isoformat(),'seconds':time.perf_counter()-start,
                    'runtime_sha256':sha(runtime),'model_sha256':sha(model),'binary_sha256':sha(exe),
                    'stdout_sha256':sha(folder/'stdout.txt'),'stderr_sha256':sha(folder/'stderr.txt'),
                    'queries':rows,'cache_statistics':cache}
            (folder/'result.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
            assert child.returncode==0 and len(rows)==279 and len(cache)==279
            records.append(record)
            print(json.dumps({'kind':kind,'size':size,'cache':enabled,'queries':len(rows),'budgets':sum(r['budget']!='None'for r in rows),'last_cache':cache[-1]}),flush=True)
summary={'scope':'3models*5user-layers*2cache modes, same exact binary,data/config/budgets/K10; 279fixed final queries/process, no dropped warmup or tails; complete SearchResult equality incl statistics/Features/provenance. No GUI,Host or language-quality acceptance.',
         'tool_sha256':sha(__file__),'metrics':{},'paired_complete_objects':0,'runs':len(records)}
for kind in models:
    for size in [-1,0,4,512,4096]:
        a=next(r for r in records if r['kind']==kind and r['size']==size and r['cache']=='off')
        b=next(r for r in records if r['kind']==kind and r['size']==size and r['cache']=='on')
        assert [r['result'] for r in a['queries']]==[r['result'] for r in b['queries']],(kind,size)
        summary['paired_complete_objects']+=len(a['queries'])
for record in records:
    previous={k:0 for k in ['lookups','hits','misses','evictions','invalidations']}
    for scheme in ['full','abbreviated','xiaohe','fuzzy-nl','correction']:
        rows=[q for q in record['queries']if q['scheme']==scheme]
        cache=[q for q in record['cache_statistics']if q['scheme']==scheme]
        totals={k:0 for k in previous}
        for q in record['cache_statistics']:
            for key in previous:
                if q['scheme']==scheme:totals[key]+=q[key]-previous[key]
                previous[key]=q[key]
        previous={k:0 for k in previous}
        summary['metrics'][f'{record["kind"]}/{record["size"]}/{record["cache"]}/{scheme}']={
            'lattice_ns':quantiles([q['lattice_ns']for q in rows]),'search_ns':quantiles([q['search_ns']for q in rows]),
            'total_query_ns':quantiles([q['lattice_ns']+q['search_ns']for q in rows]),
            'budget_failures':sum(q['budget']!='None'for q in rows),'cache_statistics':totals,
            'hit_rate':totals['hits']/totals['lookups']if totals['lookups'] else None,
            'max_resident_entries':max(q['resident_entries']for q in cache)}
(out/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'runs':len(records),'paired_full_objects':summary['paired_complete_objects'],'total_queries':sum(len(r['queries'])for r in records)}),flush=True)
