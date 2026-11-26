# Round 27 翻译完整性与独立释义

## 无依据指令

Round 26 固定模型在一个独立问候的英文原文后增译邀请进入动作。旧完整性回归先返回 101，预期存在的发现数实际为 0。新规则要求原文是单独问候、译文有对应问候和显式分隔、分隔后有礼貌标记和已知独立动作。新增 `AddedCommandAction / Unverifiable`，带准确 UTF-8 译文范围。正常问候、称呼、礼貌语尾和原文已包含指令的负例保持。完整译文保留，面板、Bridge 和隔离宿主继续共享检查层；数字、术语、条件、时间、开关和原有等价表达判断沿用。

`cargo test -p cloudleaf-translation-integrity --all-targets --locked --offline` 返回 0。红测与绿测的命令、源码摘要、返回码和两路输出位于 `.local/round21-runs/round27-added-action-{red,green}`。新规则四个直接命中例和六个范围对照均通过。

## 固定权重和控制

以与 Round 25 相同的 catalog 与权重重放 63 条真实 Marian 推理，三组分母为 17、26、20。逐条核对原始译文，63/63 与冻结材料逐字一致。Round 25 固定规则的 12 个预置错误中 10 个提示，10 个等价表达中 1 个提示。角色交换、隐含排他、任意新动作推断和正式语言质量仍受限。报告区分直接等价、结构已变化和无法核验，单条没有触发规则的结果不形成语义质量证明。

回放工具 `tools/performance/round25_translation_replay.py`、实际命令、模型文件和三组原始运行记录在 `.local/round27-translation-real`。安全摘要 SHA-256 为 `0afadf28e71a08ffb6de1466a0f6edd7bb26af36a86d995f87e536f4416dfd29`。受测整合提交 `f3c7c9872de6906915af7eed3a9b67854d3f6e49`，catalog SHA-256 为 `f88a5f57513a0aaae3bbd659950adf042c8c8dde07736084cb951f61f22c7f0f`，未加入或下载新模型。

## 释义交互

Bridge 提供显式 `--semantic-index INDEX SHA` 和独立的 `define` 事件。在同一输入修订内返回中文词头或英文原始释义反查。返回包含词条/义项身份、来源断言、原始释义与有数据时的 POS，标记 ResearchOnly。检索结果单独返回且下一个事件清除，过期修订拒绝，默认输出仅保留计数与资源身份。查询不生成上屏、学习或翻译 Worker 请求。真正的长释义 Windows 窗口交互和正式词性数据仍待交付。
