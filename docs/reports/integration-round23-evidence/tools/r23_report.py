"""生成不包含训练句子或逐例候选的 Round 23 报告。"""
import json
import re
import subprocess
from pathlib import Path

root = Path.cwd()
local = root / '.local'
report = root / 'docs/reports'
impl = subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip()
SCOPE = {'round16': 'Round 16 Tatoeba dev', 'round17': 'Round 17 新闻 dev',
         'round18': 'Round 18 输入方案', 'round20': 'Round 20 固定查询去重视图',
         'round23_frozen': 'Round 23 冻结研发验证'}
engines = {
    'base-structural': '固定 123,544 词条，结构先召回',
    'base-character': '固定 123,544 词条，字符先召回',
    'supplement-character': '补充 127,354 词条，字符先召回',
}

def read(name):
    return json.loads((local / name).read_text(encoding='utf-8'))
def write(name, rows):
    (report / name).write_text('\n'.join(rows).rstrip() + '\n', encoding='utf-8', newline='\n')
def milliseconds(value):
    return f'{value / 1e6:.2f}' if value is not None else '—'
def quantile(stats):
    return ' / '.join(milliseconds(stats.get(key)) for key in ('p50','p95','p99','max'))
def rows_json(name,data):
    (local/'round23-public'/name).write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')

