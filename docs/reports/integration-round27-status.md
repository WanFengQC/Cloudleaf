# Cloudleaf Round 27 整合交付

在 `codex/integration-round21` 基线 `a6e48cbe28d48a010629a65537cfb6743522356a` 上实施。Round 26 原本地回执、远端双平台 push CI 与四份包 CRC/SHA 已重新核对。[Round 26 CI](https://github.com/WanFengQC/Cloudleaf/actions/runs/45798911452)

| 方向 | 状态 | 实际行为与门禁 |
| --- | --- | --- |
| 简拼候选格 | partial | 保全触顶起点已接收边，跳过必空展开，缩减重复分配。两轨仍有 7/171、45/243 个真正截断，原文可保全。[简拼](integration-round27-simplified-input.md) |
| 真实 Worker 并发 | partial | 66 组无窗口真实配对和生命周期执行全部完成。171 键并发比 2.06×，243 键 1.30×。两组绝对延迟均达标，171 键倍率未达标。[并发](integration-round27-worker-performance.md) |
| 自动中英混排 | partial | opt-in，固定真实词典 16 条研发轨迹首选 14、Top10 为 15。部分小写英文仍有错误解释，真实应用未验收。[混排](integration-round27-mixed-input.md) |
| 统一设置 | partial | 已有 Win32 设置控件、共享输入偏好、备份与原子保存、词库编辑与恢复、离线和在线授权、Worker 状态及 CLI/隔离宿主加载。跨进程热应用、统一基础/补充词典管理和离线自动方向仍待接入。[设置](integration-round27-settings.md) |
| 翻译与释义 | partial | 增译提示覆盖已复现独立问候反例，63 条固定模型原译文保持。Bridge 独立释义检索有修订门禁，Windows 长释义界面未交付。[翻译](integration-round27-translation-integrity.md) |
| 发行演练 | partial | 两个实际历史 CI ZIP 的临时目录试装、升级、回退和卸载通过。正式签名发行和系统环境安装验收未执行。[发行](integration-round27-release-preparation.md) |
| 正式语言数据、模型分发 | blocked | 中文词频 unknown，独立中文 Top-1、Top-5、MRR 保持 null，未经放行的词典和模型未加到正式包。 |
| 真实 Windows 与长期稳定性 | not-run | 桌面继续 user-paused，系统 TSF 和 8h/24h 未执行，CL-010 关闭。 |

总设计为 `31ac596`，专项设计为 `42a96d8`，翻译增译与隔离发行器起始实现为 `f3c7c98`，简拼、混排、设置及产品入口接入为 `e333f79`。CLI 在坏词库下保全字面输入的修正为 `9be36f2`。原有资料和失败证据均保留。输入核心继续平台无关；输入路径不读取语言、模型或网络资源。具体修改文件、测试命令、原始日志和哈希见各专项及最终证据索引。

[50 项产品要求追踪](cloudleaf-product-readiness.md)。

## 源码与回归证据

837 组历史完整搜索、188,325 次机械完整候选、2,186 个旧参考候选、1,548 个随机参考候选均一致。固定研发语料四组各 120 条，共 480 条，答案路径、排名、完整性和预算诊断一致。差分移除时间列。默认搜索配置和回退语义通过专项及全量测试。两条真实并发轨迹按同轮同策略门槛计算，171 键并发 P95 由同轮基线 2.70 ms 升至 3.95 ms。保持 `partial`。

发行、模型、设置、输入和研究索引各项证据分别绑定原件、工具和产物身份。正式词频、独立人工 gold 和真实窗口门禁仍按各自验收条件跟踪。

源码验收提交为 `15f822acc960ca954069c6c08cb8d6dc8b00a104`。本地复跑 16 条命令均返回 0。Rust 全目标 461 通过、0 失败、4 个条件用例默认忽略。Python 206 通过。锁定离线构建、静态检查、文档、Windows 闭包、翻译 223 个依赖许可、RustSec、工作区与 vendor 检查均通过。首轮发现设置所属宿主边界和共享许可清单两项差异，均已修复并保留失败记录。所有本地命令及摘要关联到[证据索引](integration-round27-evidence/index.json)。

最终资料提交的精确 push CI、四份包和下载日志、包 SHA/ZIP CRC、许可证和来源锁核验记录放在本地 `.local/round27-delivery-receipt.json`。分支和 Draft 状态在提交前后以远程原件核对。
