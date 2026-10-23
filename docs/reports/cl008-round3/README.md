# CL-008 第三轮：完整候选核对与固定资源诊断

2026-10-23。**本轮离线分析完成；CL-008 仍部分完成，不进入 CL-010。** 默认输入核心、生产状态机、原型词典和搜索算法均未改。新增分析器与13项报告语义测试；机械宿主仅保留有序计时数组，分位数用副本计算。

设计先行 `ff5eac0`、观测补齐设计 `2ffd01c`；被测实现 `fa2f9812d19ff7ffb56f9c6ee9ae7eea5aa155d6`。源码指纹 `ffcdd12298982b8ad784e83c67f580c3ad0ba9edd9c683b9e25c1d0d00a45146`。全部精确文件、协议、二进制、探针和数据 SHA 见[provenance](provenance.json)，命令、时间、返回码及日志哈希见[validation](validation.json)。三次实验源码指纹一致；两次真实资源实验宿主二进制一致。

本地测量原件保持不变；本目录按仓库约定发布UTF-8/LF。`provenance.publication`逐文件记录本地原件与LF发布件双SHA，确认只规范化CRLF、字段与有序样本完全相同。analysis输入报告和validation日志SHA指向测量原件，`files_sha256`指向可从Git取得的发布件，避免平台换行转换使证据无法复核。

目录级 `.gitattributes` 固定命令日志为LF，仅允许其原有末尾空行，不裁剪原始输出以消除检查提示。

## 完整正确性与保留的损失

每 rank 按文本、整数 score、完整词条 IDs、全部 features、records 来源许可及顺序比较，另核对状态、错误类型、等待与原文回退。[analysis.json](analysis.json) 保存所有配置逐例的 equal_fields、可观察增删/名次、完整性原因及分组；完整原始候选仍在三份测量 JSON，不以只比文本替代路径比较。

完整参照必须是 ok、guaranteed=true、无工作预算/Beam 剪枝且整图完整；任一侧不完整，full_equivalence 为 null。非法拒绝相同也不转成完整搜索正确性。机械 ContextRerank 的保证仅限相同基础 Bigram 代表路径池，不能解释为全空间上下文最优。

- unihan：24例全部保留；标准 Exact/Position 相对完整 Reference 均 7 例一致、0差异，其余 17 例结论为 null。另有 4 例 Position 对完整旧 Exact 的同谱系一致比较，明确不算独立穷举验证。
- wiktionary：24例全部保留；标准 Exact/Position 相对完整 Reference 均 22 例一致、0差异，其余 2 例结论为 null。另有 0 例 Position 对完整旧 Exact 的同谱系一致比较，明确不算独立穷举验证。

Unihan 的7例完整参照为 empty、short-shi、short-zhong、short-lv、prefix-zhongg、waiting-zhong、unknown-zzzz，包含空串、等待及未命中，不能当作7条独立中文准确率答案。研究摘录的22例完整参照中也包含大量未命中，只有4例非空。

机械 13例×4评分×4策略，100测量/10预热；真实两资源各24例×12配置，30测量/5预热。三份报告共保存 38080 个按采集顺序排列的 ns 样本。

- Beam1/structural：可观察完整输出差异 4/13，案例 `dev-boundary, test-composition, test-beam-loss, test-repeat`；存在候选丢失的案例 `dev-boundary, test-composition, test-beam-loss, test-repeat`。剪枝方法不标为精确等价通过。
- Beam1/weighted：可观察完整输出差异 4/13，案例 `dev-boundary, test-composition, test-beam-loss, test-repeat`；存在候选丢失的案例 `dev-boundary, test-composition, test-beam-loss, test-repeat`。剪枝方法不标为精确等价通过。
- Beam1/bigram：可观察完整输出差异 5/13，案例 `dev-boundary, test-ambiguous, test-composition, test-beam-loss, test-repeat`；存在候选丢失的案例 `dev-boundary, test-ambiguous, test-composition, test-beam-loss, test-repeat`。剪枝方法不标为精确等价通过。
- Beam1/context：可观察完整输出差异 5/13，案例 `dev-boundary, test-ambiguous, test-composition, test-beam-loss, test-repeat`；存在候选丢失的案例 `dev-boundary, test-ambiguous, test-composition, test-beam-loss, test-repeat`。剪枝方法不标为精确等价通过。

未发现需要修改算法的完整参考差异，因此没有为产生改动调整算法、预算或评分。旧1260组与新增630组 Rust 差分仍保留。

## 数据范围和授权边界

Unihan 44,431条字符记录全部 unknown，runtime SHA `1b13b199440c0ca9b59074de9100f58c6a0b847b12a7b986e1b752084ff60f8e`；701条中文Wiktionary研究摘录 runtime SHA `b47287d0fd405c225409653cd8d08f34f9fcb31ef8bb8c9b9e54dbcf5fd7741c`。直接使用原 evidence 绑定的冻结文件，runtime/build/license/audit/records/notice 各项核验通过，没有改绑为 C 的新构建。见[证据核验](source-evidence-validation.json)。

