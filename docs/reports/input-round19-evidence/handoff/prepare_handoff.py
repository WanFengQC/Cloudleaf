"""只读提取Round19证据数字，输出到临时交付目录。"""
from pathlib import Path
import hashlib,json,re,subprocess
ROOT=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf');TMP=Path('C:/Users/EDY/.codex/tmp');OUT=TMP/'r19-delivery'
def read(p):return json.loads(Path(p).read_text(encoding='utf-8'))
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
checks=read(ROOT/'.local/round19-verified-results.json')
rust=(ROOT/'.local/round19-runs/verified-04/stdout.txt').read_text(encoding='utf-8')
counts=[tuple(map(int,m))for m in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',rust)]
totals=[sum(v[i]for v in counts)for i in range(3)]
search=TMP/'r19-search';paired=read(search/'paired-final/summary.json');layers=read(search/'user-layer-performance/summary.json');replay=read(search/'r18-final-replay/summary.json')
delivered=read(search/'delivery-finish/source-identities.json');bindings={}
for name,row in delivered.items():
    now=(ROOT/name).read_bytes();old=(search/'delivery-finish'/Path(name).name).read_bytes()
    bindings[name]={'current_sha256':sha(ROOT/name),'delivered_sha256':row['sha256'],'exact_bytes':now==old,'LF_normalized_source_equal':now.replace(b'\r\n',b'\n')==old.replace(b'\r\n',b'\n')}
actual_layer_binding={name:sha(ROOT/'crates/cloudleaf-search/src'/name)==value for name,value in layers['source']['production_source_files'].items()}
record={'read_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT).decode().strip(),'checks':{'commands':len(checks),'all_returncodes_zero':all(v['returncode']==0 for v in checks),'heads':sorted({v['head']for v in checks}),'rust_passed':totals[0],'rust_failed':totals[1],'rust_ignored':totals[2],'python_passed':71},'source_binding':bindings,'user_layer_measurement_current_source_binding':actual_layer_binding,'baseline_budget_failures':90,'replayed_budget':replay,'paired_query_metrics':paired['metrics'],'user_layer_query_metrics':layers['metrics'],'user_layer_runs':layers['runs'],'empty_layer_full_results_equal_count':len(layers['empty_layer_candidate_equality']),'formal_metrics':None,'differential':{'schemes_reference':33480,'schemes_complete_candidate_comparisons':83700,'users_reference':41850,'users_complete_candidate_comparisons':104625,'final_differences':0,'final_incomplete':0,'position_unsupported_schemes':16740,'position_unsupported_users':20925},'storage':read(TMP/'r19-user/delivery-identity.json'),'raw_paths':{'paired':str(search/'paired-final'),'layers':str(search/'user-layer-performance'),'replay':str(search/'r18-final-replay'),'new_diff':str(search/'runs'),'storage':str(TMP/'r19-user/runs')},'tool_sha256':sha(__file__)}
(OUT/'handoff.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
text='''# Round19 evidence handoff

实际仓库只读；没有执行UI、没有修改源码或历史报告。

- 当前暂存读取HEAD：`HEAD_VALUE`。
- 1190d0e实现时13条完整检查全部rc0；Rust **280 passed / 0 failed / 2 ignored**，Python **71**。后续独立焦点修复之后仍需重新绑定最终检查，不能沿用旧CI。
- 新方案 **33480完整Reference / 83700完整Candidate比较**；真实用户层/偏好 **41850 / 104625**。最终均0差异/0不完整；PositionDag上下文16740/20925明确不支持，不记作成功比较。二者合计188325完整比较，不包含R18历史128340重复计数。
- 固定原R18最终837输入重放预算 **90→6**，原30唯一失败修复28，仅剩两字符模型`n×32`每轮各一次，类型由Expansions改ResidentPaths，未清零；原K10/200000/50000/123544词条保持。
- **速度没有放行**。配对最终查询P95：简拼Lexical27.6448→40.8496ms，add-one24.3197→51.8392ms，WB30.8579→51.9204ms。旧不完整90组不能用作Reference真值。全拼最终查询P95分别1.5580→1.5232、1.4961→1.7733、1.3655→1.5647ms。不能与R18逐键25.25ms或Host/TSF混比。
- 36个paired进程全部rc0、720最终查询；270组双方完整Candidate相同；Full180组完整SearchResult（只移除严格零新字段）也相同。
- 用户层15新进程，OFF/empty/4/512/4096×3模型，各279查询，合计4185。每个层条件只有原6预算失败，总计30，无新失败输入。OFF/empty **837完整SearchResult**逐对象一致。用户层开启不是零开销：简拼P95随规模/模型约24.79–69.25ms，小鹤4096层字符P95达11.22/8.45ms。
- 层delta：empty1224B、4词503136B、512词935704B、4096词3995466B；用户层最大127640条（基础123544保留）。wholeprocess working约185–235MiB，private约184–238MiB，包含基础/组合索引、模型、原件、输出；**不能视为单算法分配或Host128MiB准入**。
- 完整每方案/模型/规模P50/P95/P99/max、资源样本数、工具/输入/二进制SHA见`handoff.json`及原`search/delivery-finish/user-layer-observations.md`。
- 存储16测试通过，保留API缺失、CRLF接受、恢复删原件、损坏reset、旧队列复活等red/green；StorageFull为真实半写后的错误注入，不是填满开发机磁盘；子进程中断不是断电。Windows已本地测试，Linux仍需最终CI。
- `search.rs`交付旧临时源为CRLF（55934B,32793b...），当前Git源为LF（54395B,a3f719...）；逐行源完全相同，不能说字节哈希相同。character.rs及两个新增测试原字节一致。用户层性能的10个search源码与当前全部**精确字节匹配**，包含Arc模型API；较早paired/diff请保留各自来源身份。
- 正式Top1/Top5/MRR仍null，无人工gold、TSF或8h/24h验收，无私人数据归档。

## 归档入口

临时`archive19.py`支持`--plan`，不会运行除显式版本读取外的新测试，不读取AppData，不覆盖不同既有归档。一次性目标为实际repo `docs/reports/input-round19-evidence`。只保留通过CRC/逐字节验证的gzip替代重复大raw；保留所有失败分母与源代码/工具。初次只读估算1014文件、25758568B（约24.6MiB，超过20MiB但低于30MiB）；root后续UI/CI可能新增。历史保全基线cf54：**3767旧路径和256旧Rust测试函数**。最终CI在报告commit之后生成时由外部receipt记录，不通过改报告循环制造新HEAD。
'''.replace('HEAD_VALUE',record['read_head'])
(OUT/'handoff.md').write_text(text,encoding='utf-8')
print(json.dumps({'checks':record['checks'],'source_exact_layer':all(actual_layer_binding.values()),'handoff':str(OUT/'handoff.md')},ensure_ascii=False))
