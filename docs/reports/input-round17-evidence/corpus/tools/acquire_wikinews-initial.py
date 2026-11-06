"""有界取得中文维基新闻固定修订原件及许可，冻结后才运行任何模型。"""
from pathlib import Path
from datetime import datetime,timezone
import hashlib
import json
import urllib.parse
import urllib.request
import time

OUT=Path(__file__).resolve().parent/'wikinews-originals'
OUT.mkdir(exist_ok=False)
API='https://zh.wikinews.org/w/api.php'
HEADERS={'User-Agent':'Cloudleaf-Round17-Research/1.0 (bounded linguistic evaluation, no account or editing)','Accept':'application/json'}
requests=[]

def fetch(url,name):
    start=time.perf_counter()
    with urllib.request.urlopen(urllib.request.Request(url,headers=HEADERS),timeout=30) as response:
        raw=response.read(2_000_001)
        if len(raw)>2_000_000:raise ValueError('bounded resource exceeded')
        headers=dict(response.headers);status=response.status
    (OUT/name).write_bytes(raw)
    requests.append({'url':url,'file':name,'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'status':status,'headers':headers,'seconds':time.perf_counter()-start})
    return raw
def query(params,name):
    return json.loads(fetch(API+'?'+urllib.parse.urlencode({'format':'json','formatversion':2,**params}),name))

fetch('https://zh.wikinews.org/zh-hans/'+urllib.parse.quote('Wikinews:版权信息'),'copyright.html')
fetch('https://zh.wikinews.org/zh-cn/'+urllib.parse.quote('Wikinews:轉載我們的報道'),'reuse.html')
fetch('https://creativecommons.org/licenses/by/4.0/legalcode.en','cc-by-4.0-legalcode.html')
fetch('https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use','wikimedia-terms.html')
categories=['科技','经济','環境','教育','交通','健康','文化']
pool={}
for number,category in enumerate(categories):
    doc=query({'action':'query','list':'categorymembers','cmtitle':'Category:'+category,'cmlimit':100,'cmnamespace':0,'cmsort':'timestamp','cmdir':'desc','cmprop':'ids|title|timestamp'},f'category-{number}.json')
    for item in doc.get('query',{}).get('categorymembers',[]):
        if item.get('timestamp','')<'2023-01-01':continue
        if any(part in item['title'] for part in ['報紙頭條','报纸头条','维基新闻','維基新聞']):continue
        item=dict(item)
        item['pool_categories']=[category]
        if item['pageid'] in pool:pool[item['pageid']]['pool_categories'].append(category)
        else:pool[item['pageid']]=item
    time.sleep(.15)
ranked=sorted(pool.values(),key=lambda item:hashlib.sha256(('cloudleaf-r17-wikinews-articles-v1:'+str(item['pageid'])).encode()).hexdigest())
selected=ranked[:32]
selection={'created_utc':datetime.now(timezone.utc).isoformat(),'source_family':'wikimedia-wikinews-zh','policy':'Seven predeclared topic categories; latest100 per category by category timestamp; added>=2023; exclude newspaper-headline compilations and project stories. Fixed article hash picks first32 before content/mapping/model outputs. Category timestamp is not article creation date.','pool_count':len(pool),'preselected':selected,'manual_gold':False,'usage':'research_only'}
(OUT/'selection-before-content.json').write_text(json.dumps(selection,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
articles=[]
for item in selected:
    pageid=item['pageid']
    original=query({'action':'query','pageids':pageid,'prop':'revisions|categories|pageprops','rvprop':'ids|timestamp|sha1|content|user|comment','rvslots':'main','cllimit':'max'},f'page-{pageid}-revision.json')
    page=original['query']['pages'][0]
    revision=page['revisions'][0]
    revid=revision['revid']
    rendered=query({'action':'parse','oldid':revid,'prop':'text|categories|links|revid|displaytitle','disableeditsection':1},f'page-{pageid}-rendered-original.json')
    history=query({'action':'query','pageids':pageid,'prop':'contributors','pclimit':'max'},f'page-{pageid}-contributors.json')
    info={'pageid':pageid,'title':page['title'],'revision_id':revid,'timestamp':revision['timestamp'],'revision_sha1':revision.get('sha1'),'revision_url':f'https://zh.wikinews.org/w/index.php?oldid={revid}',
        'history_url':'https://zh.wikinews.org/w/index.php?title='+urllib.parse.quote(page['title'])+'&action=history','attribution':'Wikinews contributors / 维基新闻；见固定修订与历史记录','license_id':'CC-BY-4.0','usage':'research_only','production_approved':False,
        'categories':page.get('categories',[]),'pageprops':page.get('pageprops',{}),'contributors_complete':'continue' not in history,**item}
    articles.append(info)
    print(json.dumps({'pageid':pageid,'revid':revid,'bytes':len(json.dumps(original,ensure_ascii=False).encode())},ensure_ascii=False),flush=True)
    time.sleep(.15)
manifest={'created_utc':datetime.now(timezone.utc).isoformat(),'schema_version':1,'source_family':'wikimedia-wikinews-zh','license_id':'CC-BY-4.0','usage':'research_only','production_approved':False,'articles':articles,'requests':requests,'total_download_bytes':sum(r['bytes'] for r in requests),'source_selection_sha256':hashlib.sha256((OUT/'selection-before-content.json').read_bytes()).hexdigest(),'notes':['Raw article Unicode is preserved; no zh-hans or script conversion is requested.','Selected source articles are not human IME intent or gold.','Only text is used; images/audio have separate rights and are excluded.','Contributors/history preserve attribution but do not prove every quoted passage is independently licensed; content exceptions must be screened.','No model was invoked before source selection and revision freezing.']}
(OUT/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'articles':len(articles),'pool':len(pool),'total_bytes':manifest['total_download_bytes']}))
