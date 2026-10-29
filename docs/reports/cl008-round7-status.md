# CL-008 Round 7：任务交付、原文回退门禁与评分诊断

2026-10-29。分支`codex/cl008-search`，起点`417dad662bfebedf3238f909476240427cc9100b`，被测实现**`2a2bd9a7ea9df73b647ccdba609d9aae8f0ad935`**。最终报告提交只整理证据；PR #2保持Draft，不合main、不进入CL-010、不替换默认核心。独立人工gold未交付，正式语言指标均null。

## 实际缺口与设计先行

已读AGENTS、roadmap、R6完整报告、CL008设计、PositionDag证明、真实资源协议及实际搜索/评分/观测/评估工具。[设计](../cl008-round7-design.md)先提交`8265966`，反例`ba17cbf`，随后`cea9a7f`在新测之前固定10配对/3预热与方案前置，实施`2a2bd9a`。

|实际修改文件|目的与边界|
|---|---|
|`scripts/check_language_readiness.py`、`tests/test_language_readiness.py`|错误预测明确`fallback_preserved=false`时拒绝；保留全部固定失败分母，不改配额/阈值|
|`scripts/check_ime_task_handoff.py`、`config/evaluation/ime-task-handoff-v1.json`、`tests/test_ime_task_handoff.py`|B私有显式任务/上下文/全局意图及C摘要映射，实际材料缺失登记blocked，不读取heldout答案|
|`crates/cloudleaf-search/src/profiling.rs`、`search.rs`、`examples/resource_phase_probe.rs`、`tests/profiling.rs`|显式Scoring观测及schema2；算法、排序、预算/驻留检查和Noop未读钟入口保持原义|
|`scripts/evaluate_search_phases.py`、`tests/test_search_phases.py`|新schema2分母/阶段核验，明确拒绝将旧九阶段样本改标为评分数据|
|设计、README、本报告与`cl008-round7/`|新协议、复现命令、原始失败/数据/工具/二进制身份和未完成边界|

默认input/types、候选/词典/拼音与CLI代码未改；无新增Rust依赖、unsafe分配器或新共享接口。319份起点历史报告Git blob无变化，旧测试没有删除，64/41/83资源保持；[逐文件保全](cl008-round7/history-preserved.json)。

## 真实失败、修复与回归

[红测原文及身份](cl008-round7/red/identity.json)绑定已提交`ba17cbf46100e96708d9790cb3dcf85f620cd706`：

- **P1预测门禁缺陷**：在真实旧`predictions`上，对budget/rejected/unsupported分别将首例的`fallback_preserved`改为false，原实现都不拒绝，三个子例真实`ValueError not raised`、exit1。现在非ok结果必须保全原文；没有删除预测案例或掩盖错误状态。它是评估工具缺陷，不宣称已复现默认输入核心丢字。
- **缺评分观测**：旧Rust实现成功编译，但公开profile measurements中无scoring，运行时断言失败exit101；不是编译缺enum的假红测。新增阶段后，完整Result Eq和明确扩展/EOS调用计数、全部ranker/拒绝/等待/预算边界通过。
- **缺任务交付工具**：旧树模块不存在，exit1；只说明功能缺失，不能冒充旧生产工具的行为漏洞。新增8项机械fixture覆盖任务/来源/协议/三协议审核失效、独立双审/仲裁、全局意图泄漏、上下文与固定分母；不是人工标注。

第一次本机红测runner在两个Python反例完成后，因Windows裸cargo路径查找失败而中断；完整重放改用实际绝对Cargo路径，仍在同一旧实现上得到1/1/101。[准备失败说明](cl008-round7/harness-preparation.json)不伪造当时未保存的helper完整字节。产品失败原文和最终结果均保留。

## 正式任务与C的显式映射

新handoff只打开获批train/dev任务。逐项核C原13字段规范JSON摘要、规范拼音摘要、source_binding、index身份、两位非作者审核及必要独立仲裁；审核/仲裁要求C本轮新增的`review_binding_sha256`，绑定protocol/preprocessing/lineage_audit三协议，旧审核缺字段拒绝。元信息只允许公共来源/协议/ID，不复制任务或heldout答案。

独立`context_review`补充C v1没有的上下文格式：train/dev可见任务必须绑定task SHA和`none`/`confirmed-left`及明确左侧内容；公共全案例清单只列case ID和人工意图家族，跨split同意图家族拒绝。没有从拼音/词典/模型推断正确答案或语义聚类。清单与SHA一致不证明真人、许可或语义独立性。

[实际双工具摘要对照](cl008-round7/handoff-compat.json)读取C源码`3d573384ce1f31e437bdb4e7dc109cf438c261f4`的真实函数：13字段相同、原12个空白pending任务摘要相同、三协议摘要和source摘要相同。C工具SHA`ca64a3c2e1d82dcf8df896dabd1a9634e05760d3294eebd336727cfbbe3814eb`；这不是创建人工答案或把C方向合入B。

旧base/preparation来源审批、全局family/near/规范unit、70/15/15与heldout至少2000家族、加密保管/未打开以及模型/参数/源码/二进制/词库冻结全部继续生效。heldout正文、任务摘要与真人审核仍须保管者独立检查；本工具明确不打开它。实际无人员、授权语料、上下文审核和封存，三个门禁**实际exit2**，prerequisites_ready=false、Top-1/Top-5/MRR=null。

## 同输入/同数据/同预算的新评分观测

Scoring覆盖extend内整数/特征和可选模型转移，排除Path克隆与正文拼接；finish覆盖EOS检查/转移。它与桶阶段均嵌套在traversal，ContextRerank仍独立。没有把嵌套阶段相加/相减估计独占CPU；读钟/记录开销仍在profile中。Noop不读钟，Reference/ExactDag/PositionDag/Beam排序、候选、回退和预算完全保留。

