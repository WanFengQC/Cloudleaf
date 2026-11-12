from pathlib import Path
import hashlib
import json
import re

out = Path(__file__).resolve().parent
perf = out.parent / 'r18-perf-final'
root = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
load = lambda p: json.loads(p.read_text(encoding='utf-8'))
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
s = load(perf / 'summary.json')
r = load(perf / 'resources-summary.json')
d = load(out.parent / 'r18-scheme-diff/final-validation.json')
v = load(root / '.local/round18-verified-results.json')
rust = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', (root / '.local/round18-runs/verified-04/stdout.txt').read_text(encoding='utf-8'))
assert len(v) == 13 and all(x['returncode'] == 0 for x in v)
assert all(x['returncode'] == 0 for x in s['runs'])
assert all(x['returncode'] == 0 for x in r['runs'])
changed = [k for k, h in s['source_after'].items() if sha(root / k) != h]
summary = {
    'implementation_head': s['implementation_head'],
    'root_current_head_at_check': v[0]['head'],
    'perf_source_blobs_still_match_current': not changed,
    'source_blob_mismatches': changed,
    'latency_profile_processes': len(s['runs']),
    'resource_processes': len(r['runs']),
    'all_process_returncodes_zero': True,
    'default_pairs': len(s['default_pairs']),
    'all_default_pairs_equal_except_new_zero_None_fields': all(x['full_retained_output_equal_after_only_zero_new_fields'] for x in s['default_pairs']),
    'final_profile_query_count': 837,
    'unique_final_budget_cases': len(s['budget_observations']),
    'repeated_final_profile_budget_cases': 3 * len(s['budget_observations']),
    'budget_cases': s['budget_observations'],
    'default_performance_ns': s['default_performance_ns'],
    'scheme_event_performance_ns': s['scheme_event_performance_ns'],
    'resource_measurements': [{k: x[k] for k in ['scheme','model','sample_count','seconds','sampled_working_max','sampled_private_max','OS_peak_working_max']} for x in r['runs']],
    'new_correctness_reference_queries': d['new_complete_reference_queries'],
    'new_correctness_comparisons': d['new_complete_candidate_comparisons'],
    'old_correctness_reference_queries': d['old_complete_reference_queries'],
    'old_correctness_comparisons': d['old_complete_candidate_comparisons'],
    'initial_raw_correctness_failures': d['raw_failure_count'],
    'final_correctness_failures': d['final_failures'],
    'final_correctness_incomplete': d['final_incomplete'],
    'correctness_beam_pruned_queries': d['final_beam_pruned_queries'],
    'local_check_count': len(v),
    'all_local_check_rc_zero': True,
    'rust_counts': dict(zip(['passed','failed','ignored'], [sum(int(t[i]) for t in rust) for i in range(3)])),
    'python_tests': 71,
    'formal_metrics': s['formal_metrics'],
    'binary': s['binary'],
    'runtime': s['runtime'],
    'models': s['models'],
    'evidence_files': {},
}
for p in [perf/'summary.json', perf/'resources-summary.json', perf/'observations.md', perf/'phases.md', perf/'index.json', perf/'compression.json', perf/'run.py', perf/'summarize.py', perf/'resources.py', out.parent/'r18-final-audit/preservation.json', out.parent/'r18-final-audit/review.md', out.parent/'r18-scheme-diff/final-validation.json', root/'.local/round18-verified-results.json']:
    summary['evidence_files'][str(p)] = {'bytes': p.stat().st_size, 'sha256': sha(p)}
