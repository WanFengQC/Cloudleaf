# Round 5：真实独立语言评估的来源家族与准备manifest

2026-10-27。此附加约定独立于C事实/TSV格式和既有R4正式计划，不擅改共享接口。先提交设计和攻击测试，再实现离线核验；不读取封存答案，不生成gold，不使用holdout调参。

R4已核验全局family、near group、规范输入/task摘要及来源内段落隔离。仍须显式覆盖“同一原件换来源ID”和“派生来源的段落身份继承”：新增`cloudleaf-quality-preparation-v1`准备manifest，绑定v1计划的family_index SHA、共同预处理文档/人工lineage审计文档及其精确SHA。R4原计划/门禁保持原义，新工具分别给基础门禁与准备门禁状态，任一缺失仍拒绝。

准备manifest字段：`schema_version=1`、`experiment=cloudleaf-quality-preparation-v1`、`normalization=ascii-lower-umlaut-v1`、`family_index_sha256`、`preprocessing`与`lineage_audit`（均path/sha256）、`annotation_protocol`与`annotation_review`（path/sha256）、`sources`（逐来源id/artifact_sha256/lineage_id/preprocessing_sha256/annotation_protocol_sha256/annotation_review_sha256）。这里只允许evaluation-corpus，必须恰好覆盖基础计划的全部评估来源。每个source artifact必须等于基础来源固定产物，预处理SHA必须等于共同文档；同artifact不同ID必须归入同lineage，不能靠改ID逃避。基础家族索引的source_unit_id按人工审计文档约定为lineage内规范段落ID，不能任意重编号；同lineage+unit跨split拒绝。每个来源均纳入同一标注协议和人工review范围，不以主gold.source_id代表其他来源已审核。review文件自身为`schema_version=1/sources`结构化清单，每项id/artifact_sha256/annotation_protocol_sha256；它必须恰好覆盖全部来源且与manifest绑定一致，重新散列一个只覆盖A的review不能替代A+B审核。

这只是可复查声明/摘要契约，不能自动证明段落映射、语义近重复聚类、标注者独立性或许可真实性。换lineage号、改分组声明等语义欺骗仍需独立审阅。来源内原始编号若没有规范到共同lineage ID，准备阶段就未完成；不偷偷猜测文章或句子同一性。

当前没有合法固定人工评测集、独立双审/仲裁、lineage清单和保管者，实际准备manifest必须明确blocked，指标null，命令exit2。临时fixture只测试字段/篡改/跨源泄漏/缺来源，不是批准数据。C可另设计POS/义项/短语、人评来源计划，不假定其sidecar已自动满足此manifest；未来需独立显式转换和审核。

范围：新离线`check_quality_preparation.py`、实际阻塞计划JSON与攻击测试、README和Round5报告。原R4门禁及数据格式不改；共享/平台/默认输入核心不变。新工具不代表已交付语言评估，也不放行CL-008或CL-010。
