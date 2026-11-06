"""冻结独立新闻来源研发任务；真实原文不是人工IME gold，人工字段保持null。"""
from pathlib import Path
from html.parser import HTMLParser
from collections import defaultdict,Counter
from datetime import datetime,timezone
import argparse
import hashlib
import json
import re
import unicodedata

PARSER=argparse.ArgumentParser()
PARSER.add_argument('--source-dir',type=Path,required=True)
PARSER.add_argument('--output-dir',type=Path,required=True)
PARSER.add_argument('--baseline-runtime',type=Path,required=True)
PARSER.add_argument('--combined-runtime',type=Path,required=True)
PARSER.add_argument('--tatoeba-prepared',type=Path,required=True)
ARGS=PARSER.parse_args()
SRC=ARGS.source_dir.resolve();OUT=ARGS.output_dir.resolve()

def sha(data):return hashlib.sha256(data).hexdigest()
def read_json(path):return json.loads(path.read_text(encoding='utf-8'))
def read_jsonl(path):return [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()]
def canonical(text):return ''.join(c for c in unicodedata.normalize('NFKC',text).lower() if unicodedata.category(c)[0] not in 'PZS')
def han(c):return any(a<=ord(c)<=b for a,b in [(0x3400,0x4dbf),(0x4e00,0x9fff),(0xf900,0xfaff),(0x20000,0x2fa1f),(0x30000,0x323af)])
def fragments(text):
    pending=[];result=[]
    for c in text+'\0':
        if han(c):pending.append(c)
        elif pending:result.append(''.join(pending));pending=[]
    return result
def shingles(text):return {text[p:p+3] for p in range(len(text)-2)}

class ArticleParagraphs(HTMLParser):
    def __init__(self):
        super().__init__();self.stack=[];self.parts=None;self.rows=[]
    def handle_starttag(self,tag,attrs):
        fields=dict(attrs);classes=set(fields.get('class','').split())
        blocked=tag in ['table','blockquote','figure','script','style','sup'] or any(c in classes for c in ['infobox','noprint','navbox','metadata','reflist','references','thumb','toc','ambox','mw-editsection'])
        if tag in ['br','hr','img','input','meta','link','wbr']:return
        self.stack.append((tag,blocked))
        if tag=='p':self.parts=[]
    def handle_endtag(self,tag):
        if tag=='p' and self.parts is not None:
            text=''.join(self.parts).strip()
            if text:self.rows.append(text)
            self.parts=None
        for i in range(len(self.stack)-1,-1,-1):
            if self.stack[i][0]==tag:self.stack=self.stack[:i];break
    def handle_data(self,data):
        if self.parts is not None and not any(flag for _,flag in self.stack):self.parts.append(data)

manifest=read_json(SRC/'acquisition-manifest.json')
assert manifest['acquired']==manifest['preselected']==32
for request in manifest['requests']:
    assert sha((SRC/request['file']).read_bytes())==request['sha256']
baseline_bytes=ARGS.baseline_runtime.read_bytes()
combined_bytes=ARGS.combined_runtime.read_bytes()
assert sha(baseline_bytes)=='03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
assert sha(combined_bytes)=='c9ab8b56b9d960091a1c39c37b7c1df7882ff50de2a64767174af8f4244c219f'
def index(data):
    trie={};readings=defaultdict(set)
    for line in data.decode('utf-8').splitlines()[2:]:
        fields=line.split('\t');entry={'id':fields[0],'pinyin':fields[1],'text':fields[2],'source':fields[6],'source_record_id':fields[7],'license_id':fields[8]}
        readings[entry['text']].add(entry['pinyin'])
        node=trie
        for c in entry['text']:node=node.setdefault(c,{})
        if None not in node or entry['id']<node[None]['id']:node[None]=entry
    return trie,readings
def segment(text,trie):
    best=[None]*(len(text)+1);best[-1]=(0,(),[])
    for start in range(len(text)-1,-1,-1):
        node=trie
        for end in range(start,len(text)):
            node=node.get(text[end])
            if node is None:break
            row=node.get(None);tail=best[end+1]
            if row is not None and tail is not None:
                candidate=(tail[0]+1,(row['id'],)+tail[1],[row]+tail[2])
                if best[start] is None or candidate[:2]<best[start][:2]:best[start]=candidate
    return None if best[0] is None else best[0][2]

base_trie,_=index(baseline_bytes);combo_trie,readings=index(combined_bytes)
train_rows=read_jsonl(ARGS.tatoeba_prepared/'train-fragments.jsonl')
train_canonical={canonical(row['text']):row for row in train_rows}
train_features={text:shingles(text) for text in train_canonical if len(text)>=8}
inverted=defaultdict(set)
for text,features in train_features.items():
    for feature in features:inverted[feature].add(text)