(local/'round23-public').mkdir(exist_ok=True)
provider = read('round23-word-context/verified-a/receipt.json')
frozen = read('round23-word-context/frozen-base/receipt.json')
semantic = read('round23-semantic/query-proof-final.json')
numbers = read('round23-search-replay/summary.json')
assert numbers['exact_full_results_equal'] == 837 and not numbers['budget_failures']['r21']
checks = read('round23-verified-results.json')
assert len(checks) == 16 and all(row['returncode'] == 0 and row['head'] == impl for row in checks), checks
rust_results = (local/'round21-runs/round23-verified-04/stdout.txt').read_text(encoding='utf-8')
counts = [0,0,0]
for match in re.finditer(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',rust_results):
    counts = [value+int(match[i]) for i,value in enumerate(counts,1)]
python_count = int(re.search(r'Ran (\d+) tests',(local/'round21-runs/round23-verified-08/stderr.txt').read_text(encoding='utf-8'))[1])
print('tests:',counts,python_count)
rank_studies = {name:read(f'round23-language-{name}/summary.json') for name in engines}
rows_json('language-observations.json', rank_studies)
rows_json('exact-search-identity.json', {k:v for k,v in numbers.items() if k != 'pairs'})
ranking = ['# Round 23 中文排序研发对照', '',
           '正式 Top-1、Top-5、MRR 为 `null`。表中分数衡量与已知来源正文一致的程度。拼音由历史字典或研发轨迹提供，不能代表人工输入意图。', '',
           '## 定点模型', '',
           '词表由实际训练表面构建，多个读音词条共享观测，同一表面的出现次数不会增加词条原本的频率。未观测词映射到 UNK，控制验收时仍记录 unknown。'
           '一元概率为 (count(w)+1)/(全部入边计数+词表大小)。相邻模型采用 (count(p,w)+8×P(w))/(count(p)+8)。'
           'BOS、EOS 参与相邻模型，句内词语项用路径转移均值，混合为词语与字符定点均值。每单位是一比特的 1/1024。'
           '全序先比较拼音解释代价（含纠错），再比较可信用户优先级、结构加语言分值和原池顺序。每个研发候选的内部日志包含词条 ID、来源边、子分项与池位置。', '',
           '最多重排 50 个已去重路径；保留召回上限和不完整预算结果。词语历史只在完整路径上读取，因此没有合并不同前词状态。'
           '旧模式保持原搜索路径。小型参考与优化重排比较 1,782 个完整候选排序与分解。', '',
           '## 各组分母与召回', '',
           '| 资源与召回 | 样本组 | 样本数 | 有正文 | 图可达 | 池含正文 | 池完整 |',
           '| --- | --- | ---: | ---: | ---: | ---: | ---: |']
for name, item in rank_studies.items():
    for group,metrics in item['groups'].items():
        ranking.append(f"| {engines[name]} | {SCOPE[group]} | {metrics['cases']} | {metrics['expected_count']} | {metrics['reachability'].get('yes',0)} | {metrics['pool_recall']} | {metrics['pool_complete']} |")
ranking += ['', '冻结基础组含 15 项不可映射，覆盖全组 40 项分母。Round 18、20 查询原本没有期待正文，排序指标保持空白。'
            '连续按键、原文与并发性能使用完整 Round 22 的 171 事件，结果见性能报告。', '',
            '| 召回池 | 研发组 | 模式 | 首选 / Top-3 / Top-5 / Top-10 | 正文分母 | MRR | 不完整 / 不可执行 |',
            '| --- | --- | --- | --- | ---: | ---: | --- |']
for name,item in rank_studies.items():
    for group in ('round16','round17','round23_frozen'):
        metrics=item['groups'][group]
        for mode,values in metrics['modes'].items():
            hits=' / '.join(str(v) for v in values['source_match_counts'].values())
            rr=values['reciprocal_rank']
            ranking.append(f"| {name} | {SCOPE[group]} | {mode} | {hits} | {values['with_expected']} | {rr:.4f} | {values['incomplete']} / {values['not_run_or_error']} |")
ranking += ['', '## 退化与失败', '',
            '以旧字符 Witten–Bell 为对照。以下比较保留同分母的无候选和预算失败。在选型上保留旧默认。', '',
            '| 召回池 | 研发组 | 新模式 | 改善 | 退化 | 无正文匹配 | 有正文但低位 |',
            '| --- | --- | --- | ---: | ---: | ---: | ---: |']
for name,item in rank_studies.items():
    for group in ('round16','round17','round23_frozen'):
        for mode,values in item['groups'][group]['changes_from_char_witten'].items():
            ranking.append(f"| {name} | {SCOPE[group]} | {mode} | {len(values.get('improved',[]))} | {len(values.get('regressed',[]))} | {len(values.get('missing',[]))} | {len(values.get('lower',[]))} |")
ranking += ['', '每个改善、退化与低位案例的 ID 和名次见[脱敏对照](integration-round23-evidence/round23-public/language-observations.json)。'
            '原始来源句、候选和打分回放留在 Git 外的 `.local/round23-language-*`。'
            '新词模型的 1,782 次参考对照仅验证机械行为。固定旧搜索 837 组与 188,325 次机械比较仍分别记录。',
            '', 'PR2 的正式独立质量协议继续采用家族 70/15/15 与 heldout 至少 2,000 家族。作者、两名独立审核者、仲裁、独立保管和真实使用授权未齐之前，正式指标保持空值。'
            '24 个不预填答案的待标注任务位见[人工任务包](../acceptance/round23-ime-annotation/README.md)。']
write('integration-round23-language-quality.md',ranking)
rows_json('word-base-receipt.json',provider)
rows_json('frozen-base-receipt.json',frozen)
rows_json('semantic-query-proof.json',semantic)
write('integration-round23-data-resources.md', [
    '# Round 23 可追溯语言资源', '',
    '**partial**。现用固定基础词典 123,544 条保留，生产词频均为 unknown。训练记录、离线模型和语义投影留在 Git 外。', '',
    '| 来源 | 固定材料与用途 | 准入 |',
    '| --- | --- | --- |',
    '| CC-CEDICT | 2026-06-04 固定镜像，ZIP SHA-256 `fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`，双语原始释义，词典身份独立 | 既有许可范围保留，CC BY-SA 4.0 要求署名、变更说明及相同方式共享。生产义项、关系和人工语义审核仍待交付。[MDBG](https://www.mdbg.net/chinese/dictionary?page=cc-cedict) |',
    '| Unihan 17.0.0 | SHA-256 `f7a48b2b545acfaa77b2d607ae28747404ce02baefee16396c5d2d7a8ef34b5e`，字符读音与原始英文释义 | 保留 Unicode 3.0 许可与原审批范围。[Unicode](https://www.unicode.org/license.txt) |',
    '| Tatoeba | 2026-10-31 固定周导出，原始 bz2 `374dc649bd61ed5597c58ad9f098edb831e7bbe4ea0831f82947bec1fe5c8bb8`，兼有原创与翻译句 | CC BY 2.0 FR 研究。当前所有者不能自动作原始作者；本轮生产与再分发暂不放行。[Tatoeba](https://tatoeba.org/en/terms_of_use) |',
    '| 中文维基新闻 | 固定文章与各来源版本由既有锁保留，仅作新闻开发对照 | 历史页面许可与归属逐页登记；训练与冻结工具不从开放页面补抓内容。[维基新闻版权信息](https://zh.wikinews.org/wiki/Wikinews:%E7%89%88%E6%9D%83%E4%BF%A1%E6%81%AF) |',
    '| 中文 Wiktionary | 固定不完整前缀研究来源，旧版隔离状态沿用 | 原件不完整且发布权限未通过。 |',
    '| Google Books Ngrams | 固定中文切片 508,674 行、19,074,163 个有效三元统计记录、464 行分母，执行原件 SHA 和 gzip 完整校验 | Google 公开派生统计许可与源书权利分别记录。既有词汇模型属历史研究对照，不用作本轮真实频率的分母。[Google 数据导出](https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/chi_sim-1-ngrams_exports.html) |',
    '| 中文 Wikipedia | 固定自然数条目 rev 94816134、SHA `56e173a1f64ee94c13e1061e3cc970ebe3153e75f8e8cffc50c8293073392f12` | 验证原件后返回准入 2，权利、作者链及分词协议未解决。 |', '',
    '## 真实研究观测', '',
    '运行 `scripts/build_word_context.py` 及 `scripts/freeze_word_context_validation.py`。两份干净构建的 TSV 和回执逐字节一致。'
    '保留 84,959 片段与 65,615 个关联家族，全球来源家族仍只有 1。剔除相同训练片段 1,768 条和 10 条跨原划分完全重复。'
    '词语 541,722 个，含 unknown 102,400，观察相邻关系 626,681 次。当前只有历史 Han 片段用于训练，原文本仍保留其它字符，因此本轮未证明中英混合训练覆盖。', '',
    '实际相邻／一元表 172,549 行，2,619,128 字节，SHA-256 `f8062a4a8e456135840b9df51857c84e3ea3f16f59ced4f9dc85a5bd19fade3a`。'
    '21,974/123,544 词条共享观察表面，101,570（82.21%）未观察。观察次数与表面词相关，未作为同字形多个读音的独立观察，也未写回生产词典。'
    '正式 token frequency、document frequency、Zipf 均为 null。未取得合规的多域现代统计语料。', '',
    '补充词典完整重训练触及 200,000 行上限并被拒绝。固定基线字节前缀校验后，只进行同观测数据的显式重绑，新增词表面仍 unknown。'
    '冻结 40 个内部连接家族，基础与补充分别可映射 25 和 40 条。本语料与训练共享同一全球来源家族，近重复核验不构成独立的跨域 gold。', '',
    '## 实际语义查询', '',
    '导入 PR3 的发布、验证和恢复工具，旧词典与现有生产审批保持。固定研究选样有 34 个词条、78 条原始释义、235 项发布缺件。'
    'Release 可按稳定词条 ID、中文词头、英文精确原始释义或英文词项检索，返回原始来源及多义项。'
    '真实 `nihao` 候选的注释不改变输入帧或学习，Backspace 后拒绝旧请求。'
    '英文词项匹配只返回原始释义。已审核有向关系、POS 和短语关系依然缺失。生产验证和激活均返回 2，未创建发布目录。', '',
    '机器可读[统计回执](integration-round23-evidence/round23-public/word-base-receipt.json)与[语义回执](integration-round23-evidence/round23-public/semantic-query-proof.json)记录资源身份。'
])
perf = read('round23-input-perf/summary.json')
assert not perf['errors'] and len(perf['runs']) == 12
short = [{**row, 'worker_metrics':{key:{name:value for name,value in sample.items() if name != 'raw'} for key,sample in row['worker_metrics'].items()}} for row in perf['runs']]
rows_json('input-performance.json', {**perf,'runs':short})
lines = ['# Round 23 语言模型与输入性能', '',
         '**partial**。仅 Windows 无窗口机制，沿用一线程真实离线 worker 与固定 171 按键轨迹。'
         '实际推理窗口按同机时间戳筛选。按模式重复结果一致，实验新模式的输出允许不同于旧排序。', '',
         '| 评分 / worker | 样本数 | 墙钟 P50 / P95 / P99 / max（毫秒） | 周期 P50 / P95 / P99 / max |',
         '| --- | ---: | --- | --- |']
for name,values in perf['wall_ns'].items():
    cpu=perf['thread_cycles'][name]
    lines.append(f"| {name} | {values['n']} | {quantile(values)} | {' / '.join(str(cpu[v]) for v in ('p50','p95','p99','max'))} |")
lines += ['', '使用 QueryThreadCycleTime 记录线程周期。周期不换算成毫秒，也不据此推断 ETW 调度等待。本轮未采集 ETW 或 Linux 线程周期。'
          '旧 128 MiB 输入内部账本独立，研究模式的语言索引上限 16 MiB。进程工作集和私有内存见采样回执。'
          '新模型不可默认启用。正式 5 ms P95、10 ms P99 门槛按实际负载继续验收。[Microsoft Win32 周期计数](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-querythreadcycletime)', '',
          '| 词典 / 召回 | 配置 | 组 | 搜索 P50 / P95 / P99 / max（毫秒） | 峰值活路径 |',
          '| --- | --- | --- | --- | ---: |']
for tag,item in rank_studies.items():
    for group,stats in item['groups'].items():
        for mode in ('lexical','character-witten','word','mixed'):
            row=stats['modes'][mode]
            lines.append(f"| {tag} | {mode} | {SCOPE[group]} | {quantile(row['query_ns'])} | {row['max_resident_paths']} |")
lines += ['', '输入原始样本、缓存及候选计时均来自开发机。单次搜索分配与 GEMM 内部逐任务并行规模没有独立硬件剖析。',
          '[性能采样回执](integration-round23-evidence/round23-public/input-performance.json)']
write('integration-round23-performance.md',lines)
write('integration-round23-status.md', [
    '# Round 23 语言资源与排序整合', '',
    '**partial**。仍在 `codex/integration-round21`。Round 22 基线 `d0ddf833a5c60bd661877946d080b97a9e6cf5c9`。'
    f'实施代码与本地验证绑定 `{impl}`。最终交付提交与其 push CI、产物校验单另见 Git 外最终回执。', '',
    '| 能力 | 状态 | 核对结果 |','| --- | --- | --- |',
    '| PR2 评测与人工任务 | done | 导入已有路径，真实 Release 执行机械、固定词典、阶段、资源、限制参考和三道正式就绪门禁。24 个空白标注位可交接。 |',
    '| PR3 语义资源与发布 | partial | 采用同一发布、恢复和严格审批。真实研究词语释义及候选注释已执行，正式 POS、有向关系、短语对齐及数据批准未齐。 |',
    '| 真词语统计与 Rust 重排 | partial | 冻结来源和可复现 TSV，显式研究快照；完整差异包括真实退化。保留现默认引擎、输入完整性和回退。 |',
    '| 旧搜索与候选兼容 | done | 837 组完整结果一致，预算失败 0，188,325 次旧机械候选对照无差异。新参考比较 1,782 项。 |',
    '| 实际翻译质量 | partial | 用同一固定 Marian 双向生成 17 条，12 条受控完整性对照，保留漏报及保守提示。 |',
    '| 独立正式评估与发版 | blocked | 独立人类 gold、审核者、仲裁与 heldout 保管未到位，正式 Top-1、Top-5、MRR 均 null。词频、语义和研究权重未正式放行。 |',
    '| 桌面、TSF 与长测 | not-run | 真实窗口继续 not-run/user-paused。未安装、激活或注册 TSF，未启动 8h/24h。CL-010 关闭。 |',
    '',f'16 项本地检查通过。Rust {counts[0]} 通过、{counts[2]} 忽略，Python {python_count} 通过。原 50 项要求及边界已逐项复核。'
     '原六条开发线、main 和五个 Draft 的最终状态以独立远程快照和回执为准。', '',
    '下一轮应先扩大经过许可的独立现代领域及真实意图标注，处理候选池召回损失，再比较端到端模型和混排。'
    '候选旁释义的真实窗口交互等待当前桌面暂停状态解除。', '',
    '[语言质量](integration-round23-language-quality.md) · [数据资源](integration-round23-data-resources.md) · [翻译质量](integration-round23-translation-quality.md) · '
    '[性能](integration-round23-performance.md) · [50 项产品追踪](cloudleaf-product-readiness.md)'
])
