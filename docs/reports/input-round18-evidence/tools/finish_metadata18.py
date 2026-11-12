"""补齐已执行窗口和工具操作的事实记录，不制造未执行结果。"""
from pathlib import Path
from datetime import datetime, timezone
import hashlib,json,subprocess,re,math
root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
for label,doc in [('round18-ui-final','你好内核'),('round18-ui-correction','你好你'),('round18-ui-mixed','中国人qz')]:
    folder=root/'.local'/label
    rows=json.loads((folder/'automation.json').read_text(encoding='utf-8'))
    result={'recorded_utc':datetime.now(timezone.utc).isoformat(),'launch':json.loads((folder/'launch.json').read_text(encoding='utf-8')),'actions':len(rows),'automation_sha256':sha(folder/'automation.json'),'final_document':doc,'closed_owned_window_observed':True,'close_observation':'sky.list_windows after Alt+F4 returned no owned input-round14 window','screenshots':'Tool displayed live; only screenshot identities and UIA text retained','TSF':False,'system_IME_changes':False,'no_other_app_text_input':True}
    if label=='round18-ui-correction':result['focus_interruption']='Initial n/i preserved in recovery area, not EDIT Value; continuous retry separate; document first became 你好 after mouse candidate confirmation.'
    path=folder/'completion.json'
    if not path.exists():
        with path.open('x',encoding='utf-8')as f:json.dump(result,f,ensure_ascii=False,indent=2);f.write('\n')
    else:
        old=json.loads(path.read_text(encoding='utf-8'));assert old['final_document']==doc and old['automation_sha256']==result['automation_sha256']
host=root/'.local/round18-host-benchmark'
summary=json.loads((host/'summary.json').read_text(encoding='utf-8'))
assert len(summary['records'])==12 and all(r['returncode']==0 and len(r['timings_ns'])==171 for r in summary['records'])
metadata={'source_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),'tool_sha256':sha(root/'.local/host18.py'),'binary_sha256':sha(root/'.local/build/release/cloudleaf-ime.exe'),'events_sha256':sha(root/'tests/fixtures/input-round14-events.tsv'),'run_count':12,'all_returncodes_zero':True,'total_events':2052,'summary_sha256':sha(host/'summary.json'),'scope':'K5 actual dual-engine Host memory sink, no HWND; existing default events; short benchmark, not soak or TSF.'}
(host/'identity.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf-8')
# 窗口日志只描述本次事件，不将少量样本混作固定Host性能验收。
samples=[]
for label in ['round18-ui-final','round18-ui-correction','round18-ui-mixed']:
    for line in (root/'.local'/label/'stdout.txt').read_text(encoding='utf-8').splitlines():
        if line.startswith('HOST_SAMPLE '):samples.append({'run':label,**{k:int(v) for k,v in re.findall(r'(\w+_ns)=(\d+)',line)}})
stats={}
for name in sorted({k for r in samples for k in r if k.endswith('_ns')}):
    values=sorted(r[name]for r in samples if name in r)
    stats[name]={'n':len(values),**{f'p{p}':values[math.ceil(len(values)*p/100)-1]for p in[50,95,99]},'max':max(values)}
(root/'.local/round18-ui-sample-summary.json').write_text(json.dumps({'scope':'Actual interactive K10; dependent snapshots/actions; not controlled benchmark or TSF; no discard','samples':samples,'metrics_ns':stats},indent=2)+'\n',encoding='utf-8')
print(json.dumps({'window_runs':3,'host_events':2052,'interactive_samples':len(samples)}))
