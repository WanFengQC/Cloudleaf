# Round 6 人工任务与 CL-008 的显式交付映射

`cloudleaf-ime-annotation-v1`只组织人工任务、双审/仲裁和冻结文件，不制作答案、不证明人员或权利真实性、不执行语言打分。实际队列为`data/plans/ime-annotation-round6.json`，12项均pending，尚无协议/授权/作者/审核员/仲裁员/保管者。`audit`实际exit2；正式gold未交付，指标null。

录入顺序：合法固定原件/授权→人工意图、拼音、可接受输出及完整键序列→按source记录规范SHA填`source_binding_sha256`→按协议组织两位独立非作者审核→若任一拒绝，独立仲裁当前内容→保管者按全局家族/near组与规范unit分配split→在受控目录冻结。任意任务内容/来源授权/协议变化须重新审核。`annotation_digest(task)`和`source_digest(source)`是实际工具提供的摘要方法，不手动替换审核记录以追随修改后的内容。

冻结生成train/dev/heldout独立文件、公共family-index和packet-metadata，后者不复制任何任务正文/答案。heldout文件没有加密；文件分离不是访问控制，也不是B的`custodian-encrypted`证据。正式保管者及受控路径尚未交付，当前只用synthetic fixture验证流程。

|C输出/字段|B现有要求|仍需真实交付与审核|
|---|---|---|
|`family-index.jsonl`八个字段|R4 `INDEX_FIELDS`：case_id、family_id、source_id、source_unit_id、near_duplicate_group、split、normalized_input_sha256、task_sha256|C使用同一`lower().replace('ü','v')`输入摘要；task_sha256是本工具绑定source context的完整任务摘要，仍需B独立审核其任务定义。字段相同不等于语义兼容。B仍检查41/83已见种子、70/15/15家族与heldout至少2000家族；小fixture不满足正式门槛。|
|C `sources[].artifact_sha256/lineage_id`|B evaluation-corpus artifact与preparation逐源lineage|人工来源锁另须原件path/SHA、固定版本/官方URL、许可/审核文件、评估/商业/修改/再分发权限；C source声明不能直接充当批准锁。|
|包级protocol/preprocessing/lineage_audit SHA|B preparation四个实际path/SHA文档及逐源相同预处理/标注protocol/review绑定|须交真实preprocessing、lineage_audit、annotation_protocol、annotation_review文件，不只摘要字符串；逐源清单须另绑artifact/preprocessing/protocol/review SHA，source unit须规范到同lineage。全局语义near聚类真实性仍独立审阅。|
|task双审/仲裁、source binding SHA|B结构化annotation_review清单必须恰好覆盖全部evaluation来源及原件/协议|未来转换只可据真实审核生成逐源`id/artifact_sha256/annotation_protocol_sha256`覆盖清单，不能以主source代表全部来源或从synthetic名字推断真人独立。|
|heldout独立文件与manifest SHA|B seal要求独立保管者、加密封存、未打开答案、时间与family-index绑定|实际加密/身份/许可与参数、源码、二进制、词库、协议冻结仍未满足；C工具不读取现有heldout答案、不能自动填seal为通过。|

工具的`structurally_ready=true`只说明本协议的声明/结构一致；`formal_language_ready=false`始终保留。C没有自动生成可通过B门禁的plan/preparation，未来适配须单独固定文件和审核。生产语言侧包、研究源断言图和人工IME gold三个工件不能互相替代。

```text
python -m tools.data_pipeline.annotation_workflow --pending-output <不存在的新队列JSON>
python -m tools.data_pipeline.annotation_workflow --packet <人工队列> --output <新审计JSON>
python -m tools.data_pipeline.annotation_workflow --packet <已审核队列> --freeze --output <全新受控目录>
```

未完成/未授权任务拒绝冻结，已有输出拒绝覆盖。测试使用独立临时目录和自编字段，不能将其答案列为正式语言数据。