# 先固定来源、片段及dev/heldout归属，后进行词典映射，不按可映射性补位。
frozen=[];article_diagnostics=[];source_fragments=[]
for article in manifest['articles']:
    pageid=article['pageid'];revid=article['revision_id']
    revision_doc=read_json(SRC/f'page-{pageid}-revision.json')
    wikitext=revision_doc['query']['pages'][0]['revisions'][0]['slots']['main']['content']
    parsed=read_json(SRC/f'page-{pageid}-rendered-original.json')['parse']
    assert parsed['revid']==revid
    html=parsed['text'] if isinstance(parsed['text'],str) else parsed['text']['*']
    extractor=ArticleParagraphs();extractor.feed(html)
    flags=[]
    for marker in ['copyvio','copyright violation','侵权','侵權','版权侵犯','版權侵犯','fair use','fairuse']:
        if marker in wikitext.lower():flags.append(marker)
    published=bool(re.search(r'\{\{\s*publish\s*(?:\||\}\})',wikitext,re.I)) or any(c.get('title')=='Category:已发布' for c in article['categories'])
    if not published:flags.append('not_published_template')
    dates=re.findall(r'\{\{date\|([^}]+)\}\}',wikitext,re.I)
    paragraphs=[];excluded=[]
    for n,text in enumerate(extractor.rows):
        reasons=[]
        if re.search(r'[「」『』“”\"“”]|https?://|www\.|[^\s@]+@[^\s@]+\.[^\s@]+',text):reasons.append('quotation_or_contact_shape')
        if text.startswith('【') and ('讯' in text or '訊' in text):reasons.append('date_header')
        if '消息來源' in text or '消息来源' in text:reasons.append('sources_header')
        if reasons:excluded.append({'paragraph':n,'reason':reasons,'sha256':sha(text.encode())});continue
        paragraphs.append((n,text))
    candidates=[]
    for pn,text in paragraphs:
        for fn,fragment in enumerate(fragments(text)):
            if 6<=len(fragment)<=24:
                key=f'cloudleaf-r17-news-fragments-v1:{pageid}:{revid}:{pn}:{fn}:{sha(fragment.encode())}'
                candidates.append({'source_text':fragment,'paragraph':pn,'fragment':fn,'source_text_sha256':sha(fragment.encode()),'selection_sha256':sha(key.encode())})
    candidates.sort(key=lambda r:r['selection_sha256'])
    split='dev' if int(sha(('cloudleaf-r17-news-family-v1:'+str(pageid)).encode())[:16],16)%100<50 else 'heldout'
    article_diagnostics.append({'pageid':pageid,'title':article['title'],'revision_id':revid,'published':published,'article_date_templates':dates,'copyright_exception_flags':flags,'eligible_fragments':len(candidates),'excluded_paragraphs':excluded,'split':split})
    for n in range(3):
        row={'case':f'r17news{len(frozen)+1:03}','source_family':'wikimedia-wikinews-zh','article_family':str(pageid),'source_pageid':pageid,'source_revision_id':revid,'source_title':article['title'],'source_url':article['revision_url'],'source_history_url':article['history_url'],'source_license_id':'CC-BY-4.0','source_attribution':article['attribution'],'split':split,'usage':'engineering_only','formal_gold':False,'human_confirmed_pinyin':None,'human_intent':None,'acceptable_outputs':None,'ambiguity':None,'notes':None,'copyright_exception_flags':flags}
        if n>=len(candidates):row.update(source_status='no_eligible_fragment',source_text=None)
        else:
            row.update(candidates[n]);row['source_status']='quarantined_source_exception' if flags else 'frozen'
        frozen.append(row)

parents={str(article['pageid']):str(article['pageid']) for article in manifest['articles']}
def family_find(value):
    while parents[value]!=value:
        parents[value]=parents[parents[value]];value=parents[value]
    return value
def family_union(a,b):
    a,b=family_find(a),family_find(b)
    if a!=b:parents[max(a,b)]=min(a,b)
cross_article_near=[]
for i,left in enumerate(frozen):
    if left.get('source_text') is None:continue
    a=canonical(left['source_text']);fa=shingles(a)
    for right in frozen[:i]:
        if right.get('source_text') is None or left['article_family']==right['article_family']:continue
        b=canonical(right['source_text']);fb=shingles(b)
        if a==b or (min(len(a),len(b))>=8 and 5*len(fa&fb)>=4*len(fa|fb)):
            family_union(left['article_family'],right['article_family'])
            cross_article_near.append([left['case'],right['case']])
for row in frozen:
    family=family_find(row['article_family'])
    row['article_family']=family
    row['split']='dev' if int(sha(('cloudleaf-r17-news-family-v1:'+family).encode())[:16],16)%100<50 else 'heldout'

OUT.mkdir(parents=True,exist_ok=False)
with (OUT/'sample-freeze-before-mapping.jsonl').open('w',encoding='utf-8',newline='\n') as handle:
    for row in frozen:handle.write(json.dumps(row,ensure_ascii=False,sort_keys=True)+'\n')

