"""将 Round 22 本地实际结果写入交付报告。"""
import json
import re
import statistics
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[1]
local = root / '.local'
reports = root / 'docs/reports'
def read(path):
    return json.loads((local / path).read_text(encoding='utf-8'))
def write(name, lines):
    (reports / name).write_text('\n'.join(lines).rstrip() + '\n', encoding='utf-8', newline='\n')
def fmt(value):
    return f'{value / 1e6:.2f}' if isinstance(value, (int, float)) else '—'
def percentiles(value):
    return ' / '.join(fmt(value.get(k)) for k in ('p50', 'p95', 'p99', 'max')) if value['n'] else '—'
def med(values):
    values = list(values)
    return statistics.median(values) if values else None
def maximum(rows, field):
    return max((row[field] for row in rows if row[field] is not None), default=None)
def mib(value):
    return f'{value / 1024**2:.1f}' if value is not None else '—'
def block(name):
    payload = read(name + '/summary.json')
    assert payload['finished_utc']
    return payload

primary = block('round22-performance-final')
priority = block('round22-priority-study')
search = read('round22-search-fixed/against-round21.json')
checks = read('round22-verified-results.json')
commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
assert len(checks) == 16 and all(row['returncode'] == 0 and row['head'] == commit for row in checks)
assert primary['head'] == commit and not primary['tracked_dirty']
assert len(primary['runs']) == 36 and not primary['errors']
assert all(row['semantics_equal'] and row['samples']['all'] == 171 for row in primary['runs'])
assert search['exact_pairwise_equal'] == 837 and search['mechanical_complete_comparisons'] == 188325
test = (local / 'round21-runs/round22-verified-04/stdout.txt').read_text(encoding='utf-8')
rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', test)
rust = [sum(int(row[i]) for row in rows) for i in range(3)]
py_test = (local / 'round21-runs/round22-verified-08/stderr.txt').read_text(encoding='utf-8')
python = int(re.search(r'Ran (\d+) tests', py_test)[1])
assert rust[1] == 0
native = (root / 'adapters/windows/isolated_translation.rs').read_text(encoding='utf-8')
threads = re.search(r'threads\.unwrap_or\(ResourceThreads::(One|Two|Four)\)', native)[1]
n = {'One': 1, 'Two': 2, 'Four': 4}[threads]
idle = primary['event_timing_ns']['idle-1']['all']['wall']['p95']
active = primary['event_timing_ns'][f'infer-{n}']['inference']['wall']['p95']
if not priority['errors']:
    assert len(priority['runs']) == 18

perf = [
    '# Round 22 工作进程资源与输入性能', '',
    '**partial**。包含 Windows 本机机制复测。端到端桌面验收为 `not-run/user-paused`。', '',
    f'主实验绑定实现提交 `{commit}`。离线工作进程 SHA-256 为 `{primary["identities"]["worker"]["sha256"]}`。'
    '固定词典 123,544 条、词汇模型、原 171 事件、K10、展开 200,000、驻留路径 50,000 及两份获准 Marian 权重。'
    f'环境为 {primary["environment"]["os"]}，{primary["environment"]["cpu"]}。', '',
    '12 个条件各重复 3 次，正序、倒序交替。测量 InputEngine handle 的墙钟、Windows 当前线程 CPU 及两端核心编号。'
    '全量按键和实际推理重叠单独计数。活跃推理仅计入完整落在 worker 发布的推理起止区间内的按键。'
    '加载、取消、词库同时写入和退出独立列报。取消完成表示 IPC 明确取消当前任务，实际计算按检查点停止。', '',
    '| 条件 | 全量 N | 墙钟 P50 / P95 / P99 / max（ms） | 推理重叠 N | 活跃推理 P95（ms） | 加载重叠 N | 同时保存词库 N |',
    '| --- | ---: | --- | ---: | ---: | ---: | ---: |',
]
labels = {
    'idle-1':'未启动 worker', 'ready-1':'已握手、未加载', 'loading-1':'加载中', 'infer-1':'实际推理 1 线程',
    'infer-2':'实际推理 2 线程', 'infer-4':'实际推理 4 线程', 'full-1':'满队列', 'cancel-1':'取消',
    'direction-switch-1':'更换翻译方向', 'store-1':'推理时保存合成词库', 'after-exit-1':'worker 退出后',
    'repeat-release-1':'重新启动、加载与释放',
}
for tag, evidence in primary['event_timing_ns'].items():
    all_wall = evidence['all']['wall']
    wait = evidence['inference']['wall']
    perf.append(f"| {labels[tag]} | {all_wall['n']} | {percentiles(all_wall)} | {wait['n']} | {fmt(wait.get('p95'))} | "
                f"{evidence['loading']['wall']['n']} | {evidence['inference_and_store']['wall']['n']} |")
