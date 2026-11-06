from pathlib import Path
import json
root=Path(__file__).resolve().parents[1]
tasks=[json.loads(x) for x in Path('C:/Users/EDY/.codex/tmp/r16-corpus/word-contrast/development-tasks.jsonl').read_text(encoding='utf-8').splitlines()]
lines=(root/'.local/round16-dev-witten/stdout.tsv').read_text(encoding='utf-8').splitlines()
cases={}
for line in lines:
    f=line.split('\t')
    if f[0]=='CANDIDATE':cases.setdefault(f[1],[]).append(json.loads(f[3]))
rows=[{'case':t['case'],'source':t['desired_text'],'rank':cases[t['case']].index(t['desired_text'])+1 if t['desired_text'] in cases[t['case']] else None,'top1':cases[t['case']][0]} for t in tasks if t['status']=='ready']
result={'n':32,'ready':13,'first':sum(v['rank']==1 for v in rows),'top10':sum(v['rank'] is not None for v in rows),'rows':rows}
(root/'.local/round16-witten-dev-summary.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(result,ensure_ascii=False,indent=2))
