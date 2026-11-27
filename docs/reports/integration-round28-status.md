# Cloudleaf Round 28

基线为 Round 27 `a5675b0e88fb4ea9f0e88061350f4a255beb98c5`，开发分支为 `codex/integration-round21`。工作区边界和验收条件见 [设计](../integration-round28-design.md)。

| 方向 | 证据位置 | 退出边界 |
| --- | --- | --- |
| 简拼 | [候选格与性能](integration-round28-simplified-input.md) | 固定预算的两个已知目标与简拼延迟门禁 |
| 混排 | [冻结研发轨迹](integration-round28-mixed-input.md) | 默认关闭，独立语言金标仍缺 |
| 配置 | [设置运行时](integration-round28-settings-runtime.md) | 隔离 Windows 宿主的热应用；真实 TSF 待验收 |
| 词典 | [资源管理](integration-round28-dictionary-management.md) | 分槽核验已实现；基础与补充词典独立加载和回退待实现 |
| 长释义 | [原生面板](integration-round28-definitions-ui.md) | ResearchOnly；真实桌面待验收 |
| Worker | [171 键并发](integration-round28-worker-performance.md) | 单线程、正常优先级、同轮倍率门禁 |
| 发行 | [隔离发行](integration-round28-release.md) | unsigned；正式证书及系统安装器待交付 |

## 完成与验收状态

- **done** · 运行宿主设置回执和安全热应用、独立长释义面板、隔离资料迁移与签名请求。代码级完成范围和手工边界见各专项。
- **partial** · 简拼候选解释分配下降，171/243 键简拼 P95 为 17.439/16.067 ms，仍超过 5 ms。8 轮 171 键真实推理配对的三项绝对时延达标，P95 倍率为 1.533×，仍超过 1.5×。
- **partial** · 68 条混排研发轨迹的 Top-1 为 55，Top-10 为 66。新规则把 `check` 从 Top-10 外提升至第 2；`meeting` 仍列第 7。混排默认关闭。
- **blocked** · 固定 16,384 条图边预算下两个已知简拼目标均未恢复。正式语义、语言质量、分发许可和签名也各自受原门禁约束。
- **not-implemented** · 基础及补充词典单独加载、运行时选择及回退入口；正式签名安装器；macOS、Linux 系统适配。
- **not-run** · 真实桌面、TSF 操作和 8h/24h 长测。

## 精确验证

产品实现 `e46cfc60b901739cd5f1d65185b7a675d8102e38`。研究索引测试随后修正语言和命名空间绑定，最终本地全套检查在 `b59c95bbf5ab7332731095cc95a7d75f634f5056` 执行。固定专项与两个提交的对应关系见 [证据索引](integration-round28-evidence/index.json)。

| 检查 | 结果 |
| --- | --- |
| 格式、Clippy、Rust 全目标与文档、locked/offline Release、Python、工作区、许可与依赖审计 | 16/16 返回 0；Rust 473 通过、0 失败、5 条件忽略；Python 209 通过 |
| 搜索历史对照 | 837/837 完整结果相等，预算失败 0；188,325 次机械完整候选对照 |
| 独立参考 | 2,186 个旧参考候选和 1,548 个随机参考候选通过 |
| 冻结研发语言集 | 480/480 完整结果保持相等，原始失败分母保留 |
| 固定翻译与 Bridge | 63/63 真实双向推理输出一致，51/51 对照一致，Bridge 恢复路径通过 |
| 无窗口产品回放 | CLI、Windows 隔离输入宿主、Bridge、固定语义索引正反查、68 条混排回放通过 |
| 发行 | 480 成员的双版本升级、损坏回滚与卸载演练；真实读写设置、词条及学习的迁移通过 |

逐命令及原始日志留在 `.local/round21-runs/round28-verified-01` 至 `round28-verified-16` 及各专项路径。首次失败记录包括混排错误切分、图解释用例预期差异和真实索引测试的命名空间假设。CI 用本报告提交的精确最终 HEAD 核验，原始 GitHub 状态、四份工件、日志 ZIP、SHA-256、ZIP CRC、许可、来源锁与依赖清单另记到本地 `.local/round28-delivery-receipt.json`。Round 27 的原始 CI 和四份工件及日志已重新核验。

正式 Top-1、Top-5、MRR 均为 `null`，生产词频为 `unknown`。真实桌面、TSF 和 8h/24h 长测均为 `not-run`。CL-010 保持关闭。
