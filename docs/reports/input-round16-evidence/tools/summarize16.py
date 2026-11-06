from pathlib import Path
from collections import Counter
import hashlib
import json
import re
import subprocess

root=Path(__file__).resolve().parents[1]
out=root/'.local/round16-summary';out.mkdir(exist_ok=False)
def read(path):return json.loads(Path(path).read_text(encoding='utf-8'))
def details(path):
    cases={}
    for line in Path(path).read_text(encoding='utf-8').splitlines():
        f=line.split('\t')
        if f[0]=='FRAME':cases[f[1]]={'raw':json.loads(f[4]),'candidates':[],'paths':[]}
        elif f[0]=='CANDIDATE':cases[f[1]]['candidates'].append({'text':json.loads(f[3]),'id':f[4],'frequency':f[5],'kind':f[6]})
        elif f[0]=='PATH':cases[f[1]]['paths'].append({'id':f[2],'score':int(f[3]),'features':f[4],'edges':f[5]})
        elif f[0]=='SEARCH':cases[f[1]]['search']=f[2:]
    return cases
paired=read(root/'.local/round16-paired-traces/summary.json')
development={variant:details(root/f'.local/round16-dev-{variant}/stdout.tsv') for variant in ['lexical','character','covered','word']}
tasks=[json.loads(line) for line in Path('C:/Users/EDY/.codex/tmp/r16-corpus/word-contrast/development-tasks.jsonl').read_text(encoding='utf-8').splitlines()]
devrows=[]
for task in tasks:
    row={k:task[k] for k in ['case','desired_text','status','source_sentence_id','source_owner','source_url','pinyin'] if k in task}
    row['models']={}
    if task['status']=='ready':
        for variant,cases in development.items():
            case=cases[task['case']]
            texts=[candidate['text'] for candidate in case['candidates']]
            row['models'][variant]={'source_text_rank':texts.index(task['desired_text'])+1 if task['desired_text'] in texts else None,**case}
    devrows.append(row)
devsummary={variant:{'preselected_tasks':32,'unmappable':19,'ready':13,
    'source_text_is_first':sum(row['models'].get(variant,{}).get('source_text_rank')==1 for row in devrows),
    'source_text_present_in_top10':sum(row['models'].get(variant,{}).get('source_text_rank') is not None for row in devrows),
    'final_budget_failures':sum('Some(' in row['models'].get(variant,{}).get('search',['','',''])[2] for row in devrows if variant in row['models'])}
    for variant in development}
regressions=[row['case'] for row in devrows if row['models'].get('lexical',{}).get('source_text_rank')==1 and row['models'].get('character',{}).get('source_text_rank')!=1]
improvements=[row['case'] for row in devrows if row['models'].get('lexical',{}).get('source_text_rank')!=1 and row['models'].get('character',{}).get('source_text_rank')==1]
resources={}
for variant in paired['percentiles_ns']:
    rows=[r['result'] for r in paired['runs'] if r['variant']==variant]
    samples=[s for r in rows for s in r['resources']]
    resources[variant]={'processes':len(rows),'samples':len(samples),'sampled_working_max':max(s['working'] for s in samples),'sampled_private_max':max(s['private'] for s in samples),'os_peak_working_max':max(s['os_peak_working'] for s in samples),'cpu_seconds': [r['whole_process_cpu_seconds'] for r in rows]}
full=read(root/'.local/round16-verified-results.json')
assert len(full)==13 and all(v['returncode']==0 and v['head']=='8a0838b2261d0cf74f1ae17fa028849509deedc6' for v in full)
rust=(root/'.local/round16-runs/verified-04/stdout.txt').read_text(encoding='utf-8')
counts=[tuple(map(int,m)) for m in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',rust)]
python=(root/'.local/round16-runs/verified-08/stderr.txt').read_text(encoding='utf-8')
base='5045e18981dadefd3881a05fd021b31712ac243e'
oldpaths=subprocess.check_output(['git','ls-tree','-r','--name-only',base,'--','docs/reports','data','tests','licenses'],cwd=root).decode().splitlines()
changed=subprocess.check_output(['git','diff','--name-only',base,'HEAD'],cwd=root).decode().splitlines()
assert not set(oldpaths).intersection(changed)
test_names=[]
for path in subprocess.check_output(['git','ls-tree','-r','--name-only',base,'--','crates','adapters'],cwd=root).decode().splitlines():
    if path.endswith('.rs'):
        old=subprocess.check_output(['git','show',f'{base}:{path}'],cwd=root).decode('utf-8')
        current=(root/path).read_text(encoding='utf-8')
        names=re.findall(r'#\[test\][\s\S]*?\bfn\s+(\w+)\s*\(',old)
        assert all(re.search(r'\bfn\s+'+re.escape(n)+r'\s*\(',current) for n in names), path
        test_names.extend([path+':'+n for n in names])
preservation={'base':base,'old_report_data_test_license_paths_unchanged':len(oldpaths),'existing_rust_test_functions_retained':len(test_names),'test_names':test_names}
modelpaths=[root/'.local/round16-language'/name for name in ['character.tsv','character-rebuild.tsv','covered-character.tsv','word.tsv']]
assert modelpaths[0].read_bytes()==modelpaths[1].read_bytes()
models={p.name:{'path':str(p),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in modelpaths}
summary={'implementation_head':'8a0838b2261d0cf74f1ae17fa028849509deedc6','status':'partial',
    'rust':{'passed':sum(c[0] for c in counts),'failed':sum(c[1] for c in counts),'ignored':sum(c[2] for c in counts)},
    'python_tests':int(re.search(r'Ran (\d+) tests',python).group(1)), 'checks':full,
    'models':models,'performance_ns':paired['percentiles_ns'],'resources':resources,
    'development':devsummary,'development_source_first_improvements':improvements,'development_source_first_regressions':regressions,
    'formal_language_metrics':{'top1':None,'top5':None,'mrr':None},'production_model':False,'default_engine_changed':False,
    'CL010':'closed','TSF_registered':False}
for name,value in [('summary.json',summary),('development-comparison.json',devrows),('preservation.json',preservation)]:
    (out/name).write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
md=['# Round16真实候选Top10对照','', '仅研发观察，非人工gold；原始分段、ID、Features、模型版本和全部失败见对应stdout.tsv。','']
for case in ['nihao','zhongguo','woaini','weather','target','complex','ambiguous','separated']:
    md.extend(['## '+case,'','|模式|排名|正文|分数|词条分段|','|---|---:|---|---:|---|'])
    for variant in ['originallexical','character']:
        c=paired['cases'][variant][case]
        for index,(candidate,path) in enumerate(zip(c['candidates'],c['paths']),1):
            ids=re.findall(r'entry_id: "([^"]+)"',path['edges'])
            md.append(f"|{variant}|{index}|{candidate['text']}|{path['score']}|{' / '.join(ids)}|")
    md.append('')
(out/'candidate-comparison.md').write_text('\n'.join(md)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'rust':summary['rust'],'python':summary['python_tests'],'development':devsummary,'regressions':regressions,'resources':resources},ensure_ascii=False,indent=2))
