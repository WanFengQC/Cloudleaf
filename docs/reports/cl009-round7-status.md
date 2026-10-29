# CL-009 Round 7：处理协议审核失效与来源矩阵

2026-10-29。起点`52619a963070cd74c53d0b8d76fc40bf5b7a8327`，分支`codex/cl009-data`，被测实现 **`e96ed5dc96c6cc0446f3446838fab9305047c1bc`**。其后`12b2adb`只补B/C交付文档，报告提交只归档证据；不将被测SHA改绑到报告head。无main合并、CL-010、TSF注册或新数据授权。

## 实际修改与先行设计

已读AGENTS、roadmap、product-scope、R6报告、来源登记/三个固定锁/许可原文、人工计划、侧包公式及发布/恢复源码。开始工作区clean。只新增离线工具和审核绑定，不改生产词典、核心或共享接口。

|文件|实际作用|
|---|---|
|`docs/cl009-round7-design.md`|先行设计`f2aa76f`，界定已复现缺口、审核迁移和来源矩阵范围|
|`tests/test_annotation_workflow.py`|先提交`65a5d75`真实失败，增加处理/lineage版本失效、旧审核拒绝及仲裁/manifest绑定回归，保留原断言|
|`tools/data_pipeline/annotation_workflow.py`|review及生效仲裁绑定三个处理协议；freeze manifest明确binding schema1|
|`tests/test_source_review.py`、`tools/data_pipeline/source_review.py`|缺模块测试`5e507eb`后实现，离线重放现有来源/锁/许可/证据，拒绝变更与重写，双构建可重放|
|`docs/cl009-round7-semantic-release.md`|现有类型侧包与正式频率/语义发布之间的缺项，未伪装已有发布器|
|`docs/cl009-round7-annotation-handoff.md`、管线README|B/C显式摘要、旧审核失效和上下文私有文档差异|

工具修复提交`fb162e7`；被测`e96ed5d`另补候选未批准范围断言。原schema2/3、TSV v2、音节表、123,544 runtime与1,468隔离、unknown均保持。生产发布、回滚、恢复及`linguistic_sidecar`实现未改。

## 原始缺陷与修复证据

**P1：包级预处理/来源家族审核改变却沿用旧人工审核。**R6只将review绑定任务/source与注释协议。`65a5d75`在旧实现中实际测试`preprocessing_sha256`、`lineage_audit_sha256`改为另一合法SHA，两例均出现`AssertionError: True is not false`。命令、完整SHA和原始输出在[red身份](cl009/round7/red/round7-red/identity.json)、[失败原文](cl009/round7/red/round7-red/output.txt)。这次旧源码已提交可精确重放，不是事后构造草稿或缺模块错误。

现在新增`review_binding_sha256 = sha256(canonical({protocol_sha256,preprocessing_sha256,lineage_audit_sha256}))`，两独立review和生效仲裁都必须匹配。源绑定和13字段task摘要仍保持原定义。freeze manifest另记`review_binding_schema_version=1`和摘要。旧R6审核缺字段明确拒绝，不能自动代签或迁移；新增回归保留正常、仲裁、旧审核拒绝及现有split边界。

来源矩阵的首次red是缺新模块`ModuleNotFoundError`，只证明新增功能尚缺，**不当作旧生产安全漏洞**；[原始记录](cl009/round7/red/round7-matrix-red/identity.json)单列。新矩阵4项测试核实际来源、许可/原件冲突、重复/无审查锁、CLI双构建和禁止覆盖。

## 来源矩阵与采用结论

[完整来源矩阵](cl009/round7/matrix-a.json)共9项：八条已有登记+已选Wikipedia固定revision的R6未通过准入快照。许可文件与锁的SHA、CC-CEDICT镜像证据SHA和登记archive SHA实际核对。保留source/version/original SHA/许可文本SHA/商业修改再分发声明/署名/例外/处理协议/原审核记录；没有取得时间或权限证据的字段为null，reviewed_on不冒充取得时间。工具不新增批准或改锁。

|来源|本轮结论|
|---|---|
|Unicode Unihan17|既有批准字符读音/英文原始释义范围保持，固定原件/Unicode-3.0许可身份核对；非现代词频|
|CC-CEDICT官方站候选|继续取得方式阻断；不自动请求MDBG/API/export|
|CC-CEDICT固定镜像|既有完整原始词条/读音/原始义项采用范围保持，商业/修改声明来自既有锁；不等于完整POS/义项对齐或语言质量验收|
|Wiktionary固定前缀|research_only；全件gzip/CRC及逐页例外未闭合，未升级production|
|SUBTLEX-CH|特定原件许可商业/分发范围未解决，未取数据|
|wordfreq-zh|中文组成来源和可分离性未解决，未导入|
|Google Books Ngrams中文候选|具体bulk许可/版本/单位未核，未批准|
|Leipzig候选|商业权限未核，未导入|
|Wikipedia固定page342/rev94816134|正文身份已核但权利/全部作者/提取/分词未齐，实际准入仍exit2；不是新来源锁|

本轮未广搜、取得新语料、重分发现有未全审正文或批准新统计/语义数据。矩阵是固定现有审查的机器一致性报告，不替真人或法律真实性审查。已批准词条不被矩阵降级，研究/阻断候选不被升级。

