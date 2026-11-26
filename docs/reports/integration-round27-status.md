# Cloudleaf Round 27 整合交付

在 `codex/integration-round21` 基线 `a6e48cbe28d48a010629a65537cfb6743522356a` 上实施。Round 26 原本地回执、远端双平台 push CI 与四份包 CRC/SHA 已重新核对。[Round 26 CI](https://github.com/WanFengQC/Cloudleaf/actions/runs/45798911452)

| 方向 | 状态 | 实际行为与门禁 |
| --- | --- | --- |
| 简拼候选格 | partial | 保全触顶起点已接收边，跳过必空展开，缩减重复分配。两轨仍有 7/171、45/243 个真正截断，原文可保全。[简拼](integration-round27-simplified-input.md) |
| 真实 Worker 并发 | partial | 单线程正常调度配对；用同轮、同轨迹和版本的空闲延迟作分母。[并发](integration-round27-worker-performance.md) |
| 自动中英混排 | partial | opt-in，固定真实词典 16 条研发轨迹首选 14、Top10 为 15。部分小写英文仍有错误解释，真实应用未验收。[混排](integration-round27-mixed-input.md) |
| 统一设置 | partial | 已有 Win32 设置控件、共享输入偏好、备份与原子保存、词库编辑与恢复、离线和在线授权、Worker 状态及 CLI/隔离宿主加载。跨进程热应用、统一基础/补充词典管理和离线自动方向仍待接入。[设置](integration-round27-settings.md) |
| 翻译与释义 | partial | 增译提示覆盖已复现独立问候反例，63 条固定模型原译文保持。Bridge 独立释义检索有修订门禁，Windows 长释义界面未交付。[翻译](integration-round27-translation-integrity.md) |
| 发行演练 | partial | 两个实际历史 CI ZIP 的临时目录试装、升级、回退和卸载通过。正式签名发行和系统环境安装验收未执行。[发行](integration-round27-release-preparation.md) |
| 正式语言数据、模型分发 | blocked | 中文词频 unknown，独立中文 Top-1、Top-5、MRR 保持 null，未经放行的词典和模型未加到正式包。 |
| 真实 Windows 与长期稳定性 | not-run | 桌面继续 user-paused，系统 TSF 和 8h/24h 未执行，CL-010 关闭。 |

总设计为 `31ac596`，专项设计为 `42a96d8`，翻译增译与隔离发行器起始实现为 `f3c7c98`，简拼、混排、设置及产品入口接入为 `e333f79`。原有资料和失败证据均保留。输入核心继续平台无关；输入路径不读取语言、模型或网络资源。具体修改文件、测试命令、原始日志和哈希见各专项及最终证据索引。

[50 项产品要求追踪](cloudleaf-product-readiness.md)。