perf += ['', '| 配置 | 活跃推理 N | 输入 CPU P95（ms） | 墙钟减已计 CPU P95（ms） | 端点核心变更 | 验证 / 加载 / 推理中位数（ms） | 首个结果中位数（ms） | 子进程总线程上限采样 |',
         '| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |']
for threads in (1, 2, 4):
    tag = f'infer-{threads}'
    runs = [row for row in primary['runs'] if row['tag'] == tag]
    metrics = [row['metrics']['main'] for row in runs]
    assert all(row['compute_threads'] == threads and row['resident_models'] == 1 for row in metrics)
    assert len({row['raw'] for row in metrics}) == 1
    timing = primary['event_timing_ns'][tag]['inference']
    phases = ' / '.join(fmt(med(row[k] for row in metrics)) for k in ('verification_ns', 'load_ns', 'inference_ns'))
    perf.append(f"| {threads} | {timing['wall']['n']} | {fmt(timing['cpu']['p95'])} | {fmt(timing['wall_minus_accounted_cpu']['p95'])} | "
                f"{timing['observed_core_changes']} | {phases} | {fmt(med(row['host_request_to_result_ns'] for row in metrics))} | "
                f"{maximum([row['resources']['worker'] for row in runs], 'threads')} |")
perf += ['', '中位数计算使用三轮的逐轮时长。计算池线程由实际 Rayon 调用报告，进程另有协议、读写和调度线程。'
         '子进程限定自己的 RAYON_NUM_THREADS。采样无法区分 GEMM 嵌套并行的每种调用。'
         '线程 CPU 计数有调度粒度，墙钟减 CPU 仅为差值，内存带宽、逐次分配和 ETW 调度归因未独立测量。', '',
         '| 场景 | 宿主最大观测 RSS / 私有内存（MiB） | worker 最大观测 RSS / 私有内存（MiB） | 同时采样总 RSS（MiB） | worker 最大读取量（MiB） |',
         '| --- | --- | --- | ---: | ---: |']
for tag in labels:
    runs = [row['resources'] for row in primary['runs'] if row['tag'] == tag]
    host, worker = ([row[k] for row in runs] for k in ('host', 'worker'))
    mem = lambda values: ' / '.join(mib(maximum(values, key)) for key in ('rss', 'private'))
    perf.append(f"| {labels[tag]} | {mem(host)} | {mem(worker)} | {mib(maximum(runs, 'paired_max_rss'))} | {mib(maximum(worker, 'read_bytes'))} |")
stop = [sample for row in primary['runs'] for sample in row['stop_ns']]
cancel = [sample for row in primary['runs'] for sample in row['cancel_ns']]
codec = [row['codec_ns']['p95'] for row in primary['runs'] if row['codec_ns']['n']]
perf += ['', f'每 50 ms 附近采集进程内存、线程、累计 CPU 和读 I/O。双进程 RSS 仅相加同次样本，'
         f'与 128 MiB 的输入内部分配预算分开。IPC 单进程编解码三轮 P95 为 {", ".join(fmt(v) for v in codec)} ms，'
         f'三轮取消终态分别为 {", ".join(fmt(v) for v in cancel)} ms。退出等待最大 {fmt(max(stop))} ms。'
         '握手、每条请求的队列等待、实际编码、解码累计耗时及生成步数均保留在原始 JSON。'
         '解码单步平均可由解码累计时长除实际步数得到，未采集单步 P95。', '',
         '## Windows 优先级探索 — partial', '',
         f'探索记录 {len(priority["runs"])} 次成功，{len(priority["errors"])} 次失败。使用同一组输入，按轮交替普通调度与低于正常优先级。'
         '只有在父 PID 和子进程身份都吻合时才设置探索进程的优先级，设置结果用 Windows API 回读。'
         '主实验和生产代码使用正常系统调度。探索二进制身份与主实验各自绑定。', '',
         '| 计算线程 / 优先级 | 活跃 N | 输入 P95（ms） | 推理中位数（ms） |', '| --- | ---: | ---: | ---: |']
for tag, measures in priority['event_timing_ns'].items():
    sample = measures['inference']['wall']
    runs = [row for row in priority['runs'] if row['tag'] == tag]
    perf.append(f"| {tag} | {sample['n']} | {fmt(sample.get('p95'))} | {fmt(med(row['metrics']['main']['inference_ns'] for row in runs))} |")
perf += ['', f'隔离宿主默认 {n} 个推理线程，可显式选择 1、2、4。'
         f'主实验无 worker 的输入 P95 为 {fmt(idle)} ms，默认线程活跃推理为 {fmt(active)} ms，'
         f'两者比为 {active/idle:.2f} 倍。目标分别按 5 ms、8 ms 与空闲延迟 1.5 倍核算。'
         '与 Round 21 单进程真实推理的 14.51 ms 配置背景、调度与加载范围存在差异，跨轮数值只作工程参考。', '',
         '原始按键、真实研发译文、队列状态、资源采样和失败均见[证据索引](integration-round22-evidence/index.json)。']
