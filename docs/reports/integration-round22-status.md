# Round 22 原生翻译整合状态

**partial**。继续使用 `codex/integration-round21`。Round 21 基线为 `61dbe04ca10c9d175be7ca5fc117d8dc6fb59d39`。本轮实现提交为 `682cf261b9069380d3a5c66c385f0101ee7e591f`。最终归档提交、四个精确 push CI 产物及原有分支核对以 Git 外回执为准。

| 范围 | 状态 | 结果 |
| --- | --- | --- |
| 原生隔离宿主 | done | 沿用 `--interactive-input-host` 和实际、投影两个引擎。新增可见方向、捕获高亮候选、英文缓冲区、进度、取消、原文/译文、风险确认和错误状态。正常键位保留。 |
| 提交事务 | done | 双引擎先只读预检，沿用语义比较及宿主文档提交。Applied 只写一次，NotApplied 允许显式重试或保留输入，Unknown 保留核对材料并拒绝重放。译文无学习凭据。 |
| IPC 与真实 worker | done | 继承匿名管道，协议版本 1，128 KiB 帧、16 KiB 输入及输出、现役请求上限 2。后台 I/O，按需启动，绝对同目录程序、握手、请求代次、拒绝迟到或重复结果、有界回收。 |
| 模型 | done | 固定授权本地权重双向推理。方向仅驻留一份，复用同方向。选中权重始终经完整 SHA 校验。缺失、损坏、截断、超时和取消有明确错误。发行保持 research_only。 |
| 语义保护 | done | 金额、数字、日期、时间、单位与显式术语结构化比较；保留原始译文，风险确认绑定当前任务及结果版本。 |
| 性能 | partial | 已有同机配对复测及进程资源记录，活跃推理延迟仍有尾部开销。 |
| 在线路径 | partial | 原翻译面板保留显式授权与系统凭据。隔离输入宿主只使用离线 worker。真实在线验收未执行。 |
| 质量与未整合能力 | blocked | 独立 gold、正式语义发布、PR #4 长期准入仍分别保留在原方向。正式 Top-1/Top-5/MRR 均为 null。 |
| 真实 Windows 窗口 | not-run/user-paused | 有原生生产代码和无窗口回归。人工验收矩阵 16 项全待执行。 |
| TSF 与长期压力 | not-run | TSF 注册、激活、安装、卸载、默认输入法更改、8h/24h 与物理断网验证未执行。 |

## 本地回归 — done

固定 Rust 1.90.0、Edition 2024，完整 16 项命令返回 0。Rust 全目标 413 通过、3 忽略，Python 82 通过。格式、严格 Clippy、文档测试、locked/offline release、两个许可和 RustSec 审计、九份 vendor、基准与示例工具均通过。真实模型测试按授权材料独立启用，区分 CI 无权重结果。

搜索 837 对完整结果全等，预算失败 0/837，188,325 次机械参考完整候选比较无差异。跨键缓存默认关闭。合成用户词库写入、重新获取写锁及原有学习测试继续通过。

本轮保留 IPC 解析拒绝未知字段、重复回收误报、Escape 后重试状态滞留及术语日期边界的首次失败证据。完整首轮查到 Windows 隐藏焦点夹具错误解读 SetFocus 返回的旧句柄，以及许可诊断措辞兼容问题。夹具现在直接断言目标焦点，原有行为断言保留，许可范围保持严格。[SetFocus 契约](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setfocus)

依赖锁仅新增本地包与已在锁内的轻量序列化边。输入侧 Cargo 闭包验证未引入模型加载、Candle、Tokio 或联网后端。

## 交付与后续门禁

本轮设计、人工验收矩阵、原始命令和 SHA 证据保留。最终 CI 以回执的精确提交为准。main、原六条开发线、五个 Draft 和 CL-010 沿用原边界。

Round 23 优先分离调度等待、输入搜索与张量计算的并发开销，复核完整性检查在独立样本下的误报/漏报。获得明确授权后，按矩阵执行真实窗口交互并验证私有数据与异常恢复。

[性能报告](integration-round22-performance.md) · [关键语义](integration-round22-translation-integrity.md) · [人工矩阵](../testing/round22-native-translation-manual.md) · [证据索引](integration-round22-evidence/index.json)
