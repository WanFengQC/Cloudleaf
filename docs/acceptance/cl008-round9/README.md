# 单人维护者的私有作者任务队列

本流程支持用户本人原创、填写自己的标注及改错，不代表已经取得正式gold。Codex不会填写实际身份、授权、正确答案或人工审核。独立双审、必要仲裁、heldout保管和正式70/15/15/2000家族门禁维持原标准，详见[设计](../../cl008-round9-authoring-queue.md)及[R8正式交接流程](../cl008-round8/README.md)。

## 导出未划分草稿

只输入作者材料，**禁止把已冻结或含heldout正文的包交给工具**。export仅接受全部split=null的C schema1 packet，显式固定原字节SHA；可从C分支取得现有12个pending槽，也可使用用户真实原创并按同一结构填写的私有未划分材料。工具不自动从词典/模型生成任务。

```powershell
chcp 65001 > $null
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$env:PYTHONUTF8 = "1"
$AuthorPacket = "C:\Users\EDY\.codex\worktrees\cl009-data\Cloudleaf\data\plans\ime-annotation-round6.json"
$AuthorPacketSha = (Get-FileHash -Algorithm SHA256 -LiteralPath $AuthorPacket).Hash.ToLowerInvariant()
python scripts/manage_ime_annotation_queue.py export --packet $AuthorPacket --sha256 $AuthorPacketSha --output .local/my-authoring-queue-01.json
```

路径是本机已知C材料位置，移交其他机器时显式改成实际取得的作者文件，不能猜不存在文件的内容。输出必须新建；已有文件请换新名字，不能删除历史来重跑。export成功exit0只表示草稿已生成，其usage永远为internal-development-only。初始真实队列仍12项pending，不填任何人或答案。

在私有副本的`packet.tasks`填写本人实际标注，保留原槽ID并可新增原创任务。所有split继续null；不同意图/拼音/可接受输出/键轨迹/来源/场景由本人填写，不看待测候选来决定答案。原始C字段和摘要规则不变，不把私有答案或授权扫描件提交Git。

## 授权与进度

实际来源登记在`packet.sources`，实际授权原件通过`authorization_files`按来源ID引用`{path,sha256}`；相对路径基于队列文件目录。来源原声明的authorization.sha256必须与文件原字节一致，evaluation/modification/redistribution范围须真实明确。文件存在和声明相符不能证明签署人、权限或内容真实；未落实项留空，报告继续列缺件。

```powershell
python scripts/manage_ime_annotation_queue.py status --queue .local/my-authoring-queue-01.json --output .local/my-authoring-progress-01.json
```

有效输入生成报告exit0；格式/SHA/split错误或输出已存在返回2。状态报告不包含意图/拼音/答案或完整任务，只含ID、摘要、计数及缺件。三层结果分别为：

- `personal_annotations_complete`：个人字段完整，仍只是内部草稿；作者自己设置completed也不会取得正式gold身份。
- `current_review_declarations_consistent`：声明与当前内容/来源及三份非空协议摘要匹配，reviewer ID不同且非作者，分歧需独立仲裁ID声明。不同字符串不等于真实独立人员，来源或协议变动后原声明失效。
- `formal_ready=false`、语言指标null：本工具从不放行正式评测，不执行划分/冻结/搜索，不额外打开heldout文件。真实身份、独立签署、保管/加密和语义审核仍待核验。

单人可完成原创草稿、个人标注、授权文件收集和本地改错。不能把自己两次审核算两位独立审核员，不能用AI审核代替真人gold，也不能让个人曾看过的任务继续冒充独立heldout。正式流程仍需[R8三门禁](../cl008-round8/README.md#当前可重放的阻断检查)，本队列不生成已批准handoff。

现语言计划`evaluator_id`由历史团队占位改为null；实际负责人仍由用户明确登记，不能把Git账号、Codex子任务或角色名称自动当已确认的人。保持Draft，不合main，不替换默认引擎，不进入CL-010。
