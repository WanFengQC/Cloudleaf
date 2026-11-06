from pathlib import Path
from collections import Counter
import json,hashlib
root=Path(__file__).resolve().parents[1]
train=(root/'.local/round16-corpus-a/train.txt').read_text(encoding='utf-8').splitlines();chars=Counter(''.join(train));pairs=Counter()
for line in train:
    previous='<bos>'
    for c in line+'\0':
        following='<eos>' if c=='\0' else c;pairs[previous,following]+=1;previous=following
target=Path('C:/Users/EDY/.codex/tmp/r17-data/final-audit-v2/dev-targets.tsv')
cases=[]
for line in target.read_text(encoding='utf-8').splitlines()[1:]:
    case,pinyin,text=line.split('\t');previous='<bos>';missing=[]
    for c in text+'\0':
        following='<eos>' if c=='\0' else c
        if not pairs[previous,following]:missing.append([previous,following])
        previous=following
    cases.append({'case':case,'unknown_scalars':sorted(set(text)-chars.keys()),'scalar_count':len(text),'zero_observed_transitions':missing,'transitions':len(text)+1})
layer={line.split('\t')[1] for line in (root/'.local/round17-supplement-a/layer.tsv').read_text(encoding='utf-8').splitlines() if line and not line.startswith('#') and len(line.split('\t'))>=2}
out={'scope':'Offline corpus-count diagnostic only; no model fitting, output or heldout use','train_fragments':len(train),'train_scalars':sum(chars.values()),'train_unique_scalars':len(chars),'train_bos_eos':len(train),'all40dev':cases,'dev_cases_with_unknown_scalars':sum(bool(c['unknown_scalars'])for c in cases),'dev_unknown_scalar_types':sorted(set(c for row in cases for c in row['unknown_scalars'])),'dev_zero_transition_count':sum(len(c['zero_observed_transitions'])for c in cases),'dev_transition_count':sum(c['transitions']for c in cases),'targets_sha256':hashlib.sha256(target.read_bytes()).hexdigest(),'formal_language_metrics':None}
(root/'.local/round17-quality-diagnosis/news-domain-counts.json').write_text(json.dumps(out,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n');print(json.dumps({k:v for k,v in out.items()if k!='all40dev'},ensure_ascii=False))
