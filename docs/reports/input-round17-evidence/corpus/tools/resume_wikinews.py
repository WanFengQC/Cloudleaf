"""继续已冻结的32篇取样；缓存不覆盖，429有界重试且保留失败。"""
from pathlib import Path
from datetime import datetime,timezone
import urllib.request
import urllib.parse
import urllib.error
import json
import hashlib
import time

OUT=Path(__file__).resolve().parent/'wikinews-originals'
API='https://zh.wikinews.org/w/api.php'
HEADERS={'User-Agent':'Cloudleaf-Research/1.0 (https://github.com/WanFengQC/Cloudleaf; bounded offline language evaluation)','Accept':'application/json'}
selection_path=OUT/'selection-before-content.json'
selection_bytes=selection_path.read_bytes()
selection=json.loads(selection_bytes)
requests=[];articles=[];errors=[]

def query(params,name):
    url=API+'?'+urllib.parse.urlencode({'format':'json','formatversion':2,**params})
    path=OUT/name
    if path.exists():
        raw=path.read_bytes()
        requests.append({'url':url,'file':name,'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'cache_reused':True,'original_response_metadata':'not recorded by initial script; existing raw retained'})
        return json.loads(raw)
    for attempt in range(1,4):
        time.sleep(2.2)
        try:
            with urllib.request.urlopen(urllib.request.Request(url,headers=HEADERS),timeout=30) as response:
                raw=response.read(2_000_001)
                if len(raw)>2_000_000:raise ValueError('resource exceeds bounded acquisition')
                metadata={'status':response.status,'headers':dict(response.headers)}
            with path.open('xb') as handle:handle.write(raw)
            requests.append({'url':url,'file':name,'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'cache_reused':False,**metadata})
            return json.loads(raw)
        except urllib.error.HTTPError as error:
            raw=error.read()
            failure={'url':url,'file':name,'attempt':attempt,'http_status':error.code,'headers':dict(error.headers),'body_sha256':hashlib.sha256(raw).hexdigest()}
            (OUT/(name+f'.attempt{attempt}.error')).write_bytes(raw)
            errors.append(failure)
            (OUT/'resume-errors.json').write_text(json.dumps(errors,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
            if error.code!=429 or attempt==3:raise
            wait=min(30,max(5,int(error.headers.get('Retry-After','20'))))
            time.sleep(wait)
    raise AssertionError('unreachable')

try:
    for item in selection['preselected']:
        pageid=item['pageid']
        original=query({'action':'query','pageids':pageid,'prop':'revisions|categories|pageprops','rvprop':'ids|timestamp|sha1|content|user|comment','rvslots':'main','cllimit':'max'},f'page-{pageid}-revision.json')
        page=original['query']['pages'][0];revision=page['revisions'][0];revid=revision['revid']
        rendered=query({'action':'parse','oldid':revid,'prop':'text|categories|links|revid|displaytitle','disableeditsection':1},f'page-{pageid}-rendered-original.json')
        history=query({'action':'query','pageids':pageid,'prop':'contributors','pclimit':'max'},f'page-{pageid}-contributors.json')
        info={**item,'pageid':pageid,'title':page['title'],'revision_id':revid,'timestamp':revision['timestamp'],'revision_sha1':revision.get('sha1'),
            'revision_url':f'https://zh.wikinews.org/w/index.php?oldid={revid}', 'history_url':'https://zh.wikinews.org/w/index.php?title='+urllib.parse.quote(page['title'])+'&action=history',
            'license_id':'CC-BY-4.0','usage':'research_only','production_approved':False,'attribution':'维基新闻/Wikinews contributors；固定修订及作者历史链接保留',
            'categories':page.get('categories',[]),'pageprops':page.get('pageprops',{}),'contributors_complete':'continue' not in history}
        articles.append(info)
        print(json.dumps({'pageid':pageid,'revid':revid,'title':page['title']},ensure_ascii=False),flush=True)
finally:
    result={'created_utc':datetime.now(timezone.utc).isoformat(),'source_family':'wikimedia-wikinews-zh','license_id':'CC-BY-4.0','usage':'research_only','production_approved':False,
        'preselected':len(selection['preselected']),'acquired':len(articles),'articles':articles,'requests':requests,'errors':errors,
        'source_selection_sha256':hashlib.sha256(selection_bytes).hexdigest(),
        'files':{p.name:{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in OUT.iterdir() if p.is_file()},
        'notes':['Initial selected pool and sample IDs are not changed after errors.','Current owner/editor names and contributor history do not imply individual verified signatures.','Only original Unicode text; no script conversion.','No model has been run. Source content exceptions and quotations still require explicit screening before task export.']}
    (OUT/'acquisition-manifest.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
