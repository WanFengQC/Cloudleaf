from pathlib import Path
from collections import Counter
import hashlib
import json
import time
import argparse

parser = argparse.ArgumentParser(description='词典最少词数研发分词和固定dev任务导出，不创建人工gold')
parser.add_argument('--source-dir',type=Path,default=Path(__file__).resolve().parent)
parser.add_argument('--runtime',type=Path,default=Path(r'C:\Users\EDY\.codex\worktrees\cl009-data\Cloudleaf\.local\round5-clean-6d3d4e1\release-a\runtime.tsv'))
parser.add_argument('--output-dir',type=Path)
args = parser.parse_args()
ROOT = args.source_dir.resolve()
RUNTIME = args.runtime.resolve()
EXPECTED_RUNTIME = '03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
EXPECTED_TRAIN = '1878767d1f0076011cac0be2c67afdfa0256a961030a80d91503a145d007590e'
OUT = args.output_dir.resolve() if args.output_dir else ROOT / 'word-contrast'
OUT.mkdir(exist_ok=False)
started = time.perf_counter()
runtime_bytes = RUNTIME.read_bytes()
if hashlib.sha256(runtime_bytes).hexdigest() != EXPECTED_RUNTIME:
    raise ValueError('runtime SHA mismatch')
train_bytes = (ROOT/'prepared-v3'/'train.txt').read_bytes()
if hashlib.sha256(train_bytes).hexdigest() != EXPECTED_TRAIN:
    raise ValueError('train SHA mismatch')

trie = {}
entries = {}
single_characters = set()
for line in runtime_bytes.decode('utf-8').splitlines()[2:]:
    fields = line.split('\t')
    if len(fields) != 9: raise ValueError('unexpected runtime row shape')
    entry_id, pinyin, text = fields[:3]
    entry = {'entry_id':entry_id,'pinyin':pinyin,'text':text}
    entries[entry_id] = entry
    node = trie
    for c in text: node = node.setdefault(c,{})
    current = node.get(None)
    if current is None or entry_id < current['entry_id']:
        node[None] = entry
    if len(text)==1: single_characters.add(text)

def segment(text):
    best = [None]*(len(text)+1)
    best[-1] = (0,(),[])
    for position in range(len(text)-1,-1,-1):
        node = trie
        for end in range(position,len(text)):
            node = node.get(text[end])
            if node is None: break
            entry = node.get(None)
            tail = best[end+1]
            if entry is not None and tail is not None:
                candidate = (tail[0]+1,(entry['entry_id'],)+tail[1],[entry]+tail[2])
                if best[position] is None or candidate[:2] < best[position][:2]: best[position] = candidate
    return None if best[0] is None else best[0][2]

train_lineage = [json.loads(line) for line in (ROOT/'prepared-v3'/'train-fragments.jsonl').read_text(encoding='utf-8').splitlines()]
train_lines = train_bytes.decode('utf-8').splitlines()
if [f['text'] for f in train_lineage] != train_lines: raise ValueError('train lineage ordering mismatch')
all_lineage = {r['id']:r for r in (json.loads(line) for line in (ROOT/'prepared-v3'/'source-lineage.jsonl').read_text(encoding='utf-8').splitlines())}
unknown_characters = Counter()
unknown = []
success = []
sequences = []
token_count = 0
for number, (text, lineage) in enumerate(zip(train_lines,train_lineage),1):
    result = segment(text)
    if result is None:
        missing = Counter(c for c in text if c not in single_characters)
        unknown_characters.update(missing)
        unknown.append({'train_line':number,**lineage,'reason':'no_complete_dictionary_segmentation','characters_without_single_character_entry':dict(missing)})
        continue
    if len(sequences)>=100000 or token_count+len(result)>1000000:
        raise ValueError('word training exceeds existing model capacity')
    token_count += len(result)
    sequences.append(' '.join(entry['entry_id'] for entry in result))
    success.append({'train_line':number,**lineage,'entries':result})

def write_jsonl(name, values):
    with (OUT/name).open('w',encoding='utf-8',newline='\n') as handle:
        for value in values: handle.write(json.dumps(value,ensure_ascii=False,sort_keys=True)+'\n')

sequence_bytes = ''.join(s+'\n' for s in sequences).encode('utf-8')
(OUT/'word-ID-sequences.txt').write_bytes(sequence_bytes)
sequence_sha = hashlib.sha256(sequence_bytes).hexdigest()
covered_char_bytes = ''.join(r['text']+'\n' for r in success).encode('utf-8')
(OUT/'word-covered-char-train.txt').write_bytes(covered_char_bytes)
transition_counts = Counter()
for sequence in sequences:
    tokens = sequence.split()
    transition_counts.update(zip(['<s>']+tokens,tokens+['</s>']))
write_jsonl('word-sequence-lineage.jsonl',success)
write_jsonl('uncovered-training-fragments.jsonl',unknown)
header = '\n'.join([
    '# cloudleaf-language-v1',
    'lexicon_sha256\t'+EXPECTED_RUNTIME,
    'source_sha256\t'+sequence_sha,
    'source_id\ttatoeba-cmn-20261031-minwords-ids',
    'license_id\tCC-BY-2.0-FR+CC-BY-SA-4.0',
    'tokenization\tmin-words-stable-id-v1; complete-runtime-text-path; no-script-conversion',
    'usage\tresearch_only',
    'kind\tbigram',
    'training_split\ttrain',
    'previous\tnext\tcount',
])+'\n'
(OUT/'metadata-header.tsv').write_text(header,encoding='utf-8',newline='\n')

