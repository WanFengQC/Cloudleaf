from pathlib import Path
from collections import defaultdict
import json
import time

ROOT = Path(__file__).resolve().parent
started = time.perf_counter()
rows = [json.loads(line) for line in (ROOT/'prepared-v2'/'source-lineage.jsonl').read_text(encoding='utf-8').splitlines()]
features = {r['id']:set(r['canonical'][p:p+3] for p in range(len(r['canonical'])-2)) for r in rows if len(r['canonical'])>=8}
inverted = defaultdict(list)
pairs = set()
comparisons = 0
for i, f in sorted(features.items(),key=lambda pair:(len(pair[1]),int(pair[0]))):
    candidates = set()
    for token in f: candidates.update(inverted[token])
    for other in candidates:
        previous = features[other]
        if 5*len(previous)<4*len(f): continue
        comparisons += 1
        n = len(previous & f)
        if n*5 >= (len(previous)+len(f)-n)*4:
            pairs.add((other,i))
    for token in f: inverted[token].append(i)
optimized = {(a,b) for a,b,_,_ in json.loads((ROOT/'prepared-v2'/'near-pairs.json').read_text())}
missing = sorted(pairs - optimized)
extra = sorted(optimized - pairs)
result = {'reference':'full-shingle inverted intersection join; all possible positive-overlap candidates, exact set Jaccard >= 4/5, same >=8-scalar rule', 'source_records':len(rows), 'eligible_records':len(features), 'reference_candidate_comparisons':comparisons,'reference_near_pairs':len(pairs),'optimized_near_pairs':len(optimized),'missing':missing,'extra':extra,'seconds':time.perf_counter()-started}
(ROOT/'verify-near.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2))
assert not missing and not extra