events={split:[] for split in ['dev','heldout']};tasks=[]
for row in frozen:
    row=dict(row)
    if row['source_status']!='frozen':row.update(mapping_status='not_run_source_quarantine');tasks.append(row);continue
    text=row['source_text'];canon=canonical(text)
    near=[]
    features=shingles(canon)
    if len(canon)>=8:
        candidates=set().union(*(inverted.get(feature,set()) for feature in features))
        for original in candidates:
            other=train_features[original];intersection=len(features&other);union=len(features|other)
            if 5*intersection>=4*union:near.append({'source_sentence_id':train_canonical[original]['sentence_id'],'train_text_sha256':sha(original.encode()),'intersection':intersection,'union':union})
    row['tatoeba_train_exact_overlap']=canon in train_canonical
    row['tatoeba_train_near_overlap']=sorted(near,key=lambda r:r['source_sentence_id'])
    if canon in train_canonical or near:row.update(mapping_status='quarantined_training_overlap');tasks.append(row);continue
    before=segment(text,base_trie);after=segment(text,combo_trie)
    row.update(baseline_complete_path=before is not None,combined_complete_path=after is not None,baseline_sha256=sha(baseline_bytes),combined_sha256=sha(combined_bytes))
    if after is None:row.update(mapping_status='unmappable')
    else:
        pinyin=''.join(entry['pinyin'].replace(' ','') for entry in after)
        ambiguity=[{'text':entry['text'],'chosen':entry['pinyin'],'alternatives':sorted(readings[entry['text']])} for entry in after if len(readings[entry['text']])>1]
        row.update(derived_pinyin=pinyin,derived_input_bytes=len(pinyin),derived_entries=after,reading_ambiguities=ambiguity,pinyin_source='min-words-stable-id-from-same-dictionary; not human gold',mapping_status='ready' if len(pinyin)<=64 else 'input_too_long')
        if row['mapping_status']=='ready':events[row['split']].append(row['case']+'\t<Esc> '+pinyin+'\n')
    tasks.append(row)

with (OUT/'tasks.jsonl').open('w',encoding='utf-8',newline='\n') as handle:
    for row in tasks:handle.write(json.dumps(row,ensure_ascii=False,sort_keys=True)+'\n')
for split,lines in events.items():(OUT/(split+'-events.tsv')).write_text(''.join(lines),encoding='utf-8',newline='\n')
(OUT/'article-screening.json').write_text(json.dumps(article_diagnostics,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
report={'created_utc':datetime.now(timezone.utc).isoformat(),'source_family':'wikimedia-wikinews-zh','source_manifest_sha256':sha((SRC/'acquisition-manifest.json').read_bytes()),'selection_sha256':sha((SRC/'selection-before-content.json').read_bytes()),
    'source_articles':32,'preselected_slots':len(frozen),'sampling_rule':'Seven fixed topic categories, hash-selected first32 articles; three hash-selected original Han fragments 6..24 scalars/article before mapping. No replacement after unmappable or quarantine.','split_rule':'Article pageid families unioned by sampled-fragment canonical equality or >=8scalar Jaccard3-shingle>=.8; stable minimal component pageid SHA first64bits mod100 <50 dev else heldout. train empty, this source never used for training or parameter choice.','cross_article_near_pairs':cross_article_near,'article_families_after_near_union':len({row['article_family'] for row in tasks}),'split_counts':dict(Counter(r['split'] for r in tasks)),'source_statuses':dict(Counter(r['source_status'] for r in tasks)),'mapping_statuses':dict(Counter(r['mapping_status'] for r in tasks)),
    'dev_statuses':dict(Counter(r['mapping_status'] for r in tasks if r['split']=='dev')),'heldout_statuses':dict(Counter(r['mapping_status'] for r in tasks if r['split']=='heldout')),
    'baseline_complete_paths':sum(r.get('baseline_complete_path',False) for r in tasks),'combined_complete_paths':sum(r.get('combined_complete_path',False) for r in tasks),
    'tatoeba_train_exact_overlaps':sum(r.get('tatoeba_train_exact_overlap',False) for r in tasks),'tatoeba_train_near_overlaps':sum(bool(r.get('tatoeba_train_near_overlap')) for r in tasks),
    'training_used':False,'heldout_model_evaluated':False,'formal_gold':False,'formal_metrics':{'top1':None,'top5':None,'mrr':None},'license_id':'CC-BY-4.0','production_approved':False,
    'notes':['Article source dates may predate recent category timestamps; dates are separately preserved, no assumption of all2025/2026 content.','News text covers public affairs/health/technology/culture/economy but is not an independent human IME intent corpus or balanced daily chat/work corpus.','No simplified/traditional conversion; punctuation/non-Han split fragments, so some semantic context and mixed input are absent.','Quoted/contact-shaped paragraphs and flagged source exceptions are excluded before fragment selection; no claim that all third-party rights/personality rights have been exhaustively reviewed.','Near overlap is exact3-shingle Jaccard>=.8 for >=8scalar fragments; it does not prove semantic paraphrase separation.','All human input intent, accepted outputs, notes and pronunciation confirmation remain null.']}
report['files']={p.name:{'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())} for p in OUT.iterdir() if p.is_file()}
(OUT/'summary.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(report,ensure_ascii=False,indent=2))
