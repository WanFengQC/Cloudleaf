from pathlib import Path
import json
import hashlib

ROOT=Path(__file__).resolve().parent
A=ROOT/'traditional-a'
B=ROOT/'traditional-b'
result={name:{'byte_identical':(A/name).read_bytes()==(B/name).read_bytes(),'sha256':hashlib.sha256((B/name).read_bytes()).hexdigest()} for name in ['layer.tsv','combined-runtime.tsv','lineage.jsonl','r16-combined-events.tsv']}
assert all(row['byte_identical'] for row in result.values())
(ROOT/'traditional-reproducibility.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
rows=[json.loads(line) for line in (B/'r16-all32-diagnosis.jsonl').read_text(encoding='utf-8').splitlines()]
for row in rows:
    if not row['baseline_path_exists']:
        print('\t'.join([row['case'],row['desired_text'],row['combined_pinyin'],str(row['combined_input_bytes'])]))
print('max_input_bytes',max(row['combined_input_bytes'] for row in rows))
