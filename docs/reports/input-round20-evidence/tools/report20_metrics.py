"""从已完成的原始报告生成分位数, 各比较口径分别列出。"""
import json
import re
from pathlib import Path

root = Path(__file__).resolve().parents[1]
local = root / '.local'

def load(path):
    return json.loads(path.read_text(encoding='utf-8'))

def times(row):
    return ' / '.join(f"{row[k] / 1_000_000:.3f}" for k in ('p50', 'p95', 'p99', 'max'))

def report_rows(title, names, group, key):
    lines.extend(['', title, '', '| 条件 | 每版样本 | Round 19 | Round 20 |', '| --- | ---: | ---: | ---: |'])
    for name in names:
        old, new = (group[key(version, name)] for version in ('old', 'new'))
        assert old['n'] == new['n']
        lines.append(f"| {name} | {old['n']} | {times(old)} | {times(new)} |")

search_dir = Path('C:/Users/EDY/.codex/tmp/r20-search-delivery')
summary = next(x for p in search_dir.rglob('summary.json') if isinstance(x := load(p), dict) and 'complete_pairs' in x and 'metrics_ns' in x)
paired = load(local / 'round20-engine-host-performance/summary.json')
assert paired['head'] == summary.get('final_repo_head', paired['head'])
assert paired['source_unchanged'] and paired['binaries_unchanged']
assert all(p['same'] for p in paired['engine_pairs'])
assert all(p['same'] for p in paired['host_pairs'])
assert not summary['complete_mismatches']
old_fail, new_fail = (summary['totals'][v]['budget_failures'] for v in ('r19', 'r20'))
assert summary['totals']['r19']['queries'] == summary['totals']['r20']['queries'] == 837
lines = [f'同机交替执行固定 837 个最终查询，Round 19 预算失败 {old_fail} 次，Round 20 {new_fail} 次。'
         f"双方完整的 {summary['complete_pairs']} 对查询，候选对象及来源无差异。工作计数随优化改变，逐行保留。",
         '', '最终搜索源码另完成 188,325 次机械完整候选比较，其中固定机械格 83,700 次，含用户层和偏好的格 104,625 次，均无差异。参考搜索未完成的样本不列入成立的比较次数。',
         '', '所有表中各单元依次为 P50 / P95 / P99 / 最大值，单位毫秒。最终查询是建格加搜索时间，逐键涉及完整事件处理。各组保留所有样本。',
         '', '| 固定最终查询的简拼组 | 每版样本 | Round 19 | Round 20 |', '| --- | ---: | ---: | ---: |']
for kind in ('lexical', 'addone', 'witten'):
    old, new = (summary['metrics_ns'][f'{kind}/abbreviated/{v}']['total'] for v in ('r19', 'r20'))
    lines.append(f"| {kind} | {old['n']} | {times(old)} | {times(new)} |")
lines.extend(['', '只看双方都完成的查询，add-one 与 Witten–Bell 的简拼 P95 如下。Lexical 工作计数相同，它的变化属于同机时延波动，无法归因于字符剪枝。',
              '', '| 模型 | 双方完整对数 | Round 19 P95 | Round 20 P95 |', '| --- | ---: | ---: | ---: |'])
for kind in ('addone', 'witten'):
    old, new = (summary['metrics_ns'][f'{kind}/abbreviated/{v}']['total_both_complete'] for v in ('r19', 'r20'))
    lines.append(f"| {kind} | {old['n']} | {old['p95']/1e6:.3f} | {new['p95']/1e6:.3f} |")
lines.extend(['', '原预算失败案例仍保留在总分母，逐次展开与驻留计数如下。', '',
              '| 模型 / 轮次 | Round 19 展开 / 驻留 | Round 20 展开 / 驻留 | 结果 |', '| --- | ---: | ---: | --- |'])
for row in summary['budget_rows']:
    a,b = (row[v] for v in ('r19','r20'))
    budget = '完整' if b['budget'] == 'None' else b['budget']
    lines.append(f"| {row['kind']} / {row['iteration']} | {a['expansions']} / {a['resident']} | {b['expansions']} / {b['resident']} | {budget} |")
lines.extend(['', '每个方案及模型的分位数、成功对、完整候选数、工作计数与整进程内存采样见[搜索专项证据](input-round20-evidence/index.json)中的 `search/`。'])
memory = summary['process_memory']
lines.extend(['', f"搜索进程采样的峰值工作集为 {memory['r19']['maximum_working_bytes']/2**20:.1f} → {memory['r20']['maximum_working_bytes']/2**20:.1f} MiB，观测最大私有字节为 {memory['r19']['maximum_observed_private_bytes']/2**20:.1f} → {memory['r20']['maximum_observed_private_bytes']/2**20:.1f} MiB。"])
report_rows('默认全拼 Engine K10，同一 171 事件、每模型每版五个进程。逐事件候选、来源、评分和诊断共 15 组全等。',
            ('lexical','addone','witten'), paired['engine_performance_ns'], lambda v,n:f'{v}-{n}')
host = {k:v['total'] for k,v in paired['host_performance_ns'].items()}
report_rows('无窗口 Host K5，两个引擎与内存写入端。每模型每版三个进程，逐事件的标识和 revision、按案例结束输出的帧和诊断共 12 组全等。',
            ('lexical','addone','witten','supplement-addone'), host, lambda v,n:f'{v}-{n}')
mem = [m for run in paired['runs'] for m in run['resource_samples']]
lines.extend(['', f"逐键共 {len(paired['runs'])} 个进程，50ms 间隔采样 {len(mem)} 次。观测最大工作集 {max(m['OS_peak_working'] for m in mem)/2**20:.1f}MiB、私有字节 {max(m['private'] for m in mem)/2**20:.1f}MiB，包含完整词典与模型预载。",
              '', '本机研发查询仍存在高于 5ms 的 P95，性能门禁维持 partial。最终查询比较和逐键对照均为机械或研发输入，正式准确率保持 null。'])

checks = load(local / 'round20-full-results.json')
assert len(checks) == 13 and all(c['returncode'] == 0 and c['head'] == paired['head'] for c in checks)
test = (local / 'round20-runs/full-04/stdout.txt').read_text(encoding='utf-8')
rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', test)
passed, failed, ignored = (sum(int(row[i]) for row in rows) for i in range(3))
python_count = re.search(r'Ran (\d+) tests', (local / 'round20-runs/full-08/stderr.txt').read_text(encoding='utf-8'))[1]
lines.extend(['', f'固定实现的 13 项回归全部返回 0，Rust {passed} 通过、{failed} 失败、{ignored} 忽略，Python {python_count} 通过。'
              '覆盖格式与 diff、Clippy、全目标及文档测试、release、工作区审计、Python、依赖许可与 RustSec、基准、算法评估和两组 examples。'
              '逐命令和返回码见[检查记录](input-round20-evidence/round20-full-results.json)，失败反例原文、报告、数据与构建身份见[证据索引](input-round20-evidence/index.json)。'])

path = root / 'docs/reports/input-round20-status.md'
report = path.read_text(encoding='utf-8')
marker = '<!-- ROUND20_MEASUREMENTS -->'
assert report.count(marker) == 1
path.write_text(report.replace(marker, '\n'.join(lines)), encoding='utf-8', newline='\n')
print(json.dumps({'matched_queries': summary['complete_pairs'], 'budget_failures': [old_fail, new_fail],
                  'engine_pairs': len(paired['engine_pairs']), 'host_pairs': len(paired['host_pairs']),
                  'rust_passed': passed, 'rust_ignored': ignored, 'python_passed': int(python_count)}))