研究摘录仅为固定压缩前8 MiB派生的完整JSONL行，非完整gzip；整包SHA、CRC和逐页例外审核未完成，来源顺序偏差仍在。没有升级为正式词库。原署名、变更、[Unicode许可](../cl008-real/Unicode-3.0.txt)、[CC BY-SA4许可](../cl008-real/CC-BY-SA-4.0.txt)与[摘录NOTICE](../cl008-real/wiktionary-NOTICE.txt)继续适用，候选/派生文本的许可证随 records 保留；工具代码单独为MIT。

**language_accuracy_valid=false**；Top-1/5、MRR、正式词频、语言模型质量为 null。无独立人工gold、合法训练/dev/封存heldout，不根据词典生成答案或使用heldout调参。非空候选率不是准确率。

## 全矩阵资源结果

单位ms；混合P95是本配置全部24例原始样本的分位数，最大单例P95是各例P95的最大值。两者分别列出。拒绝、空结果、工作预算失败仍在混合分母，不能据此称完成搜索更快；[资源分组](groups.md)另按kind、长度及完成/预算/剪枝/拒绝拆分。内存是每配置独立进程峰值，含预载词库/索引/宿主/候选/样本，不是每例内存；路径驻留计数不是字节数。

| 数据 | 配置 | 完整/24 | 非空/24 | 工作预算例 | Beam剪枝例 | 混合P95 | 最大单例P95 | 峰值MiB |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| unihan | reference | 7 | 19 | 15 | 0 | 192.4707 | 344.8608 | 33.95 |
| unihan | exact | 11 | 8 | 11 | 0 | 179.3015 | 226.9647 | 33.98 |
| unihan | beam1 | 6 | 19 | 0 | 16 | 5.0708 | 8.6998 | 34.00 |
| unihan | beam8 | 6 | 19 | 0 | 16 | 37.9432 | 74.2209 | 37.08 |
| unihan | beam64 | 6 | 17 | 2 | 16 | 231.4799 | 288.6756 | 34.55 |
| unihan | exact-edges128 | 2 | 3 | 20 | 0 | 1.7680 | 2.2396 | 33.92 |
| unihan | exact-expansions500 | 6 | 3 | 16 | 0 | 0.3889 | 0.8378 | 33.93 |
| unihan | exact-resident128 | 4 | 1 | 18 | 0 | 0.1141 | 0.5193 | 33.95 |
| unihan | position | 22 | 19 | 0 | 0 | 16.9688 | 29.8372 | 33.97 |
| unihan | position-edges128 | 2 | 3 | 20 | 0 | 0.1438 | 0.7040 | 33.93 |
| unihan | position-expansions500 | 6 | 3 | 16 | 0 | 0.4901 | 0.9308 | 33.93 |
| unihan | position-resident128 | 22 | 19 | 0 | 0 | 18.3106 | 33.4057 | 33.95 |
| wiktionary | reference | 22 | 4 | 0 | 0 | 0.0023 | 0.0108 | 5.43 |
| wiktionary | exact | 22 | 4 | 0 | 0 | 0.0034 | 0.0039 | 5.44 |
| wiktionary | beam1 | 22 | 4 | 0 | 0 | 0.0036 | 0.0169 | 5.43 |
| wiktionary | beam8 | 22 | 4 | 0 | 0 | 0.0035 | 0.0107 | 5.44 |
| wiktionary | beam64 | 22 | 4 | 0 | 0 | 0.0036 | 0.0086 | 5.44 |
| wiktionary | exact-edges128 | 22 | 4 | 0 | 0 | 0.0035 | 0.0187 | 5.44 |
| wiktionary | exact-expansions500 | 22 | 4 | 0 | 0 | 0.0035 | 0.0037 | 5.43 |
| wiktionary | exact-resident128 | 22 | 4 | 0 | 0 | 0.0033 | 0.0037 | 5.42 |
| wiktionary | position | 22 | 4 | 0 | 0 | 0.0024 | 0.0026 | 5.44 |
| wiktionary | position-edges128 | 22 | 4 | 0 | 0 | 0.0024 | 0.0026 | 5.44 |
| wiktionary | position-expansions500 | 22 | 4 | 0 | 0 | 0.0029 | 0.0212 | 5.41 |
| wiktionary | position-resident128 | 22 | 4 | 0 | 0 | 0.0035 | 0.0183 | 5.45 |

## 固定13例及短例退化

历史 `unihan-after.json` 的13例 P95>5ms名单完整保留；本轮新超限 13 例，名单 `word-beijing, word-renmin, word-xuexi, word-jisuanji, word-ruanjian, boundary-xian, ambiguous-xian, prefix-zhongg, sentence-woxuexi, sentence-jintian, sentence-beijing, long-xian, long-shi`。固定名单没有按新成绩删换。以下还保留全部 short/Unicode/显式边界/歧义案例。

