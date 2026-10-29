# CL-008 Round 7：任务交付语义与独立评分观测

2026-10-29。起点 `417dad662bfebedf3238f909476240427cc9100b`，先提交设计，再反例/边界测试与最小实现。默认输入核心、搜索预算/排序和共享接口不变；不合main、不进入CL-010。

## 实际缺口与范围

1. `predictions`只检查`fallback_preserved`的布尔类型。拒绝、预算或未支持结果即使明确丢失原文仍可通过预测门禁。增加真实旧实现失败反例，错误结果须保全原文；全部失败案例仍在固定分母，不能用删除病例通过。
2. R6观测的遍历包含评分，不能单独报告评分成本。新增显式研究`Scoring`阶段：extend内特征/整数/可选转移计算（不含Path克隆及文本拼接），finish内EOS；嵌套于traversal。ContextRerank仍按原独立阶段，不把嵌套相加或相减推导未观测CPU。默认Noop仍不读钟、算法和结果不变。保留既有九阶段名称，新宿主与分析报告显式阶段schema2，旧R6证据继续按原版本解释。
3. C的family-index八字段与B相同，并不证明task摘要、来源、审核及上下文语义兼容。新增B私有离线`check_ime_task_handoff.py`，不导入或复制C的工作流实现，不读取heldout答案，不自动生成gold或批准plan。

## 任务交付协议 v1

manifest为`schema_version=1/experiment=cloudleaf-ime-task-handoff-v1`，固定`family_index_sha256`；`metadata`、`visible_tasks.train/dev`、`context_review`均为实际path/SHA文件描述。metadata采用C `cloudleaf-ime-annotation-v1`公共元信息；不允许tasks/正文答案。只读取train/dev任务；heldout仍要求旧B配额、加密保管/未打开和参数冻结，task-handoff不另读取明文heldout。

C `task_sha256`明确定义为UTF-8/LF规范JSON（sort_keys、separators=(',', ':')、ensure_ascii=False，末尾LF）的13字段摘要：id、source_id、source_unit_id、family_id、near_duplicate_group、split、source_binding_sha256、author_id、input_intent、pinyin、acceptable_outputs、key_trace、scenario。source binding为完整metadata source规范JSON摘要。输入摘要保留既有ascii-lower-umlaut-v1，不偷偷换规范化协议。

C本轮review绑定增加`review_binding_sha256`，定义为包级protocol_sha256/preprocessing_sha256/lineage_audit_sha256三字段规范JSON摘要。B审核和仲裁都核这个摘要，不接受只绑protocol的旧审核。双方只共享离线方向私有契约，task原13字段定义不变。

context_review为schema1，绑定family_index_sha256和三协议review_binding_sha256，cases只覆盖train/dev全部case；每项case_id/task_sha256/intent_family_id/context_kind/confirmed_left_context。context_kind为none时left为空，confirmed-left时left为非空字符串；原C v1没有独立context字段，不假装已有。意图组为独立人工声明，不能由词典/模型推断；公共all_case_intent_groups清单只含case_id、intent_family_id覆盖全部index，跨split同意图组拒绝，不含heldout正文。保管者须提供对应review，工具不证明语义聚类或真人真实性。

逐个可见任务核字段、摘要、来源绑定、两位非作者审核和必要独立仲裁；task pinyin/ID/来源等与index逐项对应，文件恰好覆盖固定train/dev分母；批准来源及lineage来自原B独立来源审计。旧base/preparation两门禁原样组合，任何缺失都使总prerequisites_ready=false。新config登记实际缺项，Top-1/Top-5/MRR始终null；临时fixture不是正式gold。

## 对照与证据

不重复R6整套8,640旧新配对。保留Reference、ExactDag、PositionDag和Beam原机械差分/预算及稳定桶边界；新增评分观测入口与未读钟入口完整Eq、评分调用与拒绝/预算未进入阶段边界。小图Reference差分继续用真实公开API，不能把桶实现复制进测试。

若重放固定真实24探针/12配置，沿用同词库SHA/预算，以新源码/二进制和schema2单独记录。R6原样本再分析必须同时标明旧被测实现与本轮分析器，不能称新HEAD性能或单算法内存。正式语言指标无独立gold仍null。所有失败、拒绝、剪枝和超预算保留原始分母与日志。

实施范围：研究search/profiling/阶段宿主及对应测试；B语言readiness、任务交付工具/blocked配置/测试；本设计、README和独立Round7报告。旧协议/阈值和旧报告不改，来源正文与gold不新增到仓库；Rust/Python全套、资源审计和方向专项按精确实现执行。最终普通推送原Draft分支，新HEAD Windows/Linux push CI由共享方向核验。
