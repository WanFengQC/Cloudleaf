from pathlib import Path
from collections import Counter, defaultdict
import datetime
import hashlib
import json
import re
import unicodedata
import argparse

parser = argparse.ArgumentParser(description='冻结 Tatoeba 研发语料，保留研究与正式质量边界')
parser.add_argument('--source-dir', type=Path, default=Path(__file__).resolve().parent)
parser.add_argument('--output-dir', type=Path)
args = parser.parse_args()
ROOT = args.source_dir.resolve()
OUT = args.output_dir.resolve() if args.output_dir else ROOT / 'prepared-v3'
OUT.mkdir(exist_ok=False)
SOURCE_SHA = '374dc649bd61ed5597c58ad9f098edb831e7bbe4ea0831f82947bec1fe5c8bb8'
frozen = {
    'cmn_sentences_detailed.tsv.bz2': SOURCE_SHA,
    'cmn_sentences_detailed.tsv': '43c4e88355e5897aa20a1b1e73852a1edb70b6cd51a426be4dda6f08ca545005',
    'cmn_sentences_base.tsv.bz2':'d7edaa6c5096f98bd55fe7eed3c9c5797ef8aa024cd72d4079d55f9d628b348e',
    'cmn_sentences_base.tsv':'24d5d3b1e3368c6ab6a478360c27cdd5139c720ab203ceafb7212580fc0c6644',
    'cmn_tags.tsv.bz2':'52da76642b6a485e7702a0c965062a697bc84cd9696ebf218336ae0da63ce764',
    'cmn_tags.tsv':'b52ba7dd73f13344288345d9cbc9ace1d0daf0229e68459f623784049254016c',
    'cmn-cmn_links.tsv.bz2':'fd3fb1ac4c43a8ba795c1dd9c193e37fdfc3ddfcf88cb5694f6db369521be37c',
    'cmn-cmn_links.tsv':'7ba5bfef38bba2fe5cb6868741a19997348549294a427cc501e30053e849de37',
    'cmn-eng_links.tsv.bz2':'417b2697222806aaf22bf7f510dd5eed246869a1ef40d7d80c6b43f4758b54b8',
    'cmn-eng_links.tsv':'b84ee59c32150a6dca93f173ef5bc88d74fc37ed21a76d756465c763f8257e37',
}
for name, expected in frozen.items():
    if hashlib.sha256((ROOT/name).read_bytes()).hexdigest() != expected:
        raise ValueError('source identity mismatch: ' + name)

# 训练不转换简繁，规范化只用于泄漏隔离和历史样例排除。
def canonical(text):
    return ''.join(c for c in unicodedata.normalize('NFKC', text).lower() if unicodedata.category(c)[0] not in 'PZS')

def is_han(c):
    n = ord(c)
    return any(a <= n <= b for a, b in [(0x3400,0x4dbf),(0x4e00,0x9fff),(0xf900,0xfaff),(0x20000,0x2fa1f),(0x30000,0x323af)])

def fragments(text):
    result, current = [], []
    for c in text + '\0':
        if is_han(c): current.append(c)
        elif current:
            result.append(''.join(current))
            current = []
    return result

rows = []
ids = set()
for line in (ROOT / 'cmn_sentences_detailed.tsv').read_text(encoding='utf-8').splitlines():
    f = line.split('\t')
    if len(f) != 6 or not f[0].isdigit() or f[1] != 'cmn' or f[0] in ids:
        raise ValueError('invalid detailed TSV source shape or duplicate id')
    ids.add(f[0])
    rows.append({'id': f[0], 'language': f[1], 'text': f[2], 'owner': f[3], 'date_added': f[4], 'date_modified': f[5], 'source_url':'https://tatoeba.org/en/sentences/show/' + f[0], 'owner_is_guaranteed_original_author':False})
rows.sort(key=lambda r: int(r['id']))
parent = {}
def find(x):
    if x not in parent: parent[x] = x
    while parent[x] != x:
        parent[x] = parent[parent[x]]
        x = parent[x]
    return x

def union(x, y):
    a, b = find(x), find(y)
    if a != b: parent[max(a,b)] = min(a,b)

tags = defaultdict(list)
for line in (ROOT / 'cmn_tags.tsv').read_text(encoding='utf-8').splitlines():
    i, tag = line.split('\t')
    tags[i].append(tag)

for line in (ROOT / 'cmn_sentences_base.tsv').read_text(encoding='utf-8').splitlines():
    i, base = line.split('\t')
    if i not in ids: raise ValueError('base id absent from detailed source')
    find('cmn:' + i)
    if base not in ['0', '\\N']:
        union('cmn:' + i, ('cmn:' if base in ids else 'base:') + base)
for line in (ROOT / 'cmn-cmn_links.tsv').read_text(encoding='utf-8').splitlines():
    a, b = line.split('\t')
    if a not in ids or b not in ids: raise ValueError('cmn link id absent from detailed source')
    union('cmn:' + a, 'cmn:' + b)
