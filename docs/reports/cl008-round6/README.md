# CL-008 Round 6：稳定候选桶与完整跨版本证据

2026-10-28。起点`08b09277d3beeb90e284145a3423b9f00eca8d6c`；被测优化实现`9673c29151380c146ae6ecb16055072cda0f0f00`，旧算法且同一观测宿主版本`f1696fa125d27507be0d328f04717b9760163417`。PR #2继续Draft，不合main，不替换默认核心，不进入CL-010。正式独立语言指标仍null。

## 实际问题、设计与代码变化

[先行设计](../../cl008-round6-search-design.md)提交`af0eafa`，`e590b03`在实施前明确不引入allocator/unsafe例外，单算法allocation/live/RSS本轮未测。R5阶段实际定位重复桶sort/dedup；桶已排序且正文unique、最多k项，每次追加仍完整稳定sort并为BTreeSet克隆全部保留正文。本轮优化为私有稳定有序unique插入：同正文仅严格更优替换、相等保留先到edge/features，不同正文compare相等插入upper bound，最后截断k。最终跨状态合并仍用原sort/dedup。

生产修改仅`crates/cloudleaf-search/src/search.rs`（`9673c29`）：Reference完成桶、Exact/Beam状态桶、PositionDag位置桶共用该操作。原expand/resident检查、extend、遍历、评分、预算、剪枝和候选总量完全保留；不提前跳过注定被丢弃的新路径。没有发现或宣称这是P1正确性错误，属于可核对P2成本优化。

另一个实际工具缺口已修复：R5宿主JSON未包含edge_indices及完整finite_pool_only/raw_fallback，跨版本只能比较已序列化字段。同版本plain/profile的Eq本来完整，不足以替代跨版本。`aed4b29`先加回归，真实旧宿主因缺`result_debug`返回1；`f1696fa`在计时外保存完整Result Debug字段，包括错误，再冻结旧算法exe副本。该额外字段只作本轮工程对照，不是生产数据格式。优化前后宿主源SHA相同：`4fa6a37c538e8c2998bb1fb4f19e1796fd4299cbe1a69ad88651d739cb285148`。

修改清单：search.rs及两个内部桶语义测试；resource_phase_probe.rs只增加计时外完整结果；tests/test_search_phases.py新增真实release宿主验证；设计/README及本目录新证据。旧共享types、input、解析/词典、配置/工作预算、C数据和历史报告未修改。

## 失败、回归与精确身份

[红测](red/full-result.txt)和[红测身份](red/identity.json)保存命令、精确`aed4b29`、KeyError及exit1；[工具修复后的绿测](baseline/green.json)包含8项phase测试及两个桶测试。桶语义回归旧算法本来通过，未强造正确性失败，也未为红测增加生产计数接口。最终完整15条检查的实际返回码与原件见[validation](checks/validation.json)：**79 Rust / 66 Python**、locked/offline debug/release、fmt、Clippy拒绝warnings、doc、资源审计、机械100测量/10预热、原型1000及旧83条诊断符合预期。两个真实语言前置命令仍exit2，不能记为exit0成功。

|身份|SHA-256|
|---|---|
|新源码cloudleaf-source-v2|`c39bc6f8ca56ee59489be8af39eedef0b37b5aafe979e844706b4467b8fec456`|
|旧源码cloudleaf-source-v2|`3b7212ebd1ab81ce1116eaa05eda10cf4b971412ed6e4a64ded691ecdbee1f5d`|
|新阶段exe|`484ef282c50e0809499ff2afbf6514b0ba80ffc35351cb2b29726d6f0816d49f`|
|旧阶段exe|`30f3775ff89aa210ab8cd588b303075e7e862ef0576fb399ef468bbef48f8274`|
|固定runtime|`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`|
|原24探针|`124177ad89496ed2507c0726e211ea488693d6825b41b8d55c3af751ee7f3207`|
|固定资源协议|`692067c0507676bdf6c5dbe632a566e87cfcaedd6719c06bf5c75d1e5ae11e37`|
|本轮矩阵runner|`f34aae01fd59a981bfc0afee00f95d7a42d9dbe48ba479e226a743078a4538c4`|

本轮[runner原文](tools/run_round6_matrix.py)调用实际Cargo产物，核验旧exe副本SHA、固定词库/探针/协议/完整配置；记录开始/结束源码一致，保留每配置原stdout/stderr与全部样本。[provenance](provenance.json)区分原件和规范LF展示件SHA；[原始日志](original-logs.json)、[CRLF/许可原件](original-artifacts.json)的utf8字段按UTF-8编码可恢复全部字节。数据原文/候选及派生报告按CC BY-SA4，工具代码MIT不覆盖数据；许可证和署名完整保留在原件封装中。

异机重放先分别从上面两个精确commit建立隔离检出，不在现有方向工作区切换或重置未提交状态。两份检出都运行`cargo build --release --locked --offline -p cloudleaf-search --example resource_phase_probe --message-format=json`，从各Cargo实际输出取exe并固定各自SHA；同机本轮的exe摘要不能强加到不同工具链/链接环境的重建物。核对同一个runtime、cases、protocol内容SHA后，按每配置报告的command参数运行两版；只允许显式调整路径，新目录保存结果。发布runner中的本机绝对路径及冻结head断言须对应重放检出，不能为了运行而替换输入/预算或绑定旧二进制到新源码。

