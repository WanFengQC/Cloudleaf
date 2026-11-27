# Round 28 Windows 长释义

Win32 面板新增独立「长释义」页。选择中文词语或短语查询、英文释义精确反查后，面板读取左侧原文。结果使用独立的可滚动只读控件，显示词头、词条与义项 ID、词性、原文释义和来源指针。已有词性直接展示，来源未给词性时保留缺失。

- `apps/cloudleaf-translation-panel/src/definitions.rs` 通过既有语义 reader 核验索引 SHA-256，缓存已验证快照。单个响应最多显示 112 KiB，达到上限时保留完整条目并标记条数。
- `setup.rs` 在设置线程查询索引，Window 事件队列只接收结果。 `controller.rs` 校验任务 ID、输入修订、焦点和关闭状态。切换查询目标和编辑原文使旧结果失效。定义结果独立于译文、剪贴板、候选提交和学习。
- `windows_ui.rs` 单独呈现读取、无结果与错误状态。语义词典依然标记为 `ResearchOnly`。

验证包括超过普通候选预览长度的定义、词条身份、词性缺失、真实 English gloss 精确反查、哈希失败和旧任务失效。固定研究索引为 34 条词头、78 条义项，SHA-256 为 `24ce0bd4a7169893a4ed82bc9a58d68a64c12915752afe637ceb9b0f7b1ef987`。固定索引第一次回放因测试把语言标签误当作目录命名空间而返回 101。按目录中声明的语言与命名空间重新绑定后，词头和反查均通过。原始失败和成功分别位于 .local/round21-runs/round28-definitions-research 与 .local/round21-runs/round28-definitions-research-fixed。索引原件保留在本地。原生滚动视觉效果、焦点交互和真实应用输入为 `not-run/user-paused`。正式语义内容为 `blocked`。