# 先按固定散列选择32个不同来源句，不依据是否可映射或候选质量替换失败样本。
dev = [json.loads(line) for line in (ROOT/'prepared-v3'/'dev-fragments.jsonl').read_text(encoding='utf-8').splitlines()]
eligible = [r for r in dev if 2<=len(r['text'])<=24]
for r in eligible:
    key = 'cloudleaf-r16-real-dev-v1:'+r['sentence_id']+':'+r['sha256']+':'+str(r['fragment_index'])
    r['selection_sha256'] = hashlib.sha256(key.encode()).hexdigest()
selected = []
selected_sentences = set()
for r in sorted(eligible,key=lambda r:(r['selection_sha256'],int(r['sentence_id']),r['fragment_index'])):
    if r['sentence_id'] in selected_sentences: continue
    selected.append(r)
    selected_sentences.add(r['sentence_id'])
    if len(selected)==32: break
tasks, events = [], []
for number, r in enumerate(selected,1):
    source = all_lineage[r['sentence_id']]
    task = {'case':f'r16dev{number:03d}','source_sentence_id':r['sentence_id'],'source_owner':source['owner'],
            'source_url':source['source_url'],'source_family':'tatoeba-community','split':'dev',
            'source_sha256':'374dc649bd61ed5597c58ad9f098edb831e7bbe4ea0831f82947bec1fe5c8bb8',
            'runtime_sha256':EXPECTED_RUNTIME,'selection_sha256':r['selection_sha256'],
            'desired_text':r['text'],'desired_text_sha256':r['sha256'],'fragment_index':r['fragment_index'],
            'formal_gold':False,'usage':'engineering_only','pronunciation_source':'dictionary-minwords-stable-id-v1; not human pronunciation gold'}
    path = segment(r['text'])
    if path is None:
        task.update(status='unmappable',reason='no_complete_dictionary_segmentation',characters_without_single_character_entry=sorted({c for c in r['text'] if c not in single_characters}))
    else:
        pinyin = ''.join(entry['pinyin'].replace(' ','') for entry in path)
        task.update(entries=path,pinyin=pinyin,input_bytes=len(pinyin.encode('ascii')))
        if task['input_bytes']>64:
            task.update(status='input_too_long',reason='derived_pinyin_exceeds_existing_64byte_limit')
        else:
            task.update(status='ready')
            events.append(task['case']+'\t<Esc> '+pinyin+'\n')
    tasks.append(task)
write_jsonl('development-tasks.jsonl',tasks)
(OUT/'development-events.tsv').write_text(''.join(events),encoding='utf-8',newline='\n')
report = {
    'usage':'research_only','formal_gold':False,'runtime_sha256':EXPECTED_RUNTIME,'runtime_entries':len(entries),
    'source_char_training_sha256':EXPECTED_TRAIN,'word_sequence_sha256':sequence_sha,
    'tokenization':'globally minimum number of complete runtime-text dictionary entries; lexicographically ascending stable entry-ID sequence breaks ties; same正文多音读法按stable ID选取，不代表人工读音判断',
    'input_train_fragments':len(train_lines),'covered_train_fragments':len(sequences),'uncovered_train_fragments':len(unknown),
    'word_tokens':token_count,'word_vocabulary':len({entry['entry_id'] for r in success for entry in r['entries']}),
    'word_unique_transition_keys':len(transition_counts),'word_total_transitions_bos_eos':sum(transition_counts.values()),
    'word_max_sequence_line_bytes':max(len(s.encode('utf-8')) for s in sequences),'word_max_tokens_per_sequence':max(len(s.split()) for s in sequences),
    'word_coverage_matched_char_sha256':hashlib.sha256(covered_char_bytes).hexdigest(),
    'unknown_characters_occurrences':dict(unknown_characters),'unknown_single_character_types':len(unknown_characters),
    'original_char_training_units':sum(map(len,train_lines)),'covered_char_training_units':sum(len(r['text']) for r in success),
    'no_unknown_substitutions':True,'dictionary_modified':False,'heldout_or_dev_used_for_training':False,
    'development_selection':'first32 distinct sentence IDs under SHA256(cloudleaf-r16-real-dev-v1:sentence_id:fragment_sha256:fragment_index), 2..24 Han scalars; no replacement for failures',
    'dev_input_fragments':len(dev),'dev_length_eligible_fragments':len(eligible),'development_preselected_tasks':len(tasks),
    'development_statuses':dict(Counter(t['status'] for t in tasks)),
    'event_count':sum(len(t['pinyin'])+1 for t in tasks if t['status']=='ready'),
    'notes':['Unknown training fragments are wholly omitted with original lineage, never replaced by fake words or partial sentence transitions.',
             '训练和推理都使用同一词典ID单位，但真实输入搜索允许其他合法分词；自动最少词数训练分词有偏差，词级模型只是对照，不是真实人工分词金标。',
             '研发输入拼音取自待测词典，与词典存在依赖；desired_text来自Tatoeba原文，而非独立人工输入意图。任务只能报告engineering-only逐例结果。',
             'Only dev was used for task export. No heldout text was used to choose segmentation or model parameters.'],
    'seconds':time.perf_counter()-started,
}
report['files']={p.name:{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in OUT.iterdir() if p.is_file()}
(OUT/'summary.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps({k:v for k,v in report.items() if k!='unknown_characters_occurrences'},ensure_ascii=False,indent=2))