| 案例 | 固定13 | 历史Position P95 | 本轮Exact P95 | 本轮Position P95/P99 | 图边 | 扩展Exact→Position | 驻留Exact→Position |
|---|---|---:|---:|---:|---:|---:|---:|
| short-shi | false | 0.4910 | 0.2227 | 0.2433/0.2535 | 344 | 344→344 | 345→7 |
| short-zhong | false | 0.0786 | 0.0772 | 0.0830/0.0899 | 2007 | 108→108 | 109→7 |
| short-lv | false | 0.0936 | 0.1292 | 0.0799/0.1036 | 148 | 148→148 | 149→7 |
| word-beijing | true | 13.6795 | 179.3015 | 11.5502/11.9280 | 3082 | 200000→13646 | 10311→21 |
| word-renmin | true | 8.2155 | 179.5940 | 6.5809/6.7178 | 1871 | 200000→7601 | 7496→16 |
| word-xuexi | true | 15.4623 | 226.9647 | 9.8918/11.9344 | 2829 | 200000→12605 | 12314→12 |
| word-jisuanji | true | 24.0760 | 151.4974 | 14.0746/14.3275 | 3396 | 200000→14416 | 1557→16 |
| word-ruanjian | true | 17.0332 | 220.5503 | 11.2430/11.9515 | 2898 | 200000→13926 | 7340→21 |
| boundary-xian | true | 6.2916 | 113.2051 | 5.1336/5.8018 | 1833 | 135395→6989 | 6441→16 |
| ambiguous-xian | true | 5.9852 | 184.0081 | 5.8692/5.8870 | 2555 | 200000→7711 | 7163→17 |
| prefix-zhongg | true | 9.4145 | 167.3002 | 8.7139/9.8104 | 2007 | 184140→8628 | 8624→11 |
| sentence-woxuexi | true | 7.9647 | 154.0527 | 5.4360/5.4647 | 3152 | 200000→7348 | 3171→16 |
| sentence-jintian | true | 12.6618 | 156.4994 | 7.5229/8.1127 | 3025 | 200000→9781 | 2841→16 |
| sentence-beijing | true | 20.5903 | 173.2480 | 17.7913/21.3235 | 4846 | 200000→22563 | 7096→26 |
| long-xian | true | 41.0867 | 216.5194 | 29.8372/32.0145 | 8770 | 200000→39374 | 6583→21 |
| long-shi | true | 10.9463 | 199.1361 | 10.1696/12.3211 | 2752 | 200000→12384 | 3436→11 |

本轮 short-shi Position/Exact P95 比为 1.093，历史已观察的短例退化不删除。二者图和扩展数相同，Position仅改变状态容器/保留方式；单词短图省下的状态开销有限。没有阶段计时、CPU剖析或受控背景负载，不能把比值归因给特定函数或视为显著性证明。

长图中位置压缩减少了按“最后词条”分组的重复状态，扩展/驻留计数支持这一机制解释；规范化、建图、排序去重、字符串/路径物化仍在计时内，现有数据无法给出阶段耗时占比。标准Position若仍超过5ms/10ms诊断目标，明确保留失败，不称生产输入响应达标。旧Exact触预算后空输出与Position完整输出的耗时不作为等质量加速比，JSON对应比值为null。

## 本地检查与未完成

locked fetch、workspace离线debug/release构建、fmt、Clippy `-D warnings`、all-targets（74 Rust通过）、doc、资源审计、Python全套（39通过，原26保留+13分析测试）全部返回0。原始日志逐命令保留。完整参考差异0；没有删旧测试、改默认引擎、原型词典或候选算法。

历史机械宿主没有保存原始数组，本轮没有补造历史样本。机械宿主新增数组输出及排序副本发生在计时区间外，但仍可能改变进程峰值与冷热状态；历史机械分位数与本轮不是同二进制的因果对照，不用其变化宣称算法收益。

- 完成：可重放新测量、逐rank完整字段核对、完整性门禁、Beam1损失保留、固定13例/短例与预算分组诊断、原始样本及完整指纹。
- 部分：同图精确搜索正确性与工程资源边界；Unihan长例中Reference耗尽，旧Exact同谱系不能替代独立参考，性能目标未全面达到。
- 未完成/阻塞：正式多字词库、合法词频/独立评估与模型、纠错/模糊音/简拼/双拼/学习/增量；生产算法选型、默认引擎接入及A/C退出条件均未满足。保持Draft PR #2，不合并main，不进入CL-010。

重放测量命令见 validation。重新执行分析时使用新的空目录，禁止覆盖：

```powershell
python scripts/analyze_search_round3.py --mechanical .local/cl008-round3-20261023/mechanical.json --unihan .local/cl008-round3-20261023/unihan.json --wiktionary .local/cl008-round3-20261023/wiktionary.json --unihan-evidence .local/real-lexicon/unihan-evidence.json --wiktionary-evidence .local/real-lexicon/wiktionary-evidence.json --output-dir .local/cl008-round3-reanalysis-new
```
