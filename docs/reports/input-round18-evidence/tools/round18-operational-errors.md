# Round18 工具与操作错误

保留原始实现失败见各 runs 和临时方向日志；本文件只补充交付操作中未进入 run18 包装器的错误。

- `Get-Content -Encoding UTF8 .local/round17-archive.py -TotalCount 140`：exit1，路径不存在。随后 rg 查得实际入口是 `.local/archive17.py`，未创建空入口。
- `Get-Content .../runs/profile-1-abbreviated-lexical/stdout.txt`：PowerShell ItemNotFound，文件实际不采用该猜测名；改读 summary/index 及真实目录，不推测内容。
- `python .local/download_ci18.py b948f986c4c9d00bec7cdb7845283c83e6a87970 43191081791 implementation-downloads`：exit1，在行49 `assert run['head_sha']==head and run['event']=='push' and run['conclusion']=='success'` 抛出 AssertionError。当时Windows job仍in_progress；下载器按原门禁拒绝。后来独立新目录重新执行，不放宽断言。
- F8两次实际自动化未切换纠错，原窗口轨迹保留。原因不能仅凭观察确定；未修改系统设置。新增owned按钮实际验证成功。
- 首次纠错窗口将独立 shell 只读命令与按键交错，出现失焦原文保全；UIA名称混合状态信息和文档，最初口头误判为提前提交。实际 EDIT Value 和后续画面证明n/i只在恢复区，未写入文档；连续复测、候选提交和Unknown轨迹分开保存。
- 早期临时解释器初次Clippy原始日志被临时任务覆盖，只保留错误摘录，不能把摘录称作完整原文。首轮完整来源差分592失败、图/搜索预算失败和后续green均完整保留。
- `python .local/finish_metadata18.py` 首次exit1，在行16读取Host记录不存在的`events`字段，`KeyError: 'events'`；实际分母应为`len(timings_ns)`，修正元数据工具后重跑，不改Host原始结果。先前已生成的窗口完成记录按原字节保留。
- `python .local/archive18.py --plan` 首次exit1，行222 `subprocess.run(command, cwd=ROOT, capture_output=True)` 抛出 `FileNotFoundError: [WinError 2]`。该shell没有cargo/rustc的PATH入口；改用本机已验证的`.cargo/bin/*.exe`与当前Python绝对路径。该次为只读计划，未写实际归档。