## 正式频率与语义侧包现状

[发布前契约](../cl009-round7-semantic-release.md)分开token比例、DF文档计数和正token母观测派生Zipf，明确各分母、单位、语料版本/时期、提取/分词实现及配置。既有公式和unknown/零值回归保留；没有新真实观测，所以token/DF/Zipf仍无新值，不能从词序、DF或单字推多字频率。

已实现的独立v1类型侧包可校验稳定entry ID、POS/tagset、sense ID、源证据、显式有向翻译引用、短语/组件及review声明。R6研究图保留原源POS/887义项/241有向英语声明和双向lookup，属于来源追溯，不是语义事实审核或自动双向等价。**正式内容绑定审核、可信发布清单、签名/升级迁移及生产语义侧包发布器尚未实现；真实POS/义项对齐/短语语料和授权人工审读未交付。**v1的verified仅指结构和公式/引用一致性，不能据此发布production。

人工IME仍12任务pending，无真实人员、许可、答案、签名或封存。实际audit exit2、freeze exit1且未创建冻结目录。B/C本轮明确新review binding和上下文差异：C v1没有独立context/intent-family字段；B私有context_review须真人提供，B不读取heldout答案、旧配额/封存门禁不变。正式Top-1/Top-5/MRR继续null，不从词库或AI生成gold。

## 验证、数据身份及恢复边界

[11条全量检查](cl009/round7/validation/checks.json)均exit0：Rust1.90.0，locked fetch、offline debug/release build、fmt、Clippy `-D warnings`、**60 Rust**、doc、资源审计、**89 Python**及diff检查。89=旧83+2人工工作流+4来源矩阵；没有删除旧测试。各条stdout/stderr原件分别保留，不把检查数量当语言质量。

89 Python包含原有确定性双构建、损坏拒绝、原子升级/回滚、回退目标损坏及用户字典保护。真实临时fixture子进程在lock/copy-mid/cache-renamed/pointer-written/pointer-replaced/rollback-before/rollback-after七点退出73并显式恢复，另有父进程终止和未知owner拒绝。它们本轮实际执行通过；**不是123,544条全包380秒压力的重跑，不是物理断电或跨版本用户词库迁移验收。**

[6项专项](cl009/round7/diagnostics.json)绑定精确git archive，采用隔离`python -I -X utf8`执行：来源矩阵两构建exit0，pending audit/freeze分别2/1，Wiki准入2，旧正式包verify0。两份矩阵均23,295B，完整字节相同；对旧9文件SHA只读复核完全不变。没有本轮资源采样或性能比较，命令耗时只供重放记录。

|身份|SHA-256|
|---|---|
|源码git archive（e96ed5d）|`f180b3d0d7007114d51e6c96427120131fc1f4f8e2d08537e681c752774593ad`|
|矩阵原始canonical JSON，两构建|`b9a6b8425799e2b7ddaabba9585c8a883f1a9eab96650a0b527957551445cd61`|
|annotation_workflow.py|`ca64a3c2e1d82dcf8df896dabd1a9634e05760d3294eebd336727cfbbe3814eb`|
|source_review.py|`65b5e8fddafd9c09cb5b1a6a017f343d7969684d299b892da6192b401c097e1e`|
|旧正式runtime|`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`|
|release core exe|`d53f54a25bd6b4fb8c114a06966010d4b98bf0597efb7e9c5e897e96c59e5b80`|
|Python3.12.10解释器exe|`4d6f5f81a4bca11191c4c7c6b43632694d0a4ce74e068619d8fdc161d469859a`|

[identity](cl009/round7/identity.json)分别保存代码、工具、矩阵、解释器和旧包身份。两个矩阵展示JSON规范化LF/缩进，与原始canonical字节分别登记；[33份原文封装](cl009/round7/raw-artifacts.json)可按utf8.encode重建原字节，SHA/字节数逐份保存，[provenance](cl009/round7/provenance.json)区分原件/展示件。

[历史保全](cl009/round7/history-integrity.json)确认R6起点114份报告无改，旧测试无删除，保护的核心/生产发布代码无变更；[校验清单](cl009/round7/checksums.json)覆盖全部发布件Git字节，[运行器](cl009/round7/runners.json)提供原源码。最终报告HEAD的Windows/Linux push CI另由共享R7汇总逐项核对，不预先声明通过。

|状态|结论|
|---|---|
|已实现/通过|处理与lineage变更审核失效、显式旧审核拒绝、实际来源矩阵/一致性与双构建、全部工程回归和旧正式包verify/只读身份|
|部分完成|频率/语义发布前契约、既有类型侧包和源断言可追溯；没有正式材料就不能生产发布|
|数据/真人阻断|合法现代统计原件、权利/作者/例外/分词、实际人工角色/授权/双审/仲裁、意图近重复审查和heldout封存|
|未实现/未执行|正式频率/语义发布器、内容签署/升级迁移、物理断电、独立正式语言评测；未重跑旧全包380秒压力或将研究图接入默认核心|

CL-009未达到正式词频/语义发布退出条件。PR #3继续Draft，不合main，不进入CL-010。
