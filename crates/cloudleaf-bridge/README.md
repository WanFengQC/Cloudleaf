# 输入与离线翻译事务桥接

`TranslationBridge` 由宿主拥有，负责把一次显式翻译与当前输入会话、修订和词典资源绑定。候选触发中译英时，原有拼音预编辑和候选保留。英译中用宿主显式提供的英文编辑缓冲区。就绪的翻译结果必须经过显式确认，才沿用原有输入事务分配提交编号。翻译提交没有 `SelectionEvidence`。

本地提供者按需通过私有 IPC 启动同目录工作进程。工作进程验证固定 catalog、加载单方向模型并复用热模型。宿主用 `poll` 轮询。主动取消、编辑、资源或输入方案变化、失焦会使旧任务失效。NotApplied 需要显式重试，重试分配新提交编号。Unknown 隔离当前会话，待宿主核对后才能开始新会话。更换模型需确认旧工作进程已退出。无窗口命令在标准输入关闭后，通过 `shutdown` 回收后台线程与工作进程。

无窗口研发命令以固定词典、其 SHA-256、已经授权的本地模型 catalog 及其 SHA-256 为参数。目录和权重不会由该命令下载。例：

```text
cloudleaf-bridge-headless RUNTIME_TSV LEXICON_SHA CATALOG_JSON CATALOG_SHA < actions.jsonl
```

译前键入和就绪确认由下列 JSONL 显式动作控制，提交写入进程内文档：

```jsonl
{"op":"activate"}
{"op":"type","text":"nihao"}
{"op":"candidate","index":0}
{"op":"wait"}
{"op":"confirm"}
{"op":"applied"}
{"op":"english","text":"Hello!"}
{"op":"request-english"}
{"op":"wait"}
{"op":"confirm"}
{"op":"not-applied"}
{"op":"retry"}
{"op":"applied"}
```

`wait` 只在无窗口示例里等待。普通输入 `space`、`enter` 和数字选词仍经原来的输入事件处理。`cancel`、`blur`、`unknown` 和 `reset-unknown` 可用于独立事务验证，最后一项需要给出增长的 `session`。状态日志默认隐藏原文、译文与文档正文。只有显式添加 `--show-text` 才输出这些字段，适合本轮已知的非私人研发样例。Worker 状态和完整性检查项作为附加字段报告，`confirm` 仍为当前结果的显式提交操作。

桥接接受离线提供者，真实候选事务测试使用固定测试提供者。独立的原生翻译面板继续保留已授权的在线路径。本命令用于无窗口验证，Windows 窗口和 TSF 验收保持独立。
