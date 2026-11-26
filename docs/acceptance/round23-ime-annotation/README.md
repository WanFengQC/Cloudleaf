# Round 23 空白人工输入法任务

`blank-tasks.json` 提供 24 个未划分的场景槽。来源、作者、真实意图、拼音、允许答案、真实按键轨迹及评审字段均待人工填写。任务包保留与现有 C 标注及 B 作者队列相同的协议。任务槽是收集起点，正式分母由保管者冻结的家族索引决定。

## 提交给作者与审核者

1. 将空白包复制到获准的 Git 外私有目录。来源提供者填写来源标识、作者、版本、全局家族及原件 SHA，交付授权文件及评估、修改、再分发范围。只有同意并获准使用的文字可以进入任务。
2. 作者按场景提供真实原文及输入意图，填写实际输入模式、候选允许答案、标准拼音及完整按键轨迹。用 `scenario` 记录模式和场景。不要从词典推导正确答案。不要录制日常私人输入历史。
3. 两位独立审核员分别审阅意图、答案、按键和上下文。审阅绑定当前来源、任务与三协议摘要。任何拒绝或分歧交由与作者及审核员不同的仲裁者处理；改动内容后重新审核和绑定。
4. 由独立保管者检查同源、镜像、近重复及同意图家族，冻结 70/15/15 家族划分和至少 2,000 个 heldout 家族。开发者只收到经核准的 train/dev 及摘要索引。正式 heldout 答案由保管者加密封存，开发者不接收其明文。

## 私有队列的可复核命令

```powershell
$packet = 'ABSOLUTE_PRIVATE_PATH/blank-tasks.json'
$sha = (Get-FileHash -Algorithm SHA256 -LiteralPath $packet).Hash.ToLowerInvariant()
python -X utf8 scripts/manage_ime_annotation_queue.py export --packet $packet --sha256 $sha --output ABSOLUTE_PRIVATE_PATH/authoring.json
python -X utf8 scripts/manage_ime_annotation_queue.py status --queue ABSOLUTE_PRIVATE_PATH/authoring.json --output ABSOLUTE_PRIVATE_PATH/authoring-progress.json
```

保管者在自己的受控环境执行 `python -m tools.data_pipeline.annotation_workflow` 的审核与冻结操作。该冻结工具生成的 heldout 明文只留在保管者环境；正式交接须另行验证加密封存、角色与授权。公共交接只包含摘要和可见任务，具体文件及参数见 [PR2 人工交接说明](../cl008-round8/README.md)。

当前来源及人员未交付，任务标记 `pending`。进度工具正常返回只证明程序执行。正式 Top-1、Top-5 和 MRR 仍为 `null`。
