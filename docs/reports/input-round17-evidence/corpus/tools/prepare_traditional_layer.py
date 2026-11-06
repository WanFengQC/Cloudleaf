"""固定已批准词典的传统单字试验层；不修改原runtime、不推断读音或频率。"""
from pathlib import Path
from collections import defaultdict, Counter
import argparse
import hashlib
import json
import re
import sys

parser = argparse.ArgumentParser()
parser.add_argument('--repo-path', type=Path, required=True)
parser.add_argument('--baseline-release', type=Path, required=True)
parser.add_argument('--source-archive', type=Path, required=True)
parser.add_argument('--output-dir', type=Path, required=True)
args = parser.parse_args()
REPO=args.repo_path.resolve()
BASE=args.baseline_release.resolve()
SOURCE=args.source_archive.resolve()
OUT=args.output_dir.resolve()

def sha(data): return hashlib.sha256(data).hexdigest()
def jsonl(path): return [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()]
def write_json(name, obj): (OUT/name).write_text(json.dumps(obj, ensure_ascii=False, indent=2)+'\n',encoding='utf-8',newline='\n')

runtime=(BASE/'runtime.tsv').read_bytes()
assert sha(runtime)=='03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
lock=json.loads((REPO/'data/sources/locks/cccedict-20260604-mirror7019e1c.json').read_text(encoding='utf-8'))
assert sha(SOURCE.read_bytes())==lock['sha256']
sys.path.insert(0,str(REPO))
from tools.data_pipeline.cedict_source import read_archive, parse_reading, source_key
original,_=read_archive(SOURCE,lock)
source_lines=original.splitlines()
records=jsonl(BASE/'records.jsonl')
base_rows=[line.split('\t') for line in runtime.decode('utf-8').splitlines()[2:]]
base_pairs={(r[2],r[1]) for r in base_rows}
base_entries={r[0]:{'entry_id':r[0],'pinyin':r[1],'text':r[2]} for r in base_rows}
grouped=defaultdict(list)
for record in records:
    traditional=record['traditional']
    if len(traditional)!=1 or len(record['pinyin'])!=1 or traditional==record['text']:
        continue
    pinyin=' '.join(record['pinyin'])
    if (traditional,pinyin) in base_pairs: continue
    # 复核批准record的每条原文证据；不对未通过生产解析的源记录另开后门。
    for evidence in record['evidence']:
        raw=source_lines[evidence['line']-1]
        assert raw==evidence['raw'] and evidence['member']=='cedict_ts.u8'
        match=re.fullmatch(r'([^ ]+) ([^ ]+) \[([^]]+)\] /(.*)/',raw)
        assert match and match.group(1)==traditional and match.group(2)==record['simplified']
        assert parse_reading(match.group(3))==record['pinyin']
        assert source_key(match.group(1),match.group(2),match.group(3))==record['source_record_id']
    grouped[(traditional,pinyin)].append(record)
OUT.mkdir(parents=True,exist_ok=False)
header='\n'.join(runtime.decode('utf-8').splitlines()[:2])+'\n'
layer=[];lineage=[]
for (text,pinyin), provenance in sorted(grouped.items()):
    provenance.sort(key=lambda r:r['source_record_id'])
    ident='entry:trad-r17:'+sha(json.dumps([lock['source_id'],text,pinyin],ensure_ascii=False,separators=(',',':')).encode())[:32]
    fields=[ident,pinyin,text,'','unknown','','cccedict:mirror-20260604:traditional-r17',provenance[0]['source_record_id'],'CC-BY-SA-4.0']
    layer.append('\t'.join(fields)+'\n')
    lineage.append({'id':ident,'text':text,'pinyin':pinyin,'frequency':None,'frequency_kind':'unknown','source':fields[6],'license_id':fields[-1],
        'original_record_ids':[r['id'] for r in provenance],'source_record_ids':[r['source_record_id'] for r in provenance],
        'source_evidence':[e for r in provenance for e in r['evidence']],'usage':'research_only','production_approved':False})
layer_bytes=(''.join(layer)).encode('utf-8')
(OUT/'layer.tsv').write_bytes(header.encode()+layer_bytes)
combined=runtime+layer_bytes
assert combined[:len(runtime)]==runtime
(OUT/'combined-runtime.tsv').write_bytes(combined)
with (OUT/'lineage.jsonl').open('w',encoding='utf-8',newline='\n') as handle:
    for item in lineage:handle.write(json.dumps(item,ensure_ascii=False,sort_keys=True)+'\n')

def index(rows):
    trie={}
    for r in rows:
        node=trie
        for c in r['text']:node=node.setdefault(c,{})
        current=node.get(None)
        if current is None or r['entry_id']<current['entry_id']:node[None]=r
    return trie
def segment(text,trie):
    best=[None]*(len(text)+1);best[-1]=(0,(),[])
    for start in range(len(text)-1,-1,-1):
        node=trie
        for end in range(start,len(text)):
            node=node.get(text[end])
            if node is None:break
            entry=node.get(None);tail=best[end+1]
            if entry is not None and tail is not None:
                candidate=(tail[0]+1,(entry['entry_id'],)+tail[1],[entry]+tail[2])
                if best[start] is None or candidate[:2]<best[start][:2]:best[start]=candidate
    return None if best[0] is None else best[0][2]

