# Cloudleaf Round 30

整合分支 `codex/integration-round21`。本轮基线 `8c92ea0be4882d69dda97c3642d81b312ef811a7`，Round 29 生产代码 `983a9be991b5e1de2a8d87d12fde8c98f5884538` 和 push CI [45972074923](https://github.com/WanFengQC/Cloudleaf/actions/runs/45972074923) 已复核。Round 30 实现提交 `c5461c7ea160d872db82b0c514e7cbeff249bf36`，指标和原始记录绑定见[证据索引](integration-round30-evidence/index.json)。

| 范围 | 状态 | 实际结论 |
| --- | --- | --- |
| 固定预算与候选事务 | done | 52 个历史触顶样本保留完整候选，含原 43 个恢复图。事件 78、79 第一，事件 79 空格提交为 Applied。349 个未触顶事件逐边、逐弧和完整候选比对一致 |
| 三种逆向构图策略 | done | 5,120 个小图和补充枚举支持按实际保留终点判定。全改动方案尾延迟回归已撤回 |
| 171／243 键简拼 | partial | 收窄方案减少重复终点检查，并降低累计分配；各轨迹的真实 P50/P95/P99/最大值、波动、尖峰和活字节代价见延迟报告。243 键简拼 P50/P95/P99/Max 为 8.265 / 26.417 / 32.183 / 37.525 → 8.733 / 25.074 / 32.613 / 38.563 ms。两条简拼轨迹均未达到 P95 5 ms 和 P99 10 ms 目标 |
| Worker | partial | 12 轮真实配对，汇总 1.499×，四项汇总门槛通过。单轮 5/12 满足倍率门槛。 |
| 设置、混排、词典、释义、翻译 | done | 历史行为及真实资源的无窗口保护回归通过，详见兼容报告 |
| 正式语言质量及词频 | blocked | Top-1、Top-5、MRR 为 `null`。词频继续为 `unknown`。ResearchOnly 补充层保持默认关闭 |
| 真实桌面、TSF、8h/24h | not-run | 真实桌面为 `not-run/user-paused` |
| macOS / Linux 系统输入法 | not-implemented | 跨平台 Rust 核心及独立适配层继续保留，CL-010 关闭 |

## 代码和回归

- `schemes.rs` 复用相交栈，用连续音节弧偏移索引，减少候选准入排名结构的重复更新。图触顶才预留发布缓冲并计算保留终点位图。选择范围依据相同词典上的独立配对。
- `search.rs` 的 PositionDag 读取准入后保留图的后缀可达性。截断、预算和原文回退继续随结果返回。评分延迟物化试验原证据保留，生产使用既有评分器。
- `suffix_admission.rs` 与 `position_materialization.rs` 扩充词典和独立 Reference 比对。Worker 工具使用固定种子打乱六轮空闲优先与六轮推理优先，保持候选与倍率定义。
- 本地 16 项检查全部通过。Rust 全目标 486 通过、0 失败、7 条件忽略，Python 215 通过。历史 837 组搜索、188,325 次机械候选对照及 480 条冻结研发结果逐项一致。68 条混排、63 条真实双向翻译与 51 条机制对照复验。Round 29 Windows 初次失败及同提交安静重跑的原始证据保留。

## 报告和交付

- [简拼候选格、边预算和穷举](integration-round30-simplified-search.md)
- [171／243 键延迟、分配和尾部事件](integration-round30-latency.md)
- [12 轮 Worker 配对与资源采样](integration-round30-worker-stability.md)
- [兼容、许可、精确 CI 与发行验证](integration-round30-compatibility.md)

Windows/Linux push CI 取包含本报告的最终 HEAD，四个工件与 CI 日志以工作流返回的身份逐个核验，原件、SHA-256、ZIP CRC、权限范围、来源锁、依赖清单、临时目录升级回退及远程分支对照由 `.local/round30-delivery-receipt.json` 记录。main 和六条开发线以预检和交付时的精确 refs 比对，五个既有 PR 保持 Draft。

## Round 31 建议

沿用固定的 16,384／200,000／50,000 预算，先为反复出现在尾部的 243 键事件分离词格写入、排序键比较与路径复制成本。保留峰值内存对照和已恢复候选。独立 Worker 重复批次仍须平衡运行顺序，正式准入继续依赖真实桌面、授权语料与人工语言证据。
