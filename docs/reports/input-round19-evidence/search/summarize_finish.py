"""汇总原始数值与严格比较，压缩保全大日志，不替换失败证据。"""
from pathlib import Path
import gzip,hashlib,json,re
p=Path(__file__).resolve().parent
sha=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
s=json.loads((p/'paired-final/summary.json').read_text(encoding='utf-8'))
lines=['# Round19 搜索专项复核','',
'同一123544条原词典、固定研究模型、原20个最终案例，K10，200000展开/50000驻留预算。每条件3新进程，无warmup/长尾剔除。以下是单次最终查询的格+搜索时间，不是逐键Engine、Host或TSF延迟。',
'','|方案/模型/版本|n|预算|P50 ms|P95 ms|P99 ms|max ms|','|---|---:|---:|---:|---:|---:|---:|']
for key,value in s['metrics'].items():
    q=value['total_query_ns']
    lines.append('|'+key+'|'+str(value['queries'])+'|'+str(value['budgets'])+'|'+ '|'.join(format(q[k]/1e6,'.4f')for k in ['p50','p95','p99','max'])+'|')
lines+=['','36个进程全部返回0，720最终查询；270组双方完整结果的全部Candidate相同。full条件还比较完整SearchResult统计，仅删除新增严格零值user_preference_units，180组全部相同。旧版本不完整的90组不被计为正确性证明。',
'','新方案严格机械差分：1395×4×6=33480完整Reference，83700完整Candidate比较，0差异/0不完整。实际用户层和偏好：1395×5×6=41850完整Reference，104625完整Candidate比较，0差异/0不完整。PositionDag对上下文模型分别16740/20925次明确未支持，不计为成功比较。',
'','CharacterModel::rebound仅重新编译词条字符缓存并绑定新Lexicon实例，观察计数、全部转移与未知映射不变。缓存重绑与固定概率的单元测试通过。用户偏好正负例验证开启后排序生效、解释成本仍更优先、清除后恢复候选对象、完整Reference相等；未生成真实个人词库或人工gold。',
'','初次成本上限便宜原型只修复8/30，未采用。最终采用稀疏精确后缀最佳优先：同首字符组保留完整成本/偏好/score/ID/edge最佳尾，已观测行和未观测基线分别比较；预算计入标签扫描/边/行/探测，取消检查保留。Reference、Beam与默认关闭字符搜索未替换。',
'','资源采样是完整进程（原件缓冲、词典、模型、查询、输出），不是单算法内存或128MiB内部门禁。普通Windows桌面，不承诺5ms实时验收。完整资源样本和返回码见paired-final各result/summary。']
replay=p/'r18-final-replay/summary.json'
if replay.exists():
    r=json.loads(replay.read_text(encoding='utf-8'))
    lines+=['',f'原R18全部最终案例复跑：{r["queries"]}次实际查询，预算失败{r["budget_failures"]}；唯一失败为{r["unique_failures"]}。不是将不完整Reference视为正确答案，也不以进程返回0冒称每查询成功。',
    '','本轮未提升预算、减少Top10、缩小词典或删除输入。原limit-budget n×32仍失败，预算由Expansions变为ResidentPaths，明确保留；不能宣称清零。']
(p/'delivery-finish/summary.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
compression=[]
for dirname in ['runs','paired-final','r18-final-replay']:
    for f in sorted((p/dirname).rglob('*')):
        if f.is_file()and f.suffix!='.gz'and f.stat().st_size>100000:
            raw=f.read_bytes();packed=gzip.compress(raw,mtime=0);assert gzip.decompress(packed)==raw
            dest=f.with_name(f.name+'.gz')
            if dest.exists():assert dest.read_bytes()==packed
            else:dest.write_bytes(packed)
            compression.append({'source':str(f.relative_to(p)),'bytes':len(raw),'sha256':sha(f),'gzip':str(dest.relative_to(p)),'gzip_bytes':len(packed),'gzip_sha256':sha(dest),'CRC_and_exact_bytes':'passed'})
(p/'delivery-finish/compression.json').write_text(json.dumps(compression,indent=2)+'\n',encoding='utf-8')
print('\n'.join(lines))
