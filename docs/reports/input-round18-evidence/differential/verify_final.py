from pathlib import Path
from datetime import datetime,timezone
import gzip,hashlib,json,subprocess
root=Path(__file__).resolve().parent;repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
records=[]
for name in ['evidence','evidence-final','legacy-final']:
    directory=root/name;summary=json.loads((directory/'summary.json').read_text(encoding='utf-8'))
    raw=directory/'differential.stdout';packed=directory/'differential.stdout.gz'
    uncompressed=gzip.decompress(packed.read_bytes());assert uncompressed==raw.read_bytes()
    current={name:sha(repo/name)for name in summary['source_before']}
    record={'directory':name,'head':summary.get('source_head',summary.get('head')),'source_unchanged_during_run':summary['source_unchanged'],'source_still_same':current==summary['source_after'],'passed':summary['run_passed'],'summary_lines':summary['summary_lines'][-2:]if name=='legacy-final'else [summary['summary_lines'][-1]],'raw_bytes':raw.stat().st_size,'raw_sha256':sha(raw),'gzip_bytes':packed.stat().st_size,'gzip_sha256':sha(packed),'CRC_and_exact_uncompressed_bytes':True,'full_commands_and_sources':str(directory/'summary.json')}
    records.append(record)
assert all(row['source_unchanged_during_run']for row in records)
assert all(row['passed']and row['source_still_same']for row in records[1:])
record={'at_utc':datetime.now(timezone.utc).isoformat(),'scope':'mechanical correctness, not IME language accuracy; original failures retained; no heldout/user data used','raw_failure_count':592,'new_complete_reference_queries':33480,'new_complete_candidate_comparisons':83700,'old_complete_reference_queries':22320,'old_complete_candidate_comparisons':44640,'final_failures':0,'final_incomplete':0,'final_beam_pruned_queries':0,'reference_candidate_field_equality':'Candidate PartialEq compares text/score/IDs/edge_indices/features including interpretation_cost','rankers':['Structural','Weighted','Lexical','Bigram','CharacterBigram/addone','CharacterBigram/WittenBell'],'schemes':['Abbreviated','Full+7fuzzyBoth','Full+CorrectionCost1','Xiaohe'],'candidate_K':10,'old_K':[1,2,5,10,20],'new_input_count':1395,'new_beam_width':1024,'old_beam_width':4096,'records':records,'binaries':{name: {'bytes':(root/'target/release'/name).stat().st_size,'sha256':sha(root/'target/release'/name)}for name in ['r18-scheme-diff.exe','legacy.exe']},'tool_sha256':sha(Path(__file__))}
(root/'final-validation.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({key:value for key,value in record.items()if key not in ['records','binaries']},ensure_ascii=False))