missing_eng_links = []
for number, line in enumerate((ROOT / 'cmn-eng_links.tsv').read_text(encoding='utf-8').splitlines(),1):
    a, b = line.split('\t')
    if a not in ids:
        missing_eng_links.append({'line':number,'cmn_id':a,'eng_id':b,'reason':'cmn_id_absent_from_detailed_export'})
        continue
    union('cmn:' + a, 'eng:' + b)

representative = {}
for r in rows:
    norm = canonical(r['text'])
    r['canonical'] = norm
    if norm in representative: union('cmn:' + r['id'], representative[norm])
    else: representative[norm] = 'cmn:' + r['id']

# 全局稀有三字片段前缀连接，最后用完整集合精确核对 Jaccard >= 4/5。
features = {}
df = Counter()
for r in rows:
    if len(r['canonical']) >= 8:
        f = set(r['canonical'][p:p+3] for p in range(len(r['canonical'])-2))
        features[r['id']] = f
        df.update(f)
inverted = defaultdict(list)
candidate_pairs = 0
near_pairs = []
for r in sorted((r for r in rows if r['id'] in features), key=lambda r:(len(features[r['id']]),int(r['id']))):
    i = r['id']
    f = features[i]
    ordered = sorted(f, key=lambda token:(df[token],token))
    prefix = ordered[:len(f)-(4*len(f)+4)//5+1]
    candidates = set()
    for token in prefix: candidates.update(inverted[token])
    for other in candidates:
        previous = features[other]
        if 5*len(previous) < 4*len(f): continue
        candidate_pairs += 1
        intersection = len(previous & f)
        union_size = len(previous) + len(f) - intersection
        if 5*intersection >= 4*union_size:
            union('cmn:' + i, 'cmn:' + other)
            near_pairs.append([other,i,intersection,union_size])
    for token in prefix: inverted[token].append(i)

targets = ['我爱你','我愛你','这是一个比较复杂的问题','這是一個比較複雜的問題','今天天气很好','今天天氣很好','我正在开发一款中文输入法','我正在開發一款中文輸入法']
reasons = Counter()
historical_families = set()
family_rows = defaultdict(list)
for r in rows:
    reasons_here = []
    r['family'] = find('cmn:' + r['id'])
    family_rows[r['family']].append(r)
    if r['owner'] in ['', '\\N']: reasons_here.append('missing_current_owner_attribution')
    bad_tags = [t for t in tags[r['id']] if t.startswith('@') or 'copyright' in t.lower() or 'quote' in t.lower() or t.lower().startswith('by ') or t.lower() in ['unnatural','non-sentence','not a sentence','offensive','misogynistic','sexual','needs native check','incorrect transcription','change','check']]
    if bad_tags: reasons_here.append('quarantined_source_tags')
    r['tags'] = tags[r['id']]
    r['quarantined_tags'] = bad_tags
    if re.search(r'[^\s@]+@[^\s@]+\.[^\s@]+|https?://|www\.|[0-9]{7,}', r['text']): reasons_here.append('privacy_or_url_shape')
    r['historical_matches'] = [t for t in targets if t in r['canonical']]
    if r['historical_matches']:
        reasons_here.append('historical_target_substring')
        historical_families.add(r['family'])
    r['reasons'] = reasons_here
    r['fragments'] = fragments(r['text'])

for r in rows:
    if r['family'] in historical_families and not r['historical_matches']:
        r['reasons'].append('historical_target_family')
    bucket = int(hashlib.sha256(('cloudleaf-r16-tatoeba-family-v1:' + r['family']).encode()).hexdigest()[:16],16) % 100
    r['split'] = 'train' if bucket < 80 else 'dev' if bucket < 90 else 'heldout'
    if r['reasons']: reasons.update(r['reasons'])

eligible = {s:[] for s in ['train','dev','heldout']}
fragment_reasons = Counter()
for r in rows:
    if r['reasons']: continue
    for n, fragment in enumerate(r['fragments']):
        if len(fragment) < 2: fragment_reasons['shorter_than_2_scalars'] += 1
        elif len(fragment) > 128: fragment_reasons['longer_than_128_scalars'] += 1
        else: eligible[r['split']].append({'sentence_id':r['id'], 'family':r['family'], 'fragment_index':n, 'text':fragment, 'sha256':hashlib.sha256(fragment.encode()).hexdigest()})

# 预先固定 heldout > dev > train 的冲突优先级，不根据模型效果重新划分。
kept = {s:[] for s in ['train','dev','heldout']}
occupied = set()
for split in ['heldout','dev','train']:
    own = set()
    characters = 0
    for f in eligible[split]:
        if f['sha256'] in occupied:
            fragment_reasons[split + '_exact_other_split_fragment'] += 1
            continue
        if split == 'train' and (len(kept[split]) >= 100000 or characters + len(f['text']) > 1000000):
            fragment_reasons['train_capacity_omitted'] += 1
            continue
        kept[split].append(f)
        characters += len(f['text'])
        own.add(f['sha256'])
    occupied.update(own)

blacklist = sorted({hashlib.sha256(r['canonical'].encode()).hexdigest() for r in rows if r['split'] != 'train'} | {hashlib.sha256(r['text'].encode()).hexdigest() for r in rows if r['split'] != 'train'})
blacklist_set = set(blacklist)
cross_full = [r['id'] for r in rows if r['split'] == 'train' and not r['reasons'] and (hashlib.sha256(r['canonical'].encode()).hexdigest() in blacklist_set or hashlib.sha256(r['text'].encode()).hexdigest() in blacklist_set)]
assert not cross_full
assert not ({f['sha256'] for f in kept['train']} & {f['sha256'] for f in kept['dev'] + kept['heldout']})
assert not ({f['sha256'] for f in kept['dev']} & {f['sha256'] for f in kept['heldout']})

def write_jsonl(name, records):
    with (OUT / name).open('w',encoding='utf-8',newline='\n') as handle:
        for record in records: handle.write(json.dumps(record,ensure_ascii=False,sort_keys=True)+'\n')

for split in ['train','dev','heldout']:
    write_jsonl(split + '-fragments.jsonl', kept[split])
    (OUT / (split + '.txt')).write_text(''.join(f['text']+'\n' for f in kept[split]),encoding='utf-8',newline='\n')
write_jsonl('source-lineage.jsonl', rows)
(OUT / 'nontrain-fulltext-sha256.txt').write_text(''.join(x+'\n' for x in blacklist),encoding='utf-8',newline='\n')
report = {
    'created_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'source_family': 'tatoeba-community', 'usage':'research_only','formal_gold':False,
    'source_sha256':SOURCE_SHA,'source_rows':len(rows),'source_han_fragments':sum(len(r['fragments']) for r in rows),
    'cmn_eng_dangling_links_isolated':len(missing_eng_links),
    'current_owners':len({r['owner'] for r in rows}), 'owner_is_not_guaranteed_original_author':True,
    'individual_quarantined_rows':sum(bool(r['reasons']) for r in rows), 'quarantine_reason_counts_overlap':dict(reasons),
    'historical_direct_rows':sum(bool(r['historical_matches']) for r in rows),'historical_families':len(historical_families),
    'family_count':len(family_rows),'largest_family_rows':max(map(len,family_rows.values())),
    'near_rule':'NFKC/lower/punctuation-space-symbol stripped, >=8 scalars, set of 3-scalar shingles, exact Jaccard >= 4/5; connected components joined with original/base IDs and cmn/eng links',
    'near_prefix_candidate_pairs':candidate_pairs,'near_pairs':len(near_pairs),
    'split_rule':'sha256(cloudleaf-r16-tatoeba-family-v1: + stable minimal component node) first 64 bits mod100: train<80/dev<90/heldout>=90; fixed before model evaluation',
    'han_ranges':[[0x3400,0x4dbf],[0x4e00,0x9fff],[0xf900,0xfaff],[0x20000,0x2fa1f],[0x30000,0x323af]],
    'han_fragment_min_scalars':2,'han_fragment_max_scalars':128,'training_sequence_limit':100000,'training_scalar_limit':1000000,
    'fragment_exclusion_reason_counts':dict(fragment_reasons),
    'split_source_rows':dict(Counter(r['split'] for r in rows)),
    'split_retained_source_rows':dict(Counter(r['split'] for r in rows if not r['reasons'])),
    'split_fragments':{s:len(v) for s,v in kept.items()},'split_unicode_scalars':{s:sum(len(f['text']) for f in v) for s,v in kept.items()},
    'nontrain_fulltext_sha256_count':len(blacklist),'train_fulltext_blacklist_intersections':len(cross_full),
    'train_dev_heldout_exact_fragment_intersections':0,
    'notes':['Text is unconverted original Unicode; punctuation and non-Han end a fragment, so BOS/EOS represent retained Han fragment boundaries, not necessarily semantic sentence boundaries.',
             'Detailed export owner is current owner, not guaranteed original creator. Full sentence ID, owner and stable Tatoeba URL must be kept for attribution; no human annotation or original-author signature is asserted.',
             'Tags are sparse; absence of unsafe tags is not an independent content or copyright review.',
             'Near-duplicate isolation is only under the explicitly stated shingle criterion, not proof that all paraphrases or all cross-language translation families have been isolated.',
             'All partitions share global source family tatoeba-community and therefore are engineering-only splits, not independent gold.',
             'Original sentence or Han-fragment frequency in this selected learner corpus is not modern IME token frequency/DF/Zipf.'],
}
report['files']={p.name:{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in OUT.iterdir() if p.is_file()}
(OUT / 'summary.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
(OUT / 'near-pairs.json').write_text(json.dumps(near_pairs),encoding='utf-8')
(OUT / 'dangling-eng-links.json').write_text(json.dumps(missing_eng_links,indent=2),encoding='utf-8')
print(json.dumps(report,ensure_ascii=False,indent=2))
