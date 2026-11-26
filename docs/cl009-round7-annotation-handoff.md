# CL-009 → CL-008 Round 7 人工任务交付差异

R7沿用C的`cloudleaf-ime-annotation-v1`、13个任务摘要字段和八字段family-index；源绑定仍为完整source记录的canonical SHA。只新增审核必须绑定全部三个包级处理协议的约束。该变更由C先设计并通过旧实现真实失败测试，不修改共享Rust接口或B正式配额。

`review_binding_sha256`为UTF-8 canonical JSON（排序键、紧凑分隔、保留Unicode、末尾一个LF）的SHA-256，JSON对象恰有`protocol_sha256`、`preprocessing_sha256`、`lineage_audit_sha256`。两位review和生效仲裁都要提供；freeze manifest记录`review_binding_schema_version=1`和该摘要。**R6旧审核缺字段时失效，工具不得替真人补签或自动标为兼容。**

|材料|C已实现|B仍须独立核验|
|---|---|---|
|task/source摘要|annotation_digest的13字段与完整source_digest不变|逐train/dev实际任务与index/task SHA、来源原件/授权和各协议一致|
|review绑定|三个处理协议摘要和当前任务/source绑定|独立review/分歧仲裁、原件证据与真实性；C结构通过不代替B批准|
|上下文与意图家族|C v1只有input_intent/scenario，没有独立context/intent-family字段|B私有context_review schema1：绑定family-index及review_binding；公开全case意图分组仅case_id+intent_family_id；train/dev可包含context_kind none/confirmed-left及confirmed_left_context，由真人提供|
|heldout|只在独立split文件，明确未加密|B审计不打开heldout答案，仍核已有seal/独立保管/配额，且首次正式评测前冻结|
|来源矩阵|既有来源/锁/证据身份与未知权限|不是任务来源授权或人工签名，不能直接当新评估source批准|

B私有补充文档不写入C任务摘要，也不从词典或模型生成上下文/答案。没有实际人员、授权和内容，因此当前12任务pending，B旧config仍blocked；70/15/15、heldout至少2000来源家族及原正式门禁均不变。所有正式Top-1/Top-5/MRR继续null。
