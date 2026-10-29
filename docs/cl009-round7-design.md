# CL-009 Round 7：审核失效与来源证据矩阵

2026-10-29。起点`52619a963070cd74c53d0b8d76fc40bf5b7a8327`。只修改离线数据工具，不修改Rust核心、生产音节/词库格式或发布恢复实现。

## 已定位缺口与修复

R6人工任务审核绑定了任务内容、来源记录和`protocol_sha256`，但包级`preprocessing_sha256`、`lineage_audit_sha256`变化仍能沿用旧审核并通过结构门禁。分词规范或来源家族审计变更后，审核员认可的任务解释/划分已经变化；继续沿用审核会错误冻结材料。这是可直接重放的离线入口缺陷，不增加实时输入或并发复杂度。

在现有v1离线任务格式新增必需的`review_binding_sha256`，其值为`sha256(canonical({protocol_sha256,preprocessing_sha256,lineage_audit_sha256}))`。两位审核员及有分歧时的仲裁均必须绑定。`annotation_digest`的FIELDS和family-index八字段保持不变。旧R6审核没有该字段时明确失效，不自动补签名或迁移；由真实审核者重新审核后提供新摘要。manifest增加审核绑定的版本和摘要，不能把相同family-index形状当成B正式材料兼容。pending12任务不变。

先提交能在旧实现失败的preprocessing/lineage反例，再实现。正常/仲裁夹具仅补明确新字段，保留全部旧断言。正式gold仍未交付，结构校验不证明真人、授权或语义近重复。

## 来源矩阵与语义发布边界

新增`source_review.py`从实际八条`source-review.jsonl`和三个锁生成离线矩阵，核对锁的source ID、许可文件SHA及已登记的证据SHA；已登记固定archive SHA冲突必须拒绝。未知商业/修改/再分发权限保持null，来源许可声明与已有项目审核结论分列；矩阵不是新的法律批准或production升级器。既有Unihan/CC-CEDICT可用范围不变，Wiktionary摘录仍research_only。选定Wikipedia R6固定revision的未通过准入作为单独候选快照展示，不新增批准锁。

矩阵必须列来源/版本/固定原件SHA/许可原文身份/商业使用/修改/再分发/署名/页面例外/提取及分词协议缺项，保留原审查记录原文。来源未实际取得时不补取得时间或原件SHA。工具仅支持生成全新输出，双构建须相同。

现有`linguistic_sidecar`仅验证声明的结构和数学/引用一致性，不是正式发布器。本轮补正式侧包发布前约定：稳定entry/sense/relation ID、有向翻译和短语来源、内容及审核摘要绑定、许可范围、独立双审与保管封存；没有这些材料就没有production语义包。不得从研究断言猜义项对齐、反向等价或频率。

token频率=`token_count / total_tokens`；DF值为出现该词的文档数，分母为总文档数；Zipf=`log10(token_frequency)+9`且仅从同词、同源、同语料/时期/分词的正token母观测派生。没有观测保持unknown/null，DF不能充当token或Zipf的母值。该实现和旧回归保持，不为增加提交量重写已正确公式。

## 验证与交付

运行Rust locked/offline构建、fmt、Clippy、单元/集成/doc及全部Python、资源审计；保存新红测原文、源码/数据/工具/二进制身份。对新矩阵做双构建、篡改许可/锁摘要拒绝；运行既有发布/损坏/回退/恢复真实临时夹具并保留分母。旧正式9文件只读SHA复核，不重复无变化的380秒全包压力，不宣称物理断电或用户词库迁移通过。

新报告`docs/reports/cl009-round7-status.md`、证据目录`docs/reports/cl009/round7/`；R1–6不覆盖。普通推送原Draft分支，最终精确Windows/Linux push CI由共享方向核验。不合main，不进入CL-010。
