# Cloudleaf Round 10：语义发布基础设施与资源/准入门禁

2026-10-30（Asia/Shanghai）。实际按C→B→A→共享推进；用户本人是唯一负责人，worktree及Codex技术子任务不代表多人审核。先核实际仓库、Round 9报告/源码/原件与四PR，再在每方向设计/红测试后实现。共享仅文档与审阅，未合入方向代码；main保持`a564fd47b0f054ff360592af3ccb8d744021160f`。无强推、Ready切换、合并、TSF、8h/24h或CL-010。

## 精确版本与新CI

|方向/分支|实际R9起点|完整本地受测代码|R10最终HEAD / 新push CI|
|---|---|---|---|
|共享 `codex/cl007-009-contracts`|`e296c300ea1b1d41c6cdc36500f690dec0906964`|`455b4ed7c3039c366ddf7684d7033ecae793a847`；生产源码不改|报告提交后另核最终head及CI，避免自引用SHA|
|C `codex/cl009-data`|`1c9a047c74aea01e504ed355601c0fe021b53f4b`|`2472dffc854a5a0db511757949c4c11e6867c769`|`3f26f34e00c0775124f87e150a021f99fb52c567` / [40930353255](https://github.com/WanFengQC/Cloudleaf/actions/runs/40930353255)|
|B `codex/cl008-search`|`2751b1512394e09c92bca2b4329456c3cd3c30de`|`f478c66c52eac7e43afe85bc7e5cfe96e2e51254`|`59dabdc2f8a3dc87cbd7f19ed89401ee607f7954` / [40932593654](https://github.com/WanFengQC/Cloudleaf/actions/runs/40932593654)|
|A `codex/cl007-windows`|`1a05633c27c133bc0558b0fb955b0f916edb6b72`|`e58d3ae6d8671a5f8f822aa2b0d0d80f37b93596`|`e0ab78c2786d652a3a19cb544b4dc83996e63264` / [40933356010](https://github.com/WanFengQC/Cloudleaf/actions/runs/40933356010)|

A/B/C最终push均已逐项核完整head、Windows/Linux全部步骤和两个未过期产物head/digest，A每平台19步骤、B18、C17。共享最终报告提交后也必须取得自己的新push证据，旧成功不能替代。最终方向原始响应归档到[方向CI](cl007-009-round10-shared/direction-ci.json)；共享最终完整差异、报告SHA和CI提交后另存`.local/round10-final-receipt.json`并绑定PR #1，不能把报告包含的实现提交说成最终HEAD。

## 已执行检查与原件

|方向|本地完整回归|专项及限制|
|---|---|---|
|共享|11命令实际均0；64 Rust、14 Python；fmt/Clippy/all-targets/doc/release locked/offline、资源依赖审计|生产源/测试/原型资源不改，无新性能或语言结论|
|C|60 Rust、108 Python，完整适用检查通过|13项新真实实现测试、7处子进程中断边界；20项真实pending候选可读但生产拒绝|
|B|83 Rust、89 Python；最终11命令均0|384观测案例、1920计时样本、384预热和384结果调用；计数通过，正式资源不放行|
|A|157 Rust、42 Python；保留原2 ignored；11命令符合预期（10项0、pending检查2）|六类旧授权复用失败先保存；真实pending有11缺件且child_started=false，不执行长测或系统操作|

实际命令、返回码、环境、stdout/stderr和工具源码分别见方向报告及共享`pr1-round10-evidence`。编译缓存复用和二进制SHA不是新性能测量。起点R9的四个成功CI重新只读核对，完整基线快照独立保存，不改绑新head。CLI gh仍不在PATH，本续段连接器不可用后复用官方API及既有Git凭据，仅进程内认证，不打印或保存秘密。

独立字节核对A **36项Git清单/32份原件/8运行器/563旧报告**，B **165/159/6/466**，C **50/46/5/259**；清单数不含清单自身，原件用UTF-8封装还原原始字节，LF展示件另列。旧测试未删除，原Rust核心与数据/来源锁保留。C/B用审计工具SHA`5071230a750fbb6ae175a34e47641942c5e728f46101fe67ecb621937847a66e`；A历史路径增长后使用同逻辑的Windows长度修正版，旧工具副本一并归档，避免工具身份漂移。共享起点125份历史报告亦保持；[技术证据](../reviews/pr1-round10-evidence/review.json)固定实际审阅差异4,099,665B与全部blob/SHA，包含最终报告自身的差异另存最终收据。

## C：独立语义reader/verifier与发布恢复

[精确方向报告](https://github.com/WanFengQC/Cloudleaf/blob/3f26f34e00c0775124f87e150a021f99fb52c567/docs/reports/cl009-round10-status.md)。设计`adf798f`、新API红`19284c4`先于实现。新增`tools/data_pipeline/semantic_bundle.py`、两份测试、设计/操作说明；`pipeline.py/recovery.py`仅透传可选verifier/transition，默认仍原schema2行为，当前引擎不读取新sidecar。

版本化bundle/content、资源manifest、词条目录/稳定语义ID/引用、有向翻译和原relation反查、来源断言/许可与当前内容审核绑定、显式外部策略、升级迁移/损坏检测/回退/异常恢复已实现。reverse lookup不制造逆向等价事实；pending/null payload可作候选，生产拒绝未审/research_only/过期审核/坏引用及版本不兼容。包外policy及approval文件须由调用者显式信任并精确绑定；**文件一致性不证明真人、授权真实性或密码签名**，crypto签名和真人权利证明标志始终false。

实际修复两项新verifier缺口：`True==1`让错误approval reader_version通过（红`822a5e6`），以及首发无current恢复在缺policy时仍移动遗留锁（红`8f8fb5a`）。前者严格核int，后者进入guard前核策略；精确旧模块重放和原工作区身份分开记录。新API缺失红不冒充旧算法损坏；协议fixture未重置导致的首轮error也原样保留，修准备而不弱化断言。

7处真实自有子进程中断保留，不能冒充物理断电。现有CC-CEDICT九包和R8实际20个待审项形成Git外候选：**零语义payload生成**，候选manifest SHA`3fb8229edb1ccb2bf4ed26eec9c3ce5957f872f24d4eeb5ffb4e28b17b72dca9`，目标production store未创建。74.242秒含复制/校验，是离线演示而非输入延迟。CC-CEDICT/Unihan批准范围、Wiktionary research_only、Google候选及unknown不变；无真实POS、sense对齐或双向翻译交付。

## B：可对照的计数合同，资源仍未验收

[精确方向报告](https://github.com/WanFengQC/Cloudleaf/blob/59dabdc2f8a3dc87cbd7f19ed89401ee607f7954/docs/reports/cl008-round10-status.md)。合同/旧反例`02fb8ec`后实现单一观测入口、新`check_search_resources.py`及probe v2完整配置/原输入；Rust新增直接检验真实Reference部分完整路径和逐边覆盖的测试，不改算法src或默认引擎。

P1工具缺口均先红后修：旧compare容许budget+guaranteed矛盾及输入/家族错配；复核又发现无截断却guaranteed=false可跳过oracle、合法探针全拒绝也可能通过（红`f4df5a0`）。最终入口按真实方法/配置/探针语义核验，旧报告不补虚构v2字段。Reference搜索预算耗尽保留已完成完整路径子集，ExactDag/PositionDag/Beam丢弃部分路径，图边截断仍可搜已有边；全部截断/剪枝均不能声称完整。

固定runtime **123,544**条、频率全unknown，同24输入、Structural、k5、context_pool20、Beam宽8，四方法各用同标准/边128/扩展500/驻留128预算。最终384案例计数通过，无可用完整oracle下的候选等价失败；不具完整oracle的案例不冒充已证明正确。标准组触预算Reference14/24、Exact6/24、Position0/24、Beam0/24，Beam16/24剪枝；每组每方法2个非法输入拒绝，32/384保留分母。所有候选丢失/新增/排序和预算/剪枝保留在原矩阵，第一次矩阵也不覆盖。

标准各方法最大案例P95为99.1333/95.7221/4.5608/9.9357ms，每例仅5次且背景负载未控制，不宣称最优或达标。Reference/Exact慢例及截断仍是实际资源问题。ResidentPaths是活路径计数，未测单算法字节分配，不能从整体进程峰值推算；用户取消API未实现，预算中止不是取消。`production_resource_ready=false`，Top-1/Top-5/MRR仍null。

## A：完整identity绑定准入，真实平台仍阻塞

[精确方向报告](https://github.com/WanFengQC/Cloudleaf/blob/e0ab78c2786d652a3a19cb544b4dc83996e63264/docs/reports/cl007-round10-status.md)。设计`f940a6c`、旧红`241d4da`先行。旧实际授权只绑定环境/人/时间/档位，更换commit/source/binary/data/tool/config并更新外层identity后仍放行；保留同一授权原件/SHA的六个subcase真实失败，修复checker schema2绑定完整规范identity SHA，仅排除准入后新增的批准摘要避免自引用。

独立材料检查分列materials_consistent、execution_identity_binding_checked与ready；没有本次实际identity不能宣布可执行。旧授权不自动升级。原runner已在创建segment/child前复用同入口，生产Rust、COM/STA/Host、watchdog、Unknown/预算及逐segment reporter不改；续跑片段不能拼成连续8h/24h通过。新pending授权/环境/批准表与真实应用步骤复用原九类矩阵，不填真人或验收。

真实隔离VM、快照/实际恢复、负责人和执行授权仍未落实，未启动8h/24h、独立native/loader、TSF或真实应用，未新增长期内存/句柄/线程/COM趋势。原不可回收owner/COM长期占用、部署/异常文字恢复风险继续阻断，不能把准入工具回归当生产可靠性验收。

## 失败、SHA及人工决定

原始失败包括C两项新增缺陷/新API红/fixture错误与本地演示导入失败（仅修local路径补跑未执行项）；B两次实际P1红及主机PATH/内联引号准备错误；A六类旧授权红及归档WinError206。A冻结旧归档器，另目录精确重放206并保存返回码1，不把重放流冒充首次未捕获的原流；随后一次读取reports差异与旧集合核对，保留重命名检查，未重跑已过产品测试。共享发现期的gh缺失和在实现过程中查询尚未存在/命名不同文件仅工具转录，明确非原始字节捕获，不伪造成功。所有最终检查独立记录，不取链式末尾退出码充当全部结果。

|主工具/数据|SHA-256|
|---|---|
|C semantic_bundle.py|`2d453bab0d8e7780280391fd52203ee38d696c00a9b288e6ac77d8aef671fac0`|
|B资源gate|`d655a09168f547649b2ff7b9ac2b0fa3d3c498f8ab8227d7af4ad9fb24fa560c`|
|A准入checker|`045e9de59ac47e1506e969d9afc8c55cfc1f1e62d2b71f75c9a5906532492d36`|
|固定runtime|`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`|
|CC-CEDICT原件ZIP|`fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`|
|共享release CLI，未做新基准|`187484c0839c9eebbac7e1895c95b4072c9cf03f4a5f220cdf8799003a47f031`|

完整源码、其余工具、实际原件/包与各平台二进制身份分列方向identity；source-v2不包含C的tools目录，故单列新模块SHA及实现Git。builder读取物理工具字节，同固定工具的双构建不等于已验证Windows/Linux产物逐字节相同。

[人工审阅补充](../reviews/pr1-round10-human-review.md)继续引用R8十四项，[决定模板](../reviews/pr1-round10-decision-template.json)保持pending/null/false。共享Input/Types blob和方向merge-base仍旧，不代表已整合。用户实际合并后才fetch新main，保留旧head并逐方向普通merge、审冲突、完整本地回归及新CI；本轮不执行这些操作。

## 完成边界与最终决策

|状态|结论与下一步依赖|
|---|---|
|done|独立语义发布基础设施、实际verifier缺口修复、观测/计数门禁错误修复、完整identity准入、原失败与全回归/身份归档、人工准备；均为单人可推进工程|
|partial|候选与外部文件一致性、路径计数正确性可验证；生产语义真实性、算法资源退出与实际平台可用性仍未验收|
|blocked|用户实际隔离环境/恢复与执行授权/合并决定；真实语义审读/授权与审批；独立gold双审/必要仲裁/heldout及来源家族/近重复冻结；现代频率统计准入|
|not-run / 未实现|真实生产语义内容发布、密码签名部署、物理断电/真实迁移、单算法字节内存、用户取消、正式语言评测、默认引擎替换、长测/TSF/真实应用、CL-010|

1. **PR #1：可供用户考虑人工审阅，尚无批准。**公开源码无新改动，技术清单/回归齐备；最终精确CI仍须提交后核验，mergeable不代人工决定。
2. **CL-007：不放行真实应用。**仍缺隔离环境与已验证回滚、实际责任人与冻结identity授权、连续8h/24h及原可靠性退出证据，之后TSF仍需单独明确批准。
3. **CL-008：不具正式独立语言质量评测条件。**真实gold及独立人员/授权/封存未交付，指标null；计数矩阵不证明中文准确率，默认引擎不替换。
4. **CL-009：不具正式词频/语义内容发布条件。**基础设施已实现，真实内容审读与可信审批、统计来源/单位/分母准入未闭合；unknown/research_only不改。
5. **P0/P1：本轮已复现可修工具/verifier缺口均关闭，未据此宣称全产品无缺陷。**长期平台回收/输入恢复、搜索慢例/资源和正式质量/授权等退出风险仍保留；未执行项不记作通过。
6. **CL-010不允许进入。**A/B/C原退出条件尚未分别满足，四PR继续Draft、main不合并。
