"""汇总已经完成的固定样本，不重跑、不删长尾或失败。"""
from pathlib import Path
import hashlib,json,subprocess,sys

OUT=Path(__file__).resolve().parent
record=json.loads((OUT/'summary.json').read_text(encoding='utf-8'))
assert len(record['runs'])==42 and all(r['returncode']==0 for r in record['runs'])
assert len(record['engine_pairs'])==15 and all(p['same']for p in record['engine_pairs'])
assert record['source_unchanged']and record['binaries_unchanged']

lines=['# Round19 默认 Engine 与无窗口 Host 实测','',
    f"源码HEAD：`{record['head']}`；旧版来自R18最终CI `{record['old_head']}` 的固定 `input_trace.exe`，ZIP CRC与下载清单成员SHA已核验。当前二进制/源码测量前后不变。",'',
    '这组是默认用户层关闭，不是用户层开启或新方案性能。全部42个进程返回0；无GUI、私人词库、TSF、heldout或正式准确率。普通共享Windows桌面，不是专用实时验收。',
    '', '## 默认 Engine K10 配对', '',
    '每版本/模型171事件×5个新进程=855，交替旧新和模型顺序，保留首次与全部长尾；合计5130事件。15组完整稳定语义输出相同，含EVENT的handled/error/preedit及FRAME、CANDIDATE、COMMIT、SEARCH、PATH、SUMMARY。仅去掉EVENT耗时时钟列和新增加且严格等于0的user_preference_units；没有移除正文、ID、评分、edge、来源、解释成本或统计。LOAD/MODEL容量身份原样保留但不作为候选语义相等断言。', '',
    '|版本/模型|n|P50 ms|P95 ms|P99 ms|max ms|','|---|---:|---:|---:|---:|---:|']
for kind in ['lexical','addone','witten']:
    for version in ['old','new']:
        row=record['engine_performance_ns'][version+'-'+kind]
        lines.append('|'+version+'/'+kind+'|'+str(row['n'])+'|'+'|'.join(f"{row[f]/1e6:.4f}"for f in ['p50','p95','p99','max'])+'|')
changes={kind:100*(record['engine_performance_ns']['new-'+kind]['p95']/record['engine_performance_ns']['old-'+kind]['p95']-1)for kind in ['lexical','addone','witten']}
lines+=['',f"默认P95变化：Lexical {changes['lexical']:+.2f}%，add-one {changes['addone']:+.2f}%，Witten–Bell {changes['witten']:+.2f}%。两个当前字符模型仍超过5ms，不宣称零开销或门禁通过。",'',
    '## K5 双引擎 Host', '', '既有四条件各3个新进程×171事件=513；共2052事件。无HWND、内存文档sink，SIMULATED Applied不持久学习。两引擎+比较/事务的total不能与上面的单Engine K10混比。', '',
    '|模型/阶段|n|P50 ms|P95 ms|P99 ms|max ms|','|---|---:|---:|---:|---:|---:|']
for kind in ['lexical','addone','witten','supplement-addone']:
    for field in ['core','projection','compare','transaction','frame','total']:
        row=record['host_performance_ns'][kind][field]
        lines.append('|'+kind+'/'+field+'|'+str(row['n'])+'|'+'|'.join(f"{row[f]/1e6:.4f}"for f in ['p50','p95','p99','max'])+'|')
lines+=['','## 50ms进程资源观测','',
    '包含原件读取缓冲、不可变词典/模型、查询及输出诊断。这里是进程采样和采样时读取的OS峰值计数，不是单算法分配、COM、Host内部128MiB预算或物理内存硬上界；退出前最后一段可能未采样。', '',
    '|条件|进程数|采样数|working最大MiB|private最大MiB|采样OS peak最大MiB|','|---|---:|---:|---:|---:|---:|']
resources={}
for group in [f'engine-{v}-{k}'for k in ['lexical','addone','witten']for v in ['old','new']]+[f'host-{k}'for k in ['lexical','addone','witten','supplement-addone']]:
    if group.startswith('engine-'):
        _,version,kind=group.split('-');rows=[r for r in record['runs']if r['label'].startswith('engine-')and r['label'].endswith('-'+kind+'-'+version)]
    else:
        kind=group.removeprefix('host-');rows=[r for r in record['runs']if r['label'].startswith('host-')and r['label'].split('-',2)[2]==kind]
    samples=[s for r in rows for s in r['resource_samples']]
    data={'processes':len(rows),'samples':len(samples),**{f:max(s[f]for s in samples)for f in ['working','private','OS_peak_working']}}
    resources[group]=data
    lines.append('|'+group+f"|{data['processes']}|{data['samples']}|"+'|'.join(f"{data[f]/1024**2:.2f}"for f in ['working','private','OS_peak_working'])+'|')
lines+=['','## 身份与原始材料','',
    '`summary.json`记录全部实际命令、退出码、二进制/模型/词典/事件SHA、全部资源样本与源码前后SHA。`runs/*/stdout.txt.gz`为原始输出，compression.json逐项验证gzip CRC和解压原字节；stderr与result完整保留。', '',
    f"旧input_trace SHA `{record['binaries']['old']['sha256']}`；新 `{record['binaries']['new']['sha256']}`；Host `{record['binaries']['host']['sha256']}`。", '',
    '没有使用该默认性能结果声称简拼/用户词库/真实Windows输入法达到正式产品质量。']
(OUT/'handoff.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
versions={}
for cmd in [[str(Path.home()/'.cargo/bin/rustc.exe'),'-Vv'],[str(Path.home()/'.cargo/bin/cargo.exe'),'-V'],[sys.executable,'--version']]:
    r=subprocess.run(cmd,capture_output=True)
    versions[' '.join(cmd)]={'returncode':r.returncode,'stdout':r.stdout.decode('utf-8'),'stderr':r.stderr.decode('utf-8')}
handoff={'scope':record['scope'],'source_head':record['head'],'old_head':record['old_head'],'engine_events':5130,'host_events':2052,'runs':42,'all_returncodes_zero':True,'engine_pairs_all_equal':True,'engine':record['engine_performance_ns'],'p95_change_percent':changes,'host':record['host_performance_ns'],'resources':resources,'binaries':record['binaries'],'data':record['data'],'versions':versions,'formal_metrics':record['formal_metrics']}
(OUT/'handoff.json').write_text(json.dumps(handoff,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
files={str(p.relative_to(OUT)).replace('\\','/'):{'bytes':p.stat().st_size,'sha256':sha(p)}for p in OUT.rglob('*')if p.is_file()and p.name!='index.json'and p.suffix!='.exe'}
(OUT/'index.json').write_text(json.dumps(files,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'engine':record['engine_performance_ns'],'host_total':{k:v['total']for k,v in record['host_performance_ns'].items()},'p95_change':changes,'indexed_files':len(files),'resource_groups':resources},indent=2))