先冻结本轮实现，A短资源测量结束后再运行：同一个123,544条runtime、原24探针/12配置/Structural/k=5/全部原限额，每例10个plain/profile配对、3次双入口预热，**2,880对，5,760次被测查询**，额外预热不计入测量分母。每样本同一新宿主比较完整Result Eq；288配置/案例另与R6最终完整Result Debug及JSON逐字段相同，0差异。R4已报告行为字段也相同；不将耗尽Reference当全空间正确性证据。[完整身份](cl008-round7/phases/identity.json)、[逐例候选/分位数/差异](cl008-round7/phases/comparison.json)、[全部原始样本](cl008-round7/phases/matrix/summary.json)。

|配置|完整/预算/剪枝/拒绝|plain总P50/P95/P99 ms|嵌套Scoring P95 ms|
|---|---|---|---|
|Reference|8 / 14 / 0 / 2|73.0694 / 85.5663 / 93.7060|13.1668|
|ExactDag|16 / 6 / 0 / 2|11.1743 / 96.6260 / 111.3299|7.2425|
|PositionDag|22 / 0 / 0 / 2|0.7135 / 3.7766 / 5.8201|0.4231|
|Beam1|6 / 0 / 16 / 2|0.2736 / 1.2494 / 1.7579|0.0912|
|Beam8|6 / 0 / 16 / 2|1.9409 / 8.5014 / 13.8911|0.7126|
|Beam64|7 / 0 / 15 / 2|9.9428 / 45.3612 / 71.7274|4.2802|

总体每配置240个计时样本混合24探针；Scoring只在实际进入的190个样本有值，未进入为null，不能补0。逐例仅n=10，P99分辨率不足以证明稳定尾延迟；**不与R6 n=30作速度改善或资源放行比较**。全部12配置和175个失败/预算/剪枝病例见[索引](cl008-round7/failure-and-budget-index.json)，2个拒绝案例每配置均保留。工程短诊断、桌面背景未控制，不代表真实TSF或正式语言质量。

进程峰仍采集为宿主整体资源，包含预载词库、图/结果和JSON，不用它推断单算法allocation/live/RSS；本轮没有单算法内存结果。评分方法不同或调用次数不同，不能直接用累计Scoring时间推断每次评分的独占成本。

## 完整验证与身份

[17条完整命令/返回码](cl008-round7/checks/validation.json)：Rust1.90.0、locked fetch/offline debug与release、fmt、Clippy `-D warnings`、**81 Rust / 76 Python**、doc、资源/许可审计、原机械100测量/10预热、原型1000、旧83 seed均符合预期；三个正式语言门禁的实际exit2保持拒绝，不计作正式准入通过。全部旧测试保留；机械模型和83 seed不是正式gold，不据此报告中文准确率。

|身份|SHA-256|
|---|---|
|本轮源码cloudleaf-source-v2|`a43ca914bc624ca0839e62644866e7442093c36c5507b83f4e1f67542022b76d`|
|schema2阶段exe|`e2737975ae1d3305af7f3c0d6fd32422eda7151bbbec7f887d37a53e493f363c`|
|原固定runtime|`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`|
|原24探针|`124177ad89496ed2507c0726e211ea488693d6825b41b8d55c3af751ee7f3207`|
|任务handoff工具|`cd010b12f6f8e20fe51c5e857b7a2e8aa46a3dc39c2079d2239045ba6c6b5bc3`|

[provenance](cl008-round7/provenance.json)记录源码/工具/原始字节和LF展示件不同身份；[77项发布校验](cl008-round7/checksums.json)、[35份原始日志](cl008-round7/original-logs.json)及[2份CRLF原件](cl008-round7/original-artifacts.json)可按utf8字段UTF-8编码恢复原字节。完整runtime许可/署名/固定来源锁随证据保留，候选及派生数据按CC BY-SA4，工程代码MIT不覆盖来源数据；固定镜像不是最新版或发布者签名原件。

`tools/`保留本机runner原文/绝对路径与参数；异机重放从精确实现检出，按各命令在新目录执行，先核原数据/协议SHA，再冻结自己构建的二进制身份，不能强加本机exe SHA到其他工具链。最终新交付HEAD的Windows/Linux push CI由共享Round7汇总独立核对，本报告不提前宣称成功。

## 状态、剩余阻塞及下一步

- 已实现/已通过工程验证：错误原文回退门禁、B/C可见任务摘要与三协议审核/上下文/意图声明的显式映射、schema2评分阶段、全部回归及当前输入/数据/预算完整结果对照。
- 部分完成：正式规模各阶段诊断与搜索对照；预算/剪枝差异继续存在，n=10新观测不放行资源目标，单算法内存尚未测量。
- 材料阻断：合法独立人工gold、真实作者/双审/仲裁/保管者、上下文和语义近重复家族审核、heldout加密封存、现代词频与授权训练模型。
- 尚未实现：完整正式语言评测器与发布比较；同正式v2 unknown数据的旧生产排序适配；纠错/模糊音/简拼/双拼/学习/增量、默认输入核心接入及真实Windows验收。

先由真实人员交付合法任务/协议与家族/近重复审核、C来源与B私有上下文材料；冻结train/dev/heldout，再按旧门禁及新任务映射核验。参数只用train/dev，不能从待测词库自动生成gold或用heldout调参。各算法方案的当前机制与正式前置见设计表；默认生产排序不把unknown虚构为0，Bigram/上下文的机械机制不冒充语言收益。CL-008退出条件未满足，不进入CL-010。
