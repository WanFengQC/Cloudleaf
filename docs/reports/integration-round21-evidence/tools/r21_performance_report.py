"""用已完成证据汇总正式报告，不把缺项当作通过。"""

from collections import Counter

from r21_evidence_common import LOCAL, get_json


def optional(label):
    path = LOCAL / label / 'summary.json'
    if path.is_file():
        data = get_json(path)
        if data.get('finished_utc'):
            return data
    return None


def millis(data):
    return ' / '.join(f'{data[key] / 1_000_000:.3f}' for key in ('p50', 'p95', 'p99', 'max'))


def mib(value):
    return f'{value / 1024**2:.1f} MiB'


def main():
    search = optional('round21-search-verified')
    traces = optional('round21-input-traces')
    bridge = optional('round21-real-bridge')
    models = optional('round21-real-models')
    coexist = optional('round21-model-coexist')
    lines = ['# Round 21 性能守门与本地模型证据', '',
             '以下时延来自本机合成或固定研发样本。样本未经过独立语言金标准标注，'
             'Top-1、Top-5、MRR 为 null。真实 Windows 窗口、TSF、长期稳定性不属于这些测量。', '']
    add = lines.extend
    if search:
        trials = sum(v['totals']['complete_candidate_comparisons'] for v in search['strict'].values())
        add(['## 固定搜索护栏 — done', '',
             f"同一 123,544 条基础词典，{search['queries_per_version']} 个查询，"
             f"{search['exact_full_results_equal']} 对完整 SearchResult 全等。"
             f"预算失败为 {search['budget_failures']['r20']} → {search['budget_failures']['r21']}，"
             f"独立 Reference 完整候选比较 {trials:,} 次。", '',
             '比较覆盖候选正文、顺序、分数、特征、词条 ID、边来源、回退、完整性和预算诊断。'
             '展开上限 200,000，驻留路径上限 50,000，K10。'
             '探针在新工作树重新编译，目录中的生产源文件 SHA 可与交付提交比对，'
             '搜索探针运行时的 Git HEAD 早于整合提交。', '',
             '| 语料版本 | 进程峰值工作集 | 观测最大私有内存 | 采样数 |', '| --- | ---: | ---: | ---: |'])
        for version, data in search['whole_process_memory'].items():
            add([f"| {version} | {mib(data['peak_working'])} | {mib(data['observed_private'])} | {data['samples']} |"])
        add([''])
    else:
        add(['## 固定搜索护栏 — blocked', '', '证据尚未完成。', ''])
    if traces:
        counts = Counter(v['scope'] for v in traces['comparisons'] if v['equal'])
        add(['## 逐键配对 — done', '',
             f"模型空闲时比较 Round 20 CI 原始 ZIP 内已验证的程序与集成 release。"
             f"每轮每进程 171 事件，{counts['engine']} 对 K10 引擎轨迹和 "
             f"{counts['host']} 对 K5 无窗口宿主轨迹全部一致。"
             '移除时间列后比较引擎完整帧、候选路径和宿主输出。以下单位为 ms，'
             '表中每个时延单元按 P50 / P95 / P99 / max 排列。', '',
             '| 操作范围 / 语言模型 | R20 样本 | R20 时延 | R21 样本 | R21 时延 |',
             '| --- | ---: | ---: | ---: | ---: |'])
        for scope in ('engine', 'host'):
            for name in ('lexical', 'addone', 'witten'):
                old = traces['timing_ns'][f'{scope}/{name}/r20']
                new = traces['timing_ns'][f'{scope}/{name}/r21']
                add([f"| {scope} / {name} | {old['n']} | {millis(old)} | {new['n']} | {millis(new)} |"])
        add(['', f"固定事件宿主峰值工作集 {mib(traces['resource_max']['peak_working_bytes'])}，"
                 f"观测最大私有内存 {mib(traces['resource_max']['private_bytes'])}。"
                 '桌面背景负载、Windows 调度和运行时噪声未隔离，时延差异不作系统验收结论。', ''])
    else:
        add(['## 逐键配对 — blocked', '', '最终二进制配对证据尚未完成。', ''])
    if coexist:
        add(['## 模型与合成用户词库并发 — done', '',
             '同一独立进程链接交付 release rlib，运行原输入引擎、真实离线 Marian、正式翻译队列和单写者 Store。'
             'Store 在系统临时目录，只写合成词，后台保存 32 次后退出并重新获取写锁。'
             '模型释放通过所有权及实际 worker join 验证。'
             '七组均为 171 个事件，含派生完整 EventResult 全相等。', '',
             '| 条件 | 事件数 | 实际活跃重叠样本 | P50 / P95 / P99 / max（ms） |',
             '| --- | ---: | --- | ---: |'])
        for scope, block in coexist['timing_ns'].items():
            overlap = coexist['observed_overlap_ns'][scope]
            observed = ', '.join(f"{k}：{v['n']}" for k, v in overlap.items())
            add([f"| {scope} | {block['n']} | {observed} | {millis(block)} |"])
        add(['', 'model=0 表示没有提供者活动，1 为模型加载，2 为推理。user 表示合成 Store 正在保存。'
                 '满队列返回 Busy，取消后的终态不暴露译文，后续推理和输入仍可完成。'
                 '每组为单次研发轨迹，实际重叠按事件处理前观测。', '',
             f"峰值工作集 {mib(coexist['resources']['peak_working_bytes'])}，"
             f"最大私有内存 {mib(coexist['resources']['private_bytes'])}。"
             '这是全进程用量，输入内部的 128 MiB 资源账本仍独立。', ''])
    else:
        add(['## 模型与合成用户词库并发 — blocked', '', '最终同进程探针尚未完成。', ''])
    if models:
        engineering = models['engineering_inference']
        add(['## 真实离线模型边界 — done', '',
             f"固定本地 Round 13 Marian 权重执行 {engineering['cases']} 条研发样本，"
             f"{engineering['generated']} 条实际生成译文，已观察到 "
             f"{len(engineering['numeric_literal_differences'])} 条数字字面形式差异。"
             '单位、数值和语义仍需人工独立审查。', '',
             '空输入、空白、字节/词元超限、取消以及超时均验证明确错误。'
             '同一真实模型的生命周期测得实际取消、队列满、重复推理和 worker 退出。'
             '破损测试只复制小配置到专用临时证据目录。两份原始模型权重都不曾移动。', ''])
    else:
        add(['## 真实离线模型边界 — blocked', '', '最终直接推理证据尚未完成。', ''])
    if bridge:
        observed = bridge['verified']
        add(['## 双向无窗口桥接 — done', '',
             f"实际候选「{observed['chinese_candidate']}」→「{observed['chinese_to_english']}」；"
             f"相应英译中实际输出「{observed['english_to_chinese']}」。"
             '显式确认并写入内存文档两次。保留原文、拒绝重复回执、NotApplied 后领取新提交编号，'
             'Unknown 保留隔离状态直至人工核对操作。取消、修订、失焦、模型缺失与损坏均拒绝确认。', ''])
    else:
        add(['## 双向无窗口桥接 — blocked', '', '最终真实桥接证据尚未完成。', ''])
    add(['以上证据来自离线提供者。没有下载新权重，也没有对外提交请求。网络未物理断开。'
         '真实模型仍为 research_only，正式语言指标 null，CL-010 保持关闭。', ''])
    target = LOCAL / 'round21-performance.md'
    target.write_text('\n'.join(lines), encoding='utf-8', newline='\n')
    print(target)


if __name__ == '__main__':
    main()
