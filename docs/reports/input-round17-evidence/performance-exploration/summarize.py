from pathlib import Path
import csv
import difflib
import hashlib
import json

root=Path(__file__).resolve().parent
one=(root/'one-allocation/crates/cloudleaf-search/src/search.rs').read_text(encoding='utf-8')
pre=(root/'prefilter/crates/cloudleaf-search/src/search.rs').read_text(encoding='utf-8')
(root/'prefilter-only.patch').write_text(''.join(difflib.unified_diff(one.splitlines(keepends=True),pre.splitlines(keepends=True),fromfile='a/crates/cloudleaf-search/src/search.rs',tofile='b/crates/cloudleaf-search/src/search.rs')),encoding='utf-8',newline='\n')
counts={}
for kind in ['addone','witten']:
    counts[kind]={}
    for variant in ['baseline','one-allocation','prefilter']:
        rows=list(csv.DictReader((root/f'allocations-{variant}-{kind}.tsv').read_text(encoding='utf-8').splitlines(),delimiter='\t'))
        counts[kind][variant]={'query_count':len(rows),'total_allocations':sum(int(r['allocations']) for r in rows),'total_reallocations':sum(int(r['reallocations']) for r in rows),'total_cumulative_requested_bytes':sum(int(r['cumulative_requested_bytes']) for r in rows),'complex_case':next(row for row in rows if row['id']=='r14-5')}
measurement=json.loads((root/'measurements/summary.json').read_text(encoding='utf-8'))
report={'scope':'Exploratory independent temporary copies, immutable base521472d. Pure queries not InputEngine/Host/TSF. No repo edits. Root must repeat exact final implementation and event traces.',
    'minimal_recommendation':'Scorer::extend allocate text/id/edge vectors once at their final capacities. Optional prefilter computes exact-bound and seen comparison before materialization; profile must include its boundary transition in Scoring.',
    'timings':{kind:{variant:{key:values[key] for key in ['samples','p50_ns','p95_ns','p99_ns','max_ns']} for variant,values in entries.items()} for kind,entries in measurement['results'].items()},
    'old_new_full_SearchResult_comparisons':measurement['comparisons_full_result_including_statistics'],'old_new_all_equal':measurement['all_equal'],
    'allocation_counts':counts,'allocation_scope':'Temporary global System allocator counting tool, only search on prebuilt lattice. Cumulative requested bytes are not peak/live/RSS. Product code has no allocator/unsafe changes.',
    'reference_differential':{variant:json.loads((root/variant/'checks-v2/checks.json').read_text(encoding='utf-8')) for variant in ['one-allocation','prefilter']},
    'observed_tradeoff':'Add-one query P95 improves for both, but prefilter6.4673ms is slightly slower than once-only6.1821ms in these samples. Witten prefilter7.3230ms improves on once7.9625ms; once Witten max27.6577ms retained. No universal speedup claim.',
    'failures':['Initial temp --locked failed101 after adding new diff workspace member without regenerating lock; raw unit.stderr and checks.json preserved. checks-v2 generated offline lock then all tests/differentials passed.'],
    'patch_sha256':{name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in ['one-allocation.patch','prefilter.patch','prefilter-only.patch']}}
(root/'summary.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'output':str(root/'summary.json'),'patch_sha256':report['patch_sha256']},indent=2))
