# CL-008 Round 8：独立人工 gold 外部交接

2026-10-30。起点`b4b5fb7f812c56f73a132cd9f16045b58a3121c2`，分支`codex/cl008-search`。先行设计`ce5d2eadfb2801fbf4cdd49f47ac9c45dd71d55c`，被检查材料提交 **`a95e0bc360741d766b1e000add8ee4a7055d1d53`**；搜索/门禁实现仍为`2a2bd9a7ea9df73b647ccdba609d9aae8f0ad935`，没有新的算法、参数、生产代码或测试。最终提交仅归档本报告/证据，不能把其HEAD伪装成新性能测量。

本轮交付可交给实际人员的工作材料，而非继续重复机械实验。用户已明确标注员、审核1/2、仲裁员、heldout保管者和授权文件路径均待定；没有真人参与确认、签署或答案，12项任务仍pending，正式Top-1/Top-5/MRR继续null。不替换默认引擎，不合main，不进入CL-010。

## 实际修改及可直接领取的材料

|文件|用途|
|---|---|
|`docs/cl008-round8-external-handoff-design.md`|先提交本轮有界设计，保持旧协议/门禁，不因缺人再扩机械测试|
|`docs/acceptance/cl008-round8/README.md`|作者、独立双审、仲裁、保管者与评估者的实际职责、文档/摘要填写顺序和现有命令|
|`context-review.template.json`|B私有可见上下文/公共意图组的空模板，payload无案例/答案/有效SHA|
|`handoff.template.json`|公共metadata与train/dev描述的空模板，没有heldout路径|
|`intake-checklist.tsv`|13份具体交付物的责任角色、消费者和验收条件，路径/SHA空、状态全pending|
|README入口、本报告及`cl008-round8/`|可复核材料入口、真实阻断、命令/原始字节和身份|

人员/授权不建立第二套登记：C负责唯一主入口`data/plans/round8-external-handoff.json`；B只引用其归属，不用它替代旧packet或批准来源。主入口最终精确HEAD及字节由C方向报告固定；B运行不依赖另一个worktree。已有12任务队列和三协议定义的固定依据为C的`3d573384ce1f31e437bdb4e7dc109cf438c261f4`，实际队列SHA/12项均空状态见[材料核对](cl008-round8/packet-validation.json)。

两份JSON使用`pending-not-an-approved-packet`外层和空payload，**不能直接输入正式门禁**。人工材料到位后才能在新的私有文件填写payload；没有预置样本、角色编号、签名、授权文件名或approved。交付检查表中的角色名只表达职责，不冒充真实人员。

## heldout、审核和外部依赖

R7的13字段任务/source binding和三协议review binding、B私有context/intent-family映射继续使用；不复制C业务逻辑、不修改摘要定义、不自动转换为可放行配置。C结构冻结会写明文heldout，必须由独立保管者在获批私有环境中执行并实际加密/设置访问控制，公共材料只交索引、协议/ID、train/dev任务、密文及保管证明。开发者不能运行冻结来浏览答案，手册不生成或打开heldout。

双审/仲裁理由、变更后重新审核和heldout访问审计须由实际参与者提交。内容、授权、来源、预处理/lineage/注释协议或上下文变更，不能只重算SHA后沿用旧签署。工具只核声明/文件一致性，不独立证明人、权限、语义近重复或加密事实。

原70/15/15、heldout至少2000家族、已见41/83资源排除、来源家族及全局近重复、封存与参数冻结门禁保持。12槽只是启动队列，不是正式规模；没有使用词典、模型或AI生成gold，也没有以增量任务或文档数量宣称质量改善。

## 真实执行与实际阻断

[完整命令/返回码](cl008-round8/validation.json)和[原始stdout/stderr](cl008-round8/original-logs.json)保留了材料HEAD上的五项调用：

