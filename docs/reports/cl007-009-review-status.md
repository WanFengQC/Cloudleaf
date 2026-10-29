# CL-007/008/009 并行审阅状态

后续共享预算与恢复、完整候选差分、固定数据复现见[第三轮检查点](cl007-009-round3-status.md)；[第二轮检查点](cl007-009-round2-status.md)继续保留。下文保留第一轮固定版本及历史证据，不代表后续提交自动通过。

日期：2026-10-22。主分支 `main` 保持 `a564fd47b0f054ff360592af3ccb8d744021160f`。下面是独立分支的审阅检查点，全部 PR 仍为开放草稿；未合并或进入 CL-010。本报告不改变三个方向的原定退出条件。

## 版本与依赖

| 范围 | 分支及本次检查点 | 审阅入口 | 基底 |
|---|---|---|---|
| 共享契约 | `codex/cl007-009-contracts`，实现及证据至 `acdb34f` | [PR #1](https://github.com/WanFengQC/Cloudleaf/pull/1) | `main` |
| B / CL-008 | `codex/cl008-search`，`aa53da6caa547ae3124f1c7cf257598f5e13090e` | [PR #2](https://github.com/WanFengQC/Cloudleaf/pull/2) | 共享契约分支 |
| C / CL-009 | `codex/cl009-data`，`a68e03846f3de133068bbb133c77afb9955f9590` | [PR #3](https://github.com/WanFengQC/Cloudleaf/pull/3) | 共享契约分支 |
| A / CL-007 | `codex/cl007-windows`，`3579d2c576ed29285041128cd5cc0e0f90016859` | [PR #4](https://github.com/WanFengQC/Cloudleaf/pull/4) | 共享契约分支 |

先审共享事件、提交三态与数据格式，再逐方向审阅。PR #1 合并后，应重新核对 #2/#3/#4 的基底、差异及 CI；不能直接沿用堆叠分支的旧结果。方向之间不复制候选算法或各自修改共同接口。

## 已验证与未完成

| 方向 | 已完成的本轮范围 | 部分完成、未完成或阻塞 |
|---|---|---|
| A | 隔离原生 crate、COM/组合/候选实现；只读预判与同步授锁后处理；回调封闭、epoch 隔离和 Unknown 不重试/不误删；生产 COM 故障注入；依赖许可及 RustSec 快照审计 | 完整异步原生按键保全/重放、独立 watchdog、合并重复消息、宿主完整恢复、退出资源、真实应用、跨架构安装与长期兼容均未验收。禁止据此注册、激活或发布 |
| B | 全输入词条格、独立穷举参考、精确 DP/有界 Beam、加权/Bigram/上下文机制对照与预算；差分测试及报告 | 尚未接入默认引擎；机械数据不能证明中文准确率或最优算法。正式语料与独立语言评估缺失；纠错、模糊音、简拼、双拼、学习、增量仍需实施 |
| C | Unicode 17 字符源授权、来源锁定、导入/质量报告/索引/单写者本地原子回退；44,431 条记录的隔离双构建及9文件字节一致；损坏和报告语义校验 | kMandarin 字符读音不是完整词库、多音字或双语功能；真实现代词频未知，不填零。词语/短语、合法频率/训练及独立评测语料、正式语言质量尚未交付；签名与断电/崩溃恢复另验 |

没有删除既有回归测试或用合成数据替代正式数据。A/B/C 对应报告分别在各自分支：

- [A 状态与复现命令](https://github.com/WanFengQC/Cloudleaf/blob/3579d2c576ed29285041128cd5cc0e0f90016859/docs/reports/cl007/status.md)。88 项 Rust、16 项 Python 通过，Windows 原生28项包含生产 COM 故障注入；另1项 release 模拟 COM 基准显式运行通过。直接布置输入的 fixture 不是原生按键端到端证据，模块持有测试也不是完整 Deactivate/DLL 卸载证据。15个外部包和31份许可原件通过核对，RustSec 数据库快照0漏洞/0警告。
- [B 搜索实验报告](https://github.com/WanFengQC/Cloudleaf/blob/aa53da6caa547ae3124f1c7cf257598f5e13090e/docs/reports/cl008-progress.md)。72 项 Rust、17 项 Python 及1,260组确定性差分通过；参考与精确 DP 一致，有界 Beam 的截断/差异另报。所有机制成绩均标为非正式语言准确率。
- [C 干净复现报告](https://github.com/WanFengQC/Cloudleaf/blob/a68e03846f3de133068bbb133c77afb9955f9590/docs/reports/cl009-clean-replay.md)。60 项 Rust、34 项 Python 通过；干净 Git 归档、无第三方包 venv、隔离进程双构建。接受44,431条记录，18条合法源读音因当前音节表不支持运行时投影而隔离，源坏记录和冲突均为0；频率保持 unknown。两次构建9文件哈希一致；源数据统计与可独立验证的派生统计分别说明。

## 远程检查与证据边界

- 共享代码检查点 `acdb34f`：[CI 39716279234](https://github.com/WanFengQC/Cloudleaf/actions/runs/39716279234)，Windows/Linux 成功。本报告是后续文档更新，不能声称已包含在这一历史被测提交中。
- B 检查点 `aa53da6`：[PR CI 39717696624](https://github.com/WanFengQC/Cloudleaf/actions/runs/39717696624)，Windows/Linux 成功。
- C 检查点 `a68e038`：[PR CI 39734960161](https://github.com/WanFengQC/Cloudleaf/actions/runs/39734960161)，Windows/Linux 成功。
- A 检查点 `3579d2c`：[PR CI 39736023194](https://github.com/WanFengQC/Cloudleaf/actions/runs/39736023194)，Windows/Linux 成功；包括精确许可、RustSec、Python 检查和基准产物上传。

四个固定检查点的 job、步骤结果、产物大小及 digest 见[只读 CI 核实快照](cl007-009-ci.json)。这是固定版本的历史证据，不表示未来提交自动通过，也不将本次文档更新包含在旧运行中。

A 的核心和模拟 COM 基准各自保存数据/源码/二进制指纹，不是正式词库、TSF UI 或真实 Windows 应用延迟。C 的字符资源只用于有明确范围的结构与资源验证；B 读取该格式的独立测试不等于 CL-010 集成验收。

## 下一依赖与门禁

1. A 先关闭按键 ownership、顺序/满队列恢复、无消息超时与退出资源的故障证据，再考虑注册测试；随后必须在可验证的真实应用中验收。桌面工具无法确认目标时停止该自动化操作，不猜测 URL 或盲点坐标。
2. C 继续获取许可明确的词语/频率/语料，预先定义质量、兼容与迁移验收；不能凭下载可用或当前字符数量宣布正式词库完成。
3. B 保留原型和精确参考；使用合法独立数据、预登记划分和阈值评估质量/延迟/内存后，再设计默认引擎集成。heldout 不用于调参。
4. 共享契约、A 原生交互、B 生产算法与 C 正式数据分别满足退出条件后，才进入 CL-010 全链路集成。