(out/'handoff.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2)+'\n',encoding='utf-8')

lines = ['# Round18 只读最终证据复核摘录', '', '本文件供主代理整合，未修改实际工作区、未执行UI、未提交、未运行新增测试。', '',
f"性能实现提交 `{s['implementation_head']}`；当前13项完整回归绑定 `{v[0]['head']}`。性能记录的17个源码/锁文件哈希逐项仍与当前字节相同；Windows按钮变更不在这组核心性能输入内。", '',
'## 完整回归与差分', '',
'- 最终本地13条命令均rc=0；Rust 254 passed、0 failed、2 ignored；Python 71 passed。原始命令、返回码、时间、源码/日志哈希位于`.local/round18-runs/verified-01..13/result.json`及相邻stdout/stderr。',
'- 新方案：1395输入×4方案×6ranker，33,480个完整Reference查询、83,700个完整Candidate比较，最终0差异/0不完整/0Beam剪枝；PositionDag上下文不支持16,740次为明确拒绝，不能计为完成比较。',
'- 默认字符模型旧入口：22,320个完整Reference查询、44,640个完整Candidate比较，最终0差异/0不完整/0Beam剪枝。',
'- 原始592个差异全部保留，不能报告“首次即通过”。初轮另有49图预算、294Reference不完整、735优化比较不完整。后续修复的是解释候选完整edge provenance确定性及纠错可达性，而非删断言。',
'- `r18-final-audit/preservation.json`：2793个既有报告/数据/测试/许可路径保留，230个旧Rust测试函数未删除；必要旧测试只加Budget穷尽分支。', '',
'## 相同默认事件配对', '',
'每次171个事件×5个新进程=每模型每版本855个样本；15组输出比较全部一致。只移除严格零值/None的新字段，正文、ID、路径、评分、统计不归一化。无warmup丢弃。', '',
'|模型|旧P95 ms|新P95 ms|变化|旧P99 ms|新P99 ms|', '|---|---:|---:|---:|---:|---:|']
for model in ['lexical','addone','witten']:
    old=s['default_performance_ns']['old-'+model]; new=s['default_performance_ns']['new-'+model]
    lines.append(f"|{model}|{old['p95']/1e6:.4f}|{new['p95']/1e6:.4f}|{(new['p95']/old['p95']-1)*100:+.2f}%|{old['p99']/1e6:.4f}|{new['p99']/1e6:.4f}|")
lines += ['', '普通桌面研发测量，可能与轻量自有窗口操作并发；不是专用低延迟验收。字符模型P95约+7.85%/+6.73%，不得声称零开销、全面提速或严格5ms门禁通过。', '',
'## 新方案逐键延迟', '', '|方案/模型|n|P50 ms|P95 ms|P99 ms|max ms|', '|---|---:|---:|---:|---:|---:|']
for k,x in s['scheme_event_performance_ns'].items():
    lines.append(f"|{k}|{x['n']}|{x['p50']/1e6:.4f}|{x['p95']/1e6:.4f}|{x['p99']/1e6:.4f}|{x['max']/1e6:.4f}|")
lines += ['', '全部120个latency/profile进程rc0不等于全部查询成功：837个最终profile查询中90次预算失败，30个唯一方案/模型/案例×3轮；均为简拼+addone/WittenBell，分别15/20案例，Expansions=200000。InputEngine遇预算不提供局部选择，原文保留。中间EVENT只打印error，预算分母仅指最终profile输入。',
'', '阶段观测必须单列：plain query和profile query结果相同，但profile包含计时开销；scoring/bucket统计嵌套在traversal内，不能把所有阶段相加。简拼主要耗时在遍历/评分，交集其次，音节解释通常更小。具体全表见`r18-perf-final/phases.md`。', '',
'## 进程资源', '', '下表是独立15个新进程、50ms采样及OS峰值。包含词典/模型原件读取缓冲、不可变索引、搜索、帧和诊断。不是单算法分配，不是Host/COM，不是128MiB内部预算的RSS上界。', '',
'|方案/模型|采样数|采样working最大MiB|采样private最大MiB|OS working峰值MiB|', '|---|---:|---:|---:|---:|']
for x in r['runs']:
    lines.append(f"|{x['scheme']}/{x['model']}|{x['sample_count']}|{x['sampled_working_max']/1048576:.2f}|{x['sampled_private_max']/1048576:.2f}|{x['OS_peak_working_max']/1048576:.2f}|")
lines += ['', '## 真实候选观察和未放行项', '',
'- 简拼lexical：zgr首选撰稿人、中国人第3；bjdx首选北京大学；混合zhonggr首选中国人；nh首选内涵、你好不在Top10。不得把非空率或这些例子当中文准确率。',
'- 小鹤nihc、vsgorf、bwjkdaxt分别形成你好、中国人、北京大学；lo碰撞保留lo/luo音节路径。完整Top10、Features、ID、原始证据保留在profile stdout。',
'- 正式Top1/Top5/MRR全null，独立gold未交付。新方案仍opt-in，不替换默认核心。简拼字符组合预算失败、P95超过实时目标；当前仅研发partial。',
'- 微软/自然码未核实完整原始规则而未实现；小鹤+纠错拒绝；模糊只从完整精确弧扩展；物理键盘当前ASCII/v，纯API的ü能力不能冒充实际Unicode键盘验收。',
'- 无TSF、真实跨应用输入法、长期压力、学习持久化或正式语言质量验收。主代理需把实际窗口按钮和Unknown测试另行绑定各自二进制SHA。', '',
'## 原始证据和身份', '',
'`r18-perf-final/index.json`列全部文件SHA；120个run的command/result/stdout/stderr保留；stdout原件和gzip均在，`compression.json`验证CRC和解压后逐字节SHA。资源日志另在resources。差分3组压缩日志均有CRC及原始字节SHA。', '',
'- 词典runtime SHA `'+s['runtime']['sha256']+'`。',
f"- 旧input_trace二进制 `{s['binary']['old']['sha256']}`；新 `{s['binary']['new']['sha256']}`；scheme_trace `{s['binary']['scheme']['sha256']}`。",
f"- 性能工具run.py `{sha(perf/'run.py')}`；summarize.py `{sha(perf/'summarize.py')}`；resources.py `{sha(perf/'resources.py')}`。",
'', '所有完整绝对路径、工具/数据/二进制SHA和原始指标见相邻handoff.json。本摘录未核对最终push CI；需要主代理单独完成精确新HEAD两平台CI。', '']
(out/'draft.md').write_text('\n'.join(lines),encoding='utf-8')
print(json.dumps({'draft': str(out/'draft.md'), 'json': str(out/'handoff.json'), 'rust': summary['rust_counts'], 'source_mismatches': changed},ensure_ascii=False))
