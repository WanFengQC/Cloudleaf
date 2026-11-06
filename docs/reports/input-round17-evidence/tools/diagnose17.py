"""对真实候选输出作离线概率诊断；浮点解释不替代Rust定点评分。"""
from pathlib import Path
from collections import Counter
import hashlib
import json
import math

root=Path(__file__).resolve().parents[1]
out=root/'.local/round17-quality-diagnosis';out.mkdir(exist_ok=False)
comparison=json.loads((root/'docs/reports/input-round16-evidence/summary/development-comparison.json').read_text(encoding='utf-8'))
train=(root/'.local/round16-corpus-a/train.txt').read_text(encoding='utf-8').splitlines()
train_counts=Counter(train)
all_characters=Counter(''.join(train))
def load(name):
    path=root/'.local/round16-language'/name
    lines=path.read_text(encoding='utf-8').splitlines()
    metadata=dict(line.split('\t') for line in lines[1:9])
    pairs={(f[0],f[1]):int(f[2]) for f in (line.split('\t') for line in lines[10:])}
    totals=Counter();incoming=Counter();types=Counter();vocabulary={'<eos>','<unk>'}
    for (a,b),count in pairs.items():
        totals[a]+=count;incoming[b]+=count;types[a]+=1;vocabulary.add(b)
        if a!='<bos>':vocabulary.add(a)
    return metadata,pairs,totals,incoming,types,vocabulary,hashlib.sha256(path.read_bytes()).hexdigest()
models={'character':load('character.tsv'),'witten':load('character-witten.tsv')}
def explain(body,model):
    metadata,pairs,totals,incoming,types,vocabulary,sha=model
    seq=['<bos>',*(c if c in vocabulary else '<unk>' for c in body),'<eos>'];rows=[]
    for a,b in zip(seq,seq[1:]):
        count=pairs.get((a,b),0)
        if metadata['kind']=='character_bigram':num=count+1;den=totals[a]+len(vocabulary)
        else:
            total=sum(incoming.values())+len(vocabulary);observed=types[a] or 1
            num=count*total+observed*(incoming[b]+1);den=(totals[a]+observed)*total
        rows.append({'previous':a,'next':b,'observed_count':count,'previous_observed_total':totals[a],
            'probability_numerator':num,'probability_denominator':den,'log2_float_diagnostic':math.log2(num)-math.log2(den)})
    return {'model_sha256':sha,'model_kind':metadata['kind'],'source_sha256':metadata['source_sha256'],
        'text':body,'characters':len(body),'unknown_scalars':[c for c in body if c not in vocabulary],
        'exact_training_fragment_count':train_counts[body],'transitions':rows,'sum_log2_float_diagnostic':sum(r['log2_float_diagnostic'] for r in rows)}
rows=[]
for case in comparison:
    if case['status']!='ready':continue
    result={'case':case['case'],'source_text':case['desired_text'],'source_url':case['source_url'],'models':{},'reading_human_confirmed':False}
    for name,model in models.items():
        actual=case['models'][name];text=actual['candidates'][0]['text']
        source=explain(case['desired_text'],model);top=explain(text,model)
        result['models'][name]={'source':source,'actual_top1':top,'source_rank_in_actual_top10':actual['source_text_rank'],
            'actual_score':actual['paths'][0]['score'],'actual_features':actual['paths'][0]['features'],
            'actual_search':actual['search'],'float_diagnostic_scope':'Unquantized rational probability explanation only; actual order/score/Features from recorded Rust InputEngine.',
            'evidence_categories':['E:source_not_first' if text!=case['desired_text'] else 'source_is_first',
                'G:unknown_source_character' if source['unknown_scalars'] else 'source_characters_all_in_vocabulary',
                'I:only_one_previous_character_in_model'],
            'no_budget_or_beam_failure_in_final_frame': 'budget: None' in actual['search'][2] and 'beam_pruned: 0' in actual['search'][2]}
    rows.append(result)
summary={'source':'Round16 32 preselected tasks, 13 ready remain all included; separate 19 diagnosis supplied by traditional-layer audit.',
    'ready_cases':len(rows),'real_training_sequences':len(train),'real_training_unicode_scalars':sum(map(len,train)),
    'training_vocabulary':len(all_characters),'models':{n:{'kind':v[0]['kind'],'sha256':v[-1],'BOS_count':v[2]['<bos>'],'EOS_count':v[3]['<eos>']} for n,v in models.items()},
    'case_diagnostics':rows,'formal_metrics':{'top1':None,'top5':None,'mrr':None}}
(out/'all-ready-probability-diagnostics.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'ready':len(rows),'source_unknown_cases':{n:sum(bool(r['models'][n]['source']['unknown_scalars']) for r in rows) for n in models}},ensure_ascii=False))