## 同机完整版本矩阵

固定123,544条词库，unknown频率123,544，多字110,045；原24探针×12配置、每例30次plain/profile配对及5次双入口预热。旧新各8,640对（合计34,560次被测查询）分别同版本完整Eq通过；**288组跨版本完整Result Debug与既有JSON字段全部相等**。两版所有已报告字段也与原R4固定结果相同。拒绝/预算/剪枝每版175组均建立[完整索引](failure-and-budget-index.json)，不删失败分母。

|配置|完整|预算|Beam剪枝|拒绝|plain总P95 ms 旧/新|桶阶段P95 ms 旧/新|
|---|---:|---:|---:|---:|---:|---:|
| reference | 8 | 14 | 0 | 2 | 174.7538 / 82.2693 | 79.6529 / 18.5426 |
| exact | 16 | 6 | 0 | 2 | 166.6070 / 86.7641 | 80.3999 / 26.6678 |
| beam1 | 6 | 0 | 16 | 2 | 1.1968 / 1.2517 | 0.1786 / 0.1396 |
| beam8 | 6 | 0 | 16 | 2 | 10.7092 / 8.2869 | 4.0885 / 1.5312 |
| beam64 | 7 | 0 | 15 | 2 | 60.7956 / 37.8315 | 46.5330 / 16.1500 |
| exact-edges128 | 4 | 18 | 0 | 2 | 1.8286 / 0.9511 | 1.9266 / 0.5369 |
| exact-expansions500 | 6 | 16 | 0 | 2 | 1.0199 / 0.2473 | 0.1611 / 0.0473 |
| exact-resident128 | 6 | 16 | 0 | 2 | 0.1066 / 0.0745 | 0.0176 / 0.0113 |
| position | 22 | 0 | 0 | 2 | 5.7603 / 3.0970 | 3.5997 / 1.0101 |
| position-edges128 | 4 | 18 | 0 | 2 | 0.3224 / 0.1489 | 0.1572 / 0.0456 |
| position-expansions500 | 6 | 16 | 0 | 2 | 0.4228 / 0.2025 | 0.1996 / 0.0643 |
| position-resident128 | 22 | 0 | 0 | 2 | 5.5810 / 2.9762 | 3.6335 / 1.0088 |

这些总体分位数混合24探针；阶段分母只含实际进入的样本，桶阶段嵌套在遍历，不能相加或相减。每配置交换旧新**进程**顺序，没有逐查询跨版本交错；桌面背景未控制，时间对比只是当前固定负载的诊断，不能单独证明统计因果、算法最优或产品TSF延迟。整个进程峰包含词库、双图/双结果、累计JSON及新增Debug，不代表单算法内存，亦不用于声称内存收益。

标准Position合法22例的逐例目标见[position-resource-gates](position-resource-gates.json)：旧P95>5ms `2`例/P99>10ms `0`例；新为`2`/`0`例。参考耗尽和Beam有损仍保留；本轮不据更快的局部搜索宣布资源门禁全面通过。无独立gold，不计算Top-1/Top-5/MRR；这些探针不是词典之外的语言正确答案。

## C任务工作流与正式质量门禁

C新增annotation-v1任务明确意图、拼音、可接受输出、按键轨迹、场景、source binding、双审/仲裁和角色。其family-index的8字段形状与B现有契约对齐，不等于兼容或批准：还需逐任务语义/hash核验、所有evaluation-corpus来源的artifact/lineage和权利审核、共同preprocessing/lineage audit/annotation protocol/review的path与SHA、结构化review全来源覆盖。C明文heldout文件不等于B要求的封存/保管证据；最低家族数和70/15/15配额仍由原base门禁检查，不能靠小fixture解除。

实际没有真实标注员、独立双审/仲裁/保管者及合法固定gold，C待填任务不是人工材料。旧readiness与新增preparation均继续拒绝，[报告](checks/quality-preparation.json)prerequisites_ready=false、language_accuracy_valid=false、Top-1/Top-5/MRR=null。不从待测词典生成独立答案，不用heldout调参，未修改旧两个门禁。

## 状态与下一依赖

- 完成：重复桶维护的窄优化、完整跨版本结果字段缺口修复、设计/回归先行、全部本地检查、原失败/预算分母与新旧完整矩阵保存。
- 部分完成：实际正式规模资源定位和对照；单算法allocation/live/RSS没有测量，完整语言质量和不同输入分布仍未知。
- 未通过/材料阻断：Reference/Exact预算失败、Beam剪枝与参考不完整；合法独立人工gold/来源家族语义审核/双审仲裁/封存，现代频率及正式语言模型。
- 未实现：纠错、模糊音、简拼、双拼、学习、增量、默认输入核心接入；本方向不代替真实Windows应用验收。

旧Round1–5报告按起点Git blob保存，见[历史核验](history-preserved.json)。最终纯证据提交的HEAD和Windows/Linux push CI由共享Round6报告记录，不能将本地检查或旧CI推为新head验收。PR2保持Draft，不合main，不进入CL-010。
