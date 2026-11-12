from pathlib import Path
import re,json
root=Path(__file__).resolve().parent
ref={};rows=[]
for line in (root/'evidence/differential.stdout').read_text(encoding='utf-8').splitlines():
    if not line.startswith(('REFERENCE\t','COMPARE\t')):continue
    f=line.split('\t');value=line.split('SearchResult { candidates: ',1)[1].split('], raw_fallback:',1)[0]+']'
    if f[0]=='REFERENCE':ref[tuple(f[1:4])]=value
    elif 'comparable=true\tequal=false' in line:
        before=ref[tuple(f[1:4])]
        normalize=lambda s:re.sub(r'edge_indices: \[[^\]]*\]','edge_indices: []',s)
        rows.append({'scheme':f[1],'ranker':f[2],'raw':json.loads(f[3]),'method':f[4],'only_edge_indices_differ':normalize(before)==normalize(value),'reference_candidate_texts':re.findall(r'Candidate \{ text: ("(?:[^"\\]|\\.)*")',before),'actual_candidate_texts':re.findall(r'Candidate \{ text: ("(?:[^"\\]|\\.)*")',value)})
summary={'failures':len(rows),'edge_indices_only_failures':sum(v['only_edge_indices_differ']for v in rows),'candidate_text_or_score_or_features_failures':sum(not v['only_edge_indices_differ']for v in rows),'rows':rows}
(root/'evidence/failure-diagnosis.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({k:v for k,v in summary.items()if k!='rows'}));print(json.dumps(next((r for r in rows if not r['only_edge_indices_differ']),None),ensure_ascii=False))