base_trie=index(base_entries.values())
layer_entries=[{'entry_id':r['id'],'text':r['text'],'pinyin':r['pinyin']} for r in lineage]
combined_trie=index([*base_entries.values(),*layer_entries])
readings_by_text=defaultdict(set)
for entry in [*base_entries.values(),*layer_entries]:readings_by_text[entry['text']].add(entry['pinyin'])
single={r['text'] for r in base_entries.values() if len(r['text'])==1}
source_missing=defaultdict(list)
for item in lineage:source_missing[item['text']].append(item)
tasks=jsonl(REPO/'docs/reports/input-round16-evidence/corpus/word-contrast/development-tasks.jsonl')
diagnostics=[];events=[]
for t in tasks:
    before=segment(t['desired_text'],base_trie)
    after=segment(t['desired_text'],combined_trie)
    missing=sorted({c for c in t['desired_text'] if c not in single})
    row={**t,'baseline_path_exists':before is not None,'combined_path_exists':after is not None,
        'baseline_missing_characters':missing,'missing_character_evidence':{c:[{'entry_id':r['id'],'pinyin':r['pinyin'],'source_record_ids':r['source_record_ids']} for r in source_missing[c]] for c in missing},
        'diagnosis_categories':['B:dictionary_traditional_head_projection'] if missing else [],
        'pinyin_parser_failed':None,'pinyin_parser_status':'not_run_before_reading_derivation','word_lattice_failure_demonstrated':None,'reading_human_confirmed':False,
        'mixed_text':any(not ('\u3400'<=c<='\u9fff') for c in t['desired_text']),
        'baseline_sha256':sha(runtime),'combined_sha256':sha(combined)}
    if after:
        pinyin=''.join(r['pinyin'].replace(' ','') for r in after)
        ambiguity=[{'text':entry['text'],'selected_pinyin':entry['pinyin'],'dictionary_readings':sorted(readings_by_text[entry['text']])} for entry in after if len(readings_by_text[entry['text']])>1]
        row.update(combined_entries=after,combined_pinyin=pinyin,combined_input_bytes=len(pinyin),combined_status='ready' if len(pinyin)<=64 else 'input_too_long',reading_ambiguities=ambiguity,reading_ambiguity_present=bool(ambiguity),human_confirmed_pinyin=None)
        if len(pinyin)<=64:events.append(t['case']+'\t<Esc> '+pinyin+'\n')
    else:row.update(combined_status='unmappable')
    diagnostics.append(row)
with (OUT/'r16-all32-diagnosis.jsonl').open('w',encoding='utf-8',newline='\n') as handle:
    for row in diagnostics:handle.write(json.dumps(row,ensure_ascii=False,sort_keys=True)+'\n')
(OUT/'r16-combined-events.tsv').write_text(''.join(events),encoding='utf-8',newline='\n')
(OUT/'r16-combined-targets.tsv').write_text('case\tpinyin\tsource_text\n'+''.join(r['case']+'\t'+r['combined_pinyin']+'\t'+r['desired_text']+'\n' for r in diagnostics if r['combined_path_exists']),encoding='utf-8',newline='\n')
unmapped=[r for r in diagnostics if not r['baseline_path_exists']]
report={'usage':'research_only','production_approved':False,'baseline_entries':len(base_rows),'baseline_sha256':sha(runtime),
    'source_zip_sha256':sha(SOURCE.read_bytes()),'source_expanded_sha256':sha(original.encode()),'source_lock_sha256':sha((REPO/'data/sources/locks/cccedict-20260604-mirror7019e1c.json').read_bytes()),
    'approved_original_records':len(records),'layer_entries':len(layer),'layer_characters':len({r['text'] for r in lineage}),'layer_pairs_overlap_with_baseline':0,
    'combined_entries':len(base_rows)+len(layer),'combined_sha256':sha(combined),'baseline_exact_byte_prefix_preserved':True,
    'all32_baseline_status':dict(Counter(t['status'] for t in tasks)), 'all32_combined_status':dict(Counter(r['combined_status'] for r in diagnostics)),
    'nineteen_unmappable_before':len(unmapped),'nineteen_complete_after':sum(r['combined_path_exists'] for r in unmapped),'nineteen_with_missing_traditional_characters':sum(bool(r['baseline_missing_characters']) for r in unmapped),
    'nineteen_input_too_long_after':sum(r['combined_status']=='input_too_long' for r in unmapped),'nineteen_with_ambiguous_dictionary_readings':sum(r['reading_ambiguity_present'] for r in unmapped),'all32_with_ambiguous_dictionary_readings':sum(r['reading_ambiguity_present'] for r in diagnostics),'runtime_parser_rule_changed':False,'manual_reading_gold':False,
    'notes':['Original approved records already carry traditional/simplified and source-reading evidence; baseline runtime projects only simplified heads. New single-character rows are separate research data, never a mutation of the approved release.',
        'Missing single character rows alone are not proof no phrase can cover them; complete dynamic programming segmentation was independently rerun on all32 before and after.',
        'Returned pinyin is dictionary-derived stable-ID minimum-word segmentation, not human-confirmed pronunciation. Rust lattice reachability needs root actual crate verification.',
        'Unmapped samples had no pinyin generated previously, so they cannot be classified as pinyin-parser failures or search failures.']}
report['files']={p.name:{'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())} for p in OUT.iterdir() if p.is_file()}
write_json('manifest.json',report)
print(json.dumps(report,ensure_ascii=False,indent=2))
