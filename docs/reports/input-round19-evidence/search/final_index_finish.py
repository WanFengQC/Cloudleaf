"""列出可归档新增证据；不包含编译目录、可执行文件或重复大原始日志。"""
from pathlib import Path
import gzip,hashlib,json
p=Path(__file__).resolve().parent
excluded={'target','.git','__pycache__'}
suffixes={'.exe','.dll','.pdb','.obj','.rlib','.rmeta'}
files={};skipped=[]
for path in sorted(p.rglob('*')):
    if not path.is_file():continue
    rel=path.relative_to(p)
    if any(part in excluded for part in rel.parts)or path.suffix.lower()in suffixes:continue
    if rel.as_posix()=='delivery-finish/index.json':continue
    packed=path.with_name(path.name+'.gz')
    if path.suffix!='.gz'and packed.is_file():
        assert gzip.decompress(packed.read_bytes())==path.read_bytes()
        skipped.append(str(rel));continue
    data=path.read_bytes()
    if path.suffix=='.gz':gzip.decompress(data)
    files[rel.as_posix()]={'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
record={'scope':'Temporary task evidence only; source patches delivered separately, no actual repository changes. New actual-source benchmark exactbinding saved in user-layer-performance/actual-source-binding.json. All original failed/truncated logs remain either raw or deterministicCRC-verifiedgzip.','files':files,'skipped_duplicate_raw':skipped,'total_bytes':sum(v['bytes']for v in files.values()),'count':len(files),'test_results':{'R18_reference':33480,'R18_complete_candidates':83700,'preference_reference':41850,'preference_complete_candidates':104625,'differences':0,'incomplete':0},'R18_final_replay':{'queries':837,'budgets':6,'original_budgets':90,'original_unique_fixed':28,'unique_remaining':2},'user_layer_replay':{'queries':4185,'budgets':30,'new_failure_cases':0,'off_empty_SearchResult_equal':837},'formal_metrics':None}
(p/'delivery-finish/index.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'files':len(files),'bytes':record['total_bytes'],'skipped_large_raw_duplicates':len(skipped)}))
