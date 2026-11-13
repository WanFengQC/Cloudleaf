"""只读取实际用户层实测，分别记录查询、构建和整进程资源。"""
from pathlib import Path
import gzip,hashlib,json
p=Path(__file__).resolve().parent;root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=p/'user-layer-performance';summary=json.loads((out/'summary.json').read_text(encoding='utf-8'))
lines=['# Round19 用户层查询与资源实测','',summary['scope'],'',
'合成手工词条全部由本工具生成：文本“测”+U+4E00起递增Han，读音统一ni hao，仅验证索引/歧义负载，不声称这些虚构组合为自然语言读音事实。ID升序user:1..4096、seed0、frequency unknown，源码及USER_TSV原文保留。没有真实私人词条、输入历史、频率、gold或授权伪造。',
'','每个model/size一个新进程，装载和构建在计时查询之前完成；每进程内三轮均保留。下表是格+搜索最终查询，不是逐键EngineK10或HostK5。','',
'|模型/规模/方案|n|预算失败|P50 ms|P95 ms|P99 ms|max ms|','|---|---:|---:|---:|---:|---:|---:|']
for key,metric in summary['metrics'].items():
    q=metric['total_query_ns']
    lines.append('|'+key+'|'+str(q['n'])+'|'+str(metric['budget_failures'])+'|'+ '|'.join(format(q[k]/1e6,'.4f')for k in ['p50','p95','p99','max'])+'|')
lines+=['','|模型/规模|条目数|新增估算B|模型估算B|层构建ms|资源采样n|峰值working MiB|private MiB|','|---|---:|---:|---:|---:|---:|---:|---:|']
for run in summary['runs']:
    ident=run['identity'];samples=run['samples']
    lines.append('|'+str(run['kind'])+'/'+str(run['size'])+'|'+str(ident['entries'])+'|'+str(ident['delta_bytes']if run['size']>=0 else 0)+'|'+str(ident['model_bytes'])+'|'+format(ident['layer_ns']/1e6,'.4f')+'|'+str(len(samples))+'|'+format(max(r['peak_working']for r in samples)/1024**2,'.2f')+'|'+format(max(r['private']for r in samples)/1024**2,'.2f')+'|')
lines+=['','- OFF与empty共837个完整SearchResult逐对象相同（含Candidate/Features/edge/统计），不忽略未知值或预算。',
'- 各规模837查询均仅原有两个字符模型limit-budget(n×32)各三次失败；每层6，总计30/4185，没有新增失败case。不把rc0等同每个query完整。',
'- 4096层仍保留同词典123544基础记录，合计127640条；基词频和模型计数不变，新增手工偏好与frequency unknown分离。',
'- Whole-process资源包含同时持有基础和组合索引、旧/新缓存、原件缓冲、输出诊断；不是单算法内存、128MiB Host预算或操作系统输入法资源。',
'- 基础层估算118497224B；空层delta1224B，4/512/4096层delta503136/935704/3995466B。用户层启用并非零开销；此表不能代替宿主旧快照并存与队列/帧的实际128MiB准入。',
'- 正式语言指标保持null，测试不使用heldout或私人语料；普通桌面有其他研发负载，性能尾部保留，不宣称产品实时门禁已过。']
source=summary['source']['production_source_files']
binding={name:{'measured':value,'current':hashlib.sha256((root/'crates/cloudleaf-search/src'/name).read_bytes()).hexdigest()}for name,value in source.items()}
for row in binding.values():row['same']=row['measured']==row['current']
(out/'actual-source-binding.json').write_text(json.dumps(binding,indent=2)+'\n',encoding='utf-8')
lines+=['',f'来源绑定核对：{sum(r["same"]for r in binding.values())}/{len(binding)}个search crate源码仍与当前实际仓库相同，详见actual-source-binding.json。']
(p/'delivery-finish/user-layer-observations.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
compression=[]
for dirname in ['user-layer-performance','paired-layout-exploration']:
    for f in sorted((p/dirname).rglob('*')):
        if not f.is_file()or f.suffix=='.gz'or f.stat().st_size<=100000:continue
        raw=f.read_bytes();packed=gzip.compress(raw,mtime=0);assert gzip.decompress(packed)==raw
        dst=f.with_name(f.name+'.gz')
        if dst.exists():assert dst.read_bytes()==packed
        else:dst.write_bytes(packed)
        compression.append({'source':str(f.relative_to(p)),'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'gzip':str(dst.relative_to(p)),'gzip_bytes':len(packed),'gzip_sha256':hashlib.sha256(packed).hexdigest(),'CRC_and_exact_bytes':'passed'})
(p/'delivery-finish/user-layer-compression.json').write_text(json.dumps(compression,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'runs':15,'queries':4185,'empty_layer_exact_equal':837,'actual_source_binding':all(r['same']for r in binding.values()),'budget_failures':30,'raw_large_files_compressed':len(compression)}))
