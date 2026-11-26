# Round 26 离线 Worker 故障与恢复

**Bridge Windows Release 故障回放通过。** 交互路径发出取消或停止接单。后台任务与子进程回收可分别观察。只有证实旧进程退出，后续显式请求才能启动新代。已取消、已过期和提交结果 Unknown 的事务保持隔离。

| 情形 | 观测结果 | 证明范围 |
| --- | --- | --- |
| catalog 缺失或 SHA 不符 | ModelNotFound 与 OfflineCatalogMismatch，两个用例均无子进程。原文保留，随后 `nihao` 可出候选。 | Bridge Release |
| 同目录 Worker 缺失 | OfflineWorker(InvalidWorkerPath)。翻译为空，普通输入仍可查询。 | Bridge Release 隔离目录 |
| 模型权重损坏 | 在 catalog 的独立副本写损坏权重，得到 OfflineWorker(ModelInvalid)。启动后的失败仍能继续普通输入。 | Bridge Release，原始权重未改 |
| 推理中取消、原文变更 | generation 1 / request 4 到 Translating 后取消。无译文、保留原文。显式 request 5 得到结果；改原文后确认返回 StaleConfirmation。 | Bridge Release |
| 推理中 Worker 意外退出 | 按实际 Bridge 报出的 PID 打开并校验进程句柄，随后终止和等待退出。generation 1 / request 6 返回 OfflineWorker(WorkerExited)，PID 清空、原文保留、无自动重放。显式请求启用 generation 2 / request 1；EOF 后持有的第二个进程句柄确认退出。 | Bridge Release |
| 提交 Unknown | 状态 CommitUnknown，显式 Retry 返回 OutcomeUnknown，文档未写入，开启新会话后仍无重复文本。 | Bridge Release |
| 版本、握手、协议和队列 | 版本错误、超大帧、截断帧、非法 UTF-8、不同 generation、队列上限和取消由现有 Worker IPC 测试覆盖。HandshakeTimeout 有独立代码；Bridge 本轮未单独模拟恶意握手的全部变体。 | 定向 IPC 测试及实现审查 |
| 任务超时、输出超长及模型初始化失败 | Timeout、ResponseTooLarge、ModelLoadFailed 保持独立。超时拒绝晚到结果有定向回归。本轮未逐个注入三个产品入口。 | 定向回归及实现审查 |
| 监督器退出但子进程状态无法证明 | 保留 PID，报告 WorkerReapFailed 并阻止替换。两后台线程回归确认关闭前只能返回 Busy，结束后回收错误在后续检查中保留，hook 调用一次。 | IPC 和翻译服务的注入测试 |
| 在线 API 失败、真实窗口及系统 TSF 恢复 | 需要独立入口证据。 | 本报告未执行 |

受测源码 `c765e634342b19f40fbde2ac4e2043187373a9ef`。Bridge、数据、工具和模型 SHA 见 [提供者统一](integration-round26-provider-unification.md)。真实故障回放原始目录 `.local/round26-bridge-worker-real/`，包含每种用例的标准输入、输出、错误和返回码，五个进程返回码均为 0。正常用例 stdout JSONL SHA-256 为 `ce4b5ba263c8744bde139fd82fab885843524e36f797544ca65ef66c1496ae49`。定向 Rust 测试、严格 Clippy、构建与实际测试命令及哈希见 `.local/round21-runs/round26-{adapter-exact-tests,adapter-exact-clippy,bridge-release,bridge-worker-real}/result.json`。

全回归首次在共享构建负载下，既有 `native_empty_composition_delete_has_the_same_unknown_boundary_as_nonempty_writes` 的注入收到 `MonitorFailed`，预期为 `Unknown`，返回 101；其后独立精确复测 1/1 通过。该探针上的守望线程有 250 ms 同伴心跳约束，首次结果可能涉及并发调度，原断言和生产逻辑均保留。该事件与隔离宿主实际模型测试中模型身份的错误测试断言是两个独立故障。
