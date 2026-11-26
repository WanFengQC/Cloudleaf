# CL-008 Round 8：交给真实人员的人工 gold 执行包

2026-10-30。当前状态是**待外部材料**，不是已批准的标注包。没有真实作者、两位独立审核员、仲裁员、heldout保管者、授权或答案；正式Top-1/Top-5/MRR继续null。默认输入引擎不替换。本包没有新算法、样本答案、签名或通过声明。

用户本轮已明确：标注员、审核1/2、仲裁员、heldout保管者及授权文件路径均待定。下面的角色名称只表示责任，不是真实身份；实际参与确认和现存签署材料未到位前不得换成预设角色编号。双审及仲裁理由、变更后的重新审核记录、heldout访问审计也属于必须交付的外部原件。

## 由谁先提供什么

人员/授权只有C方向的`data/plans/round8-external-handoff.json`作为主登记，B不维护第二套人名或批准字段；该登记只收实际身份/文件位置，不能代替授权文件。当前C任务与摘要依据固定为[C提交3d573384的12项待填队列](https://github.com/WanFengQC/Cloudleaf/blob/3d573384ce1f31e437bdb4e7dc109cf438c261f4/data/plans/ime-annotation-round6.json)、[独立gold计划](https://github.com/WanFengQC/Cloudleaf/blob/3d573384ce1f31e437bdb4e7dc109cf438c261f4/data/plans/ime-human-gold-v1.json)和[三协议交接约定](https://github.com/WanFengQC/Cloudleaf/blob/3d573384ce1f31e437bdb4e7dc109cf438c261f4/docs/cl009-round7-annotation-handoff.md)。R8主登记的最终精确提交在本轮报告中记录，不依赖本机另一个worktree才能执行B工具。

[交付检查表](intake-checklist.tsv)逐份列责任角色、消费者和验收条件，实际路径/SHA栏全部留空、status全部pending。由项目负责人分配真实人员后，领取者在私有副本填写；不让Codex、模型输出或测试fixture代替人员/许可。

|角色|实际工作及独立性|
|---|---|
|原作者/标注员|仅使用原创或明确获准文本，填写意图、拼音、可接受输出、场景和完整按键轨迹；注明source/unit/lineage，不查看待测候选来决定答案。|
|两名独立审核员|分别审同一任务及授权/来源，均不能是作者；各自确认当前annotation/source与三协议摘要，不能让工具补approved。|
|独立仲裁员|出现拒绝或分歧时处理原争议；不能与作者或两审核员相同，绑定当前内容及三协议。修改内容后原审核重新进行。|
|heldout保管者|与上述参与者及评估者分离；全局分组/split、私有明文、实际加密/访问控制和保管证明由其负责；不把heldout答案给开发者。|
|评估负责人|看train/dev、选择参数和预算；在获批封存后固定实际源码/模型/参数/词库/二进制/协议，不能heldout调参。|
|外部验收负责人|独立核人、授权、语义近重复和保管事实，作正式评测放行决定；工具只能检查文件/摘要/声明一致性。|

12项现有pending槽仅用于启动工作，不是满足正式覆盖或样本规模的评测集。70/15/15按全局家族划分、heldout至少2000独立家族和已见41/83资源排除仍执行；不足时继续阻断，不能为较小任务包降低阈值。

## 材料分层与填写顺序

1. C来源审核人取得固定原件、原许可/授权和批准范围，明确评估、商业、修改及再分发权限；未知不填true。先登记人员/授权并固定protocol、preprocessing、lineage_audit三份实际文档。原件、引用、私人输入历史或未经授权内容不能塞入仓库。
2. 标注员和独立审核者按C实际工作流填写任务与source，保存双审/争议/仲裁原文及摘要。预处理、lineage、注释协议、来源或任务内容变动后旧审核失效；不得只重散列后代签。
3. 保管者在**获批私有环境**完成全局family/near/规范unit与意图分组，确定三个split。C v1结构冻结会写明文`heldout-tasks.jsonl`且`heldout_encrypted=false`；因此只能由保管者在其独占目录执行，开发评估者不能执行后浏览答案。C结构成功不等于B正式就绪。
4. 保管者实际完成加密、访问控制、密文完整性及授权保管证明，再交公共family-index、packet-metadata、train/dev任务和密文/证明。不得把明文heldout复制到公共packet、邮件、Git、报告或本工具输入。加密方法及密钥保管需真实环境批准，本包不假定已经配置。
5. B人工上下文审核者填写[context模板](context-review.template.json)的payload到新的私有文件，再填写[handoff模板](handoff.template.json)的payload。模板外层明确pending，**不是门禁输入**；没有审核材料不应提取payload去凑通过。
6. 评估负责人填写新的正式plan/preparation/handoff，来源及文档路径/原字节SHA逐项绑定；各相对路径均以对应manifest所在目录为准。保管者确认其文件没有暴露heldout答案后，按三个现有命令依次检查，任何非零即停止正式评测。工具全过仍需真人外部核实，当前没有正式语言打分执行结果。

## B私有上下文与任务映射

`context_review`根字段为schema_version=1、family_index_sha256、review_binding_sha256、all_case_intent_groups、cases。

- `all_case_intent_groups`恰好覆盖全部index案例，每项仅`case_id/intent_family_id`。意图组ID使用不泄漏文本的标识；同意图/改写关系须真人审查，不由词典/模型或自动聚类决定。含heldout的是公共ID和组标识，不是正文。
- `cases`只覆盖train/dev，每项严格为`case_id/task_sha256/intent_family_id/context_kind/confirmed_left_context`。`none`要求空左侧文本；`confirmed-left`要求真人确认的非空左侧上下文。不得编造上下文，也不能放heldout正文。
- `task_sha256`继续引用C规范13字段任务摘要，source binding继续绑定完整source记录；review binding继续绑定三协议。B私有context不偷偷加入C任务摘要。字段相同不证明内容或人员真实。

`handoff`只接受`metadata`、`visible_tasks.train/dev`、`context_review`的`{path,sha256}`实际描述和相同family_index_sha256。metadata必须是C公共`packet-metadata.json`，只含来源/协议/ID；train/dev文件恰好覆盖可见分母。不能增加heldout任务路径。

旧preparation还需四份实际文件：preprocessing、lineage_audit、annotation_protocol、annotation_review；逐源原件/协议/review必须全部覆盖。源授权与三协议、家族及人工审核变化不能只改单个manifest的SHA来沿用旧签署。

## 当前可重放的阻断检查

以下命令仅核当前仓库的空材料登记，不读heldout答案、不运行搜索或语言打分。输出目录需全新；如果已存在，换新run ID，不能删除历史或覆盖报告。

```powershell
chcp 65001 > $null
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$env:PYTHONUTF8 = "1"
$env:PYTHONIOENCODING = "utf-8"
$Round8Evidence = ".local/cl008-r8-external-check-20261030-01"
if (Test-Path -LiteralPath $Round8Evidence) { throw "输出已存在，请换新run ID" }
New-Item -ItemType Directory -Path $Round8Evidence | Out-Null
python scripts/check_language_readiness.py --plan config/evaluation/language-readiness-v1.json --output "$Round8Evidence/base.json"
if ($LASTEXITCODE -ne 2) { throw "当前空材料登记应保持阻断，请核对状态" }
python scripts/check_quality_preparation.py --plan config/evaluation/language-readiness-v1.json --preparation config/evaluation/quality-preparation-v1.json --output "$Round8Evidence/preparation.json"
if ($LASTEXITCODE -ne 2) { throw "当前准备材料应保持阻断，请核对状态" }
python scripts/check_ime_task_handoff.py --plan config/evaluation/language-readiness-v1.json --preparation config/evaluation/quality-preparation-v1.json --handoff config/evaluation/ime-task-handoff-v1.json --output "$Round8Evidence/handoff.json"
if ($LASTEXITCODE -ne 2) { throw "当前任务交付应保持阻断，请核对状态" }
```

真实材料到位后，在保管者/评估者明确各文件权限的前提下，以新的私有plan/preparation/handoff路径替换上述三份空登记，并使用全新输出目录；正式提交应得到各工具exit0，但这也不计算Top-1/Top-5/MRR或证明人/授权/加密真实。保留每条命令、返回码、原始stdout/stderr、精确commit及文件/工具/二进制SHA；出错不得删除案例或把错误候选从分母移除。

当前缺件、真实三门禁输出及C引用的精确身份见[Round 8状态报告](../../reports/cl008-round8-status.md)。本包不批准长测、TSF注册或CL-010，也不授权向第三方发送任何材料。