|命令|实际返回码|实际结论|
|---|---:|---|
|`check_language_readiness.py --plan config/evaluation/language-readiness-v1.json`|2|protocol通过；来源批准、独立gold、family-index、heldout保管封存、参数冻结缺失|
|`check_quality_preparation.py --plan … --preparation config/evaluation/quality-preparation-v1.json`|2|基础和逐源lineage/预处理/双审材料未交付|
|`check_ime_task_handoff.py --plan … --preparation … --handoff config/evaluation/ime-task-handoff-v1.json`|2|可见任务/上下文/意图审核及全部基础前置未交付|
|`python scripts/check_workspace.py`|0|本分支精确7 crate/0外部依赖及64/41/83资源审计通过，非语言质量验收|
|`git diff --check`|0|当时工作区格式检查通过，最终暂存检查另记录|

三个结果均`prerequisites_ready=false`、`language_accuracy_valid=false`、Top-1/Top-5/MRR=null；未打开heldout答案。`sealed_at`缺失等原始错误按原文保留，没有为了让报告好看而修改门禁或添加虚假材料。它们是本轮再次核对仍阻断，不是新的质量实验。

两份空模板形状、13项空交接表、真实C待填队列、相对链接及生产/门禁/历史无改的检查由[材料核对原件](cl008-round8/packet-validation.json)记录。只是低影响文档/模板核对，**新增测试0**。本轮未重复Rust全套、算法机械/阶段矩阵、原型基准或语言打分；原R7的81 Rust/76 Python及2,880对仍属于原被测实现，不改绑为R8新成绩。最终文档HEAD的Windows/Linux push CI由共享方向另核，CI自动回归不替真人完成gold。

## 身份与保全

|身份|实际SHA-256|
|---|---|
|未变源码cloudleaf-source-v2|`a43ca914bc624ca0839e62644866e7442093c36c5507b83f4e1f67542022b76d`|
|语言计划原件|`f8565f739865a25d789bbaf3d6a885f1877219e3ab0879f5688cf5111bde2e46`|
|准备计划原件|`c738ee1f8149ac0f134bed9962b439164bdf71b5ab8dd784861d7352f8d189bf`|
|handoff计划原件|`88d16dffc95ddadcfbc7b6df63026c6fef723cade89592c98cf1d6b79bbbb2be`|
|handoff工具|`cd010b12f6f8e20fe51c5e857b7a2e8aa46a3dc39c2079d2239045ba6c6b5bc3`|
|现有phase exe，373,248B|`e2737975ae1d3305af7f3c0d6fd32422eda7151bbbec7f887d37a53e493f363c`|

工具/协议/输入各SHA、原exe本机实测身份见validation；该exe本轮没有执行，没有编译新二进制。实际审核输入是三个空登记，未读取runtime内容或重新做词库性能。旧runtime SHA只在历史R7资料内，不冒充本轮重建结果。

[provenance](cl008-round8/provenance.json)区分原始UTF-8/CRLF与LF展示件，[原件封装](cl008-round8/original-artifacts.json)可无损重建，[Git发布字节校验](cl008-round8/checksums.json)与[运行器](cl008-round8/runners.json)可核。起点所有历史报告原路径无改，旧测试、工具、配置、资源及Rust生产代码无差异；本轮新增路径/具体差异在本报告对应commit中。

|状态|结果及下一步|
|---|---|
|已完成|交付可领取的职责/材料/空模板/保管流程，实际旧门禁缺件重核及原件归档；没有伪造修复或人工结果|
|部分完成|R7工具已支持摘要/双审/仲裁/上下文声明，R8已把执行顺序交给外部人员；仍不是完整人工语言质量评测器|
|外部阻塞|真实人员、授权及原件、语义/近重复审核、实际上下文、heldout加密/访问日志/保管及正式规模家族材料|
|未实现/未执行|真实人工gold、正式语言打分/发布比较、默认引擎替换；未重复机械矩阵、基准或本地Rust回归|

下一步由项目负责人落实C主登记中的真实人和签署材料，按执行包分工领取并保留理由/访问审计；完成固定授权和全局划分后，再让保管者封存、评估者冻结，使用既有三个门禁逐层核验。材料不足继续pending/null。CL-008未满足退出条件，PR #2继续Draft，不合main、不进入CL-010。
