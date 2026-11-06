# Round17 执行工具错误原文摘录

这是本轮工具调用已观察到的错误摘录，不冒充产品失败或独立测试通过。完整有效反例、stdout/stderr、命令、返回码另见各 runs/result.json；下列调用没有被完整包装，原对话工具输出保留。

- `python -X utf8 .local/setup17.py`，首次 exit1：`FileNotFoundError: [WinError 2] 系统找不到指定的文件。`。原因是Python子进程未用绝对cargo.exe路径；改为`C:/Users/EDY/.cargo/bin/cargo.exe`，保留同一固定Git archive，实际build exit0。
- 首次 `git apply --check .../tests-only.patch`：`error: dev/null: No such file or directory`。补丁缺`new file mode 100644`；补齐该头并保留UTF-8/LF，`--ignore-space-change`处理工作树CRLF上下文。之前还出现`error: patch failed: adapters/windows/candidate.rs:473`与`session.rs:2003`，未应用实际源码，不计为red。
- 首次错误包名 `cargo test -p cloudleaf-windows ...` exit101：`error: package ID specification cloudleaf-windows did not match any packages`，正确包名是cloudleaf-windows-ime，见后续有实际测试的日志。
- 错误过滤器 `isolated_window_and_owned`、`retained_material` 和未应用补丁前`isolated_input`均曾exit0但`running 0 tests`，全部排除有效通过分母。有效red/green使用实际函数或完整模块名称，分别运行1/1/7测试。
- `discover17.py cfedc80a snapshot-implementation` exit1：`urllib.error.HTTPError: HTTP Error 422: Unprocessable Entity`。改为实际完整SHA cfedc80f14e4ea7773ffed335ffe5fccfa5c2fcc；精确push记录实际取得。
- 临时Host测试曾将NotApplied后Space误当显式重试，真实core拒绝该期望；测试改用既有RetryCommit，未修改核心。临时数据审阅曾比较不同工具版本的traditional-b/c，保留失败，重建d与c后同字节。上述原始日志在host-temp/runs及corpus/tools/audit-execution-record.json。

本轮没有根据链式shell最终返回码判定所有子命令通过。真正13项交付检查全部独立包装并逐项记录returncode，red和空过滤均单独保存。
