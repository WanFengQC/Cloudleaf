# Round 30 兼容性与交付核验

本地复验实现提交 `c5461c7ea160d872db82b0c514e7cbeff249bf36`。锁定工具链为 Rust 1.90.0 和 Edition 2024。基线的三项 Windows 恢复失败和后来安静重跑保留在 `.local/round21-runs/round29-final-04` 与 `.local/round21-runs/round29-verified-04`。退出码分别为 101 和 0，原始文件哈希在 `.local/round30-baseline/identity.json`。

| 检查 | 实际结果 | 原始记录 |
| --- | --- | --- |
| 历史搜索 | 837/837 完整结果与 Round 29 一致，188,325 次机械候选对照 | `.local/round30-legacy-search` |
| 参考与随机 | 2,186 条旧参考候选和 1,548 条随机参考候选的既有断言通过 | `.local/round21-runs/round30-reference2186`、`round30-reference1548` |
| 冻结研发评估 | 480 条完整结果一致，正式 Top-1、Top-5、MRR 均为 `null` | `.local/round30-language-regression` |
| 自动混排 | 默认关闭。68 条轨迹，共 933 个事件与 1,676 行。除耗时外完全一致，研发集为 55/68 首选、66/68 Top-10 | `.local/round21-runs/round30-mixed-replay` |
| Marian 双向与风险 | 63/63 真实输出及 51/51 机制对照与 Round 29、25 一致 | `.local/round30-translation-real` |
| 实际 Bridge | Worker 退出后发起任务成功、取消通过、Unknown 保持隔离 | `.local/round30-bridge-actual` |
| 独立入口 | CLI、设置宿主与研究释义通过固定配置和修订隔离回放 | `.local/round30-entry-replays`、`.local/round21-runs/round30-definition-real` |
| 双宿主词典生命周期 | 固定真实 CC-CEDICT、Unihan、补充层及 lexical，在两个无窗口宿主验证切换、回退、损坏拒绝、旧状态隔离与用户资料保护 | `.local/round21-runs/round30-resources-exact`、`round30-host-exact` |
| 真实无窗口 Worker 与 DLL | IPC 和现有 Windows 宿主实测双方向，真实本轮 DLL 用独立假 manager 完成装载与资源退还 | `.local/round21-runs/round30-worker-ipc-real`、`round30-host-translation-real`、`round30-dll-loader-real` |
| 本地 16 项 | 全部返回 0。全目标 Rust 486 通过、0 失败、7 条件忽略；Python 215 通过 | `.local/round30-checked-results.json` |

固定资源的不确定频率保持 `unknown`。ResearchOnly 补充词典仍默认关闭。回放没有改变离线权重、生成配置与默认评分。设置/候选、独立面板、平台适配、用户词库、活动词典与学习继续依赖现有隔离契约。

本轮第一版 `2d90d71` 的全目标 Clippy 返回 101，原因为音节弧索引循环触发 `needless_range_loop`。首批原始记录保留在 `.local/round21-runs/round30-verified-03`，随后中止该版本的回归。修复改用顺序不变的切片迭代，最终实现按全套命令复验。初版性能及 Worker 原始目录亦单独保留。

## 精确 push CI 与归档

Windows/Linux job、四份工件及原始 CI 日志的可信记录，以包含本报告的最终提交为绑定目标。交付时使用 `.local/round30-ci/final/summary.json`、`.local/round30-ci/final-downloads/downloads.json` 和 `.local/round30-delivery-receipt.json` 核验：每份工件独立计算 SHA-256 和 ZIP CRC，依赖清单及许可文件对照同一提交，逐文件验证来源锁，并实际在临时目录走升级、损坏拒绝和回退。该回执为生成于 push 完成后的记录。

真实应用与桌面保持 `not-run/user-paused`。TSF、长测与系统安装未执行。macOS 和 Linux 的系统输入法适配仍 `not-implemented`。CL-010 处于关闭状态。