write('integration-round22-performance.md', perf)

state = [
    '# Round 22 原生翻译整合状态', '',
    '**partial**。继续使用 `codex/integration-round21`。Round 21 基线为 `61dbe04ca10c9d175be7ca5fc117d8dc6fb59d39`。'
    f'本轮实现提交为 `{commit}`。最终归档提交、四个精确 push CI 产物及原有分支核对以 Git 外回执为准。', '',
    '| 范围 | 状态 | 结果 |', '| --- | --- | --- |',
    '| 原生隔离宿主 | done | 沿用 `--interactive-input-host` 和实际、投影两个引擎。新增可见方向、捕获高亮候选、英文缓冲区、进度、取消、原文/译文、风险确认和错误状态。正常键位保留。 |',
    '| 提交事务 | done | 双引擎先只读预检，沿用语义比较及宿主文档提交。Applied 只写一次，NotApplied 允许显式重试或保留输入，Unknown 保留核对材料并拒绝重放。译文无学习凭据。 |',
    '| IPC 与真实 worker | done | 继承匿名管道，协议版本 1，128 KiB 帧、16 KiB 输入及输出、现役请求上限 2。后台 I/O，按需启动，绝对同目录程序、握手、请求代次、拒绝迟到或重复结果、有界回收。 |',
    '| 模型 | done | 固定授权本地权重双向推理。方向仅驻留一份，复用同方向。选中权重始终经完整 SHA 校验。缺失、损坏、截断、超时和取消有明确错误。发行保持 research_only。 |',
    '| 语义保护 | done | 金额、数字、日期、时间、单位与显式术语结构化比较；保留原始译文，风险确认绑定当前任务及结果版本。 |',
    '| 性能 | partial | 已有同机配对复测及进程资源记录，活跃推理延迟仍有尾部开销。 |',
    '| 在线路径 | partial | 原翻译面板保留显式授权与系统凭据。隔离输入宿主只使用离线 worker。真实在线验收未执行。 |',
    '| 质量与未整合能力 | blocked | 独立 gold、正式语义发布、PR #4 长期准入仍分别保留在原方向。正式 Top-1/Top-5/MRR 均为 null。 |',
    '| 真实 Windows 窗口 | not-run/user-paused | 有原生生产代码和无窗口回归。人工验收矩阵 16 项全待执行。 |',
    '| TSF 与长期压力 | not-run | TSF 注册、激活、安装、卸载、默认输入法更改、8h/24h 与物理断网验证未执行。 |', '',
    '## 本地回归 — done', '',
    f'固定 Rust 1.90.0、Edition 2024，完整 16 项命令返回 0。Rust 全目标 {rust[0]} 通过、{rust[2]} 忽略，Python {python} 通过。'
    '格式、严格 Clippy、文档测试、locked/offline release、两个许可和 RustSec 审计、九份 vendor、基准与示例工具均通过。'
    '真实模型测试按授权材料独立启用，区分 CI 无权重结果。', '',
    '搜索 837 对完整结果全等，预算失败 0/837，188,325 次机械参考完整候选比较无差异。'
    '跨键缓存默认关闭。合成用户词库写入、重新获取写锁及原有学习测试继续通过。', '',
    '本轮保留 IPC 解析拒绝未知字段、重复回收误报、Escape 后重试状态滞留及术语日期边界的首次失败证据。完整首轮查到 Windows 隐藏焦点夹具错误解读 SetFocus 返回的旧句柄，以及许可诊断措辞兼容问题。夹具现在直接断言目标焦点，原有行为断言保留，许可范围保持严格。[SetFocus 契约](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setfocus)'
    '依赖锁仅新增本地包与已在锁内的轻量序列化边。输入侧 Cargo 闭包验证未引入模型加载、Candle、Tokio 或联网后端。', '',
    '## 交付与后续门禁', '',
    '本轮设计、人工验收矩阵、原始命令和 SHA 证据保留。最终 CI 以回执的精确提交为准。'
    'main、原六条开发线、五个 Draft 和 CL-010 沿用原边界。', '',
    'Round 23 优先分离调度等待、输入搜索与张量计算的并发开销，复核完整性检查在独立样本下的误报/漏报。'
    '获得明确授权后，按矩阵执行真实窗口交互并验证私有数据与异常恢复。', '',
    '[性能报告](integration-round22-performance.md) · [关键语义](integration-round22-translation-integrity.md) · '
    '[人工矩阵](../testing/round22-native-translation-manual.md) · [证据索引](integration-round22-evidence/index.json)',
]
write('integration-round22-status.md', state)
print(json.dumps({'implementation': commit, 'rust': rust, 'python': python, 'performance_runs': len(primary['runs']), 'default_threads': n,
                  'idle_p95_ns': idle, 'default_inference_p95_ns': active}, ensure_ascii=False))

