from pathlib import Path
import json,re
root=Path(__file__).resolve().parent
lines=(root/'evidence/differential.stdout').read_text(encoding='utf-8').splitlines()
rows=[v for v in lines if '\tcorrection\tstructural\t"aahah"' in v]
for row in rows:
    if row.startswith(('REFERENCE','COMPARE')):
        fields=row.split('\t');value=row.split('SearchResult { ',1)[1]
        candidates=value.split('], raw_fallback:',1)[0]
        pieces=candidates.split('Candidate { ')[1:]
        print(fields[:7])
        for p in pieces:
            text=re.search(r'text: ("(?:[^"\\]|\\.)*")',p).group(1)
            score=re.search(r'score: (-?\d+)',p).group(1)
            ids=re.search(r'entry_ids: (\[[^\]]*\])',p).group(1)
            edges=re.search(r'edge_indices: (\[[^\]]*\])',p).group(1)
            cost=re.search(r'interpretation_cost: InterpretationCost \{([^}]+)',p).group(1)
            print(text,score,ids,edges,cost)
print('bytes',len((root/'evidence/differential.stdout').read_bytes()))
