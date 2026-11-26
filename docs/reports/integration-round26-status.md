# Cloudleaf Round 26 整合交付

整合分支为 `codex/integration-round21`。对照 Round 25 最终 `d56ce7167b10d7ee96ff341bb27573cdfcc7a8bc`、实现 `064fe68d8b5c9b8b98faa21f2cf73fe2cb2f73c4` 和 [Windows/Linux push CI](https://github.com/WanFengQC/Cloudleaf/actions/runs/45782325057)。

| 方向 | 状态 | 实际交付 |
| --- | --- | --- |
| 输入热点与优化 | done | 载入时存相同定点预评分，lexical PositionDag 按候选格边复用同次查询结果。默认词库、排序、预算及跨键缓存开关沿用。[输入延迟](integration-round26-input-latency.md) |
| 并发与真实门槛 | partial | 独占 A/B 的 171、243 键 lexical 空闲 P95 分别降至 1.75、2.43 ms，推理并发 P95 为 3.42、4.78 ms。并发倍率 1.95×、1.97× 均高于 1.5× 门槛。各策略与共享负载见 [Worker 配对](integration-round26-worker-isolation.md) |
| 三个入口的统一离线链路 | done | 独立面板与 Bridge 切换到独立 Worker；Windows 隔离宿主沿用同一 IPC。三个入口均实际双向推理，面板 63 条原始译文逐字一致。进程与任务身份及故障证据见 [统一适配](integration-round26-provider-unification.md) 和 [恢复](integration-round26-recovery.md) |
| 回归、配置与旧功能 | partial | 输入与翻译依赖闭包分离，维持设置兼容、显式原文与提交确认。统一设置中心和自动中英混排未实现。[兼容性](integration-round26-compatibility.md) |
| 正式词频、独立 gold、模型分发 | blocked | 正式词频 unknown。中文 Top-1、Top-5、MRR 均为 null。现有 Marian 固定权重用于本地研发，独立模型与数据分发审批待补 |
| 桌面、TSF、长测 | not-run | 桌面继续 user-paused；未执行 8h/24h，也未注册或修改系统输入法 |

阶段设计提交为 `abab11f` 和 `816bada`。搜索实现为 `52c99b3`、`c929e61`，共享 Worker 源码及入口回归起始提交为 `5669b26`，调度矩阵工具为 `c765e63`，实际证据核验修正为 `bb58b28`。实现源码、检查和原始研究临时目录分别绑定身份。主分支和旧 Draft 逐项通过远程核对后生成 Git 外回执。

[50 项产品需求与验收](cloudleaf-product-readiness.md)。CL-010 保持关闭。

## 本地回归与定版

源码在 `600ff017a32b8ecf970e4478abf266dedb29e8f2` 上完成第二次完整回归。16 条命令均返回 0，Rust 443 通过、4 个条件用例按默认配置忽略，Python 202 通过。Rust 文档测试、release、工作区、Windows 依赖、翻译依赖、许可、vendor、RustSec 与资源检查均成功。首轮并发下的既有 watchdog 测试失败仍保留，独立复测和第二次工作区全量测试均通过。另运行固定模型 Release IPC ignored 测试与隔离宿主真实模型 ignored 测试，两项各 1 通过。原始退出码、数据、工具和二进制 SHA 见[证据索引](integration-round26-evidence/index.json)。私有研发逐字原件保留在索引指定本地目录。

最终证据提交只增加记录。其自身的 push Run、四份工件及归档 SHA、文件 CRC 和许可内容逐项绑定于 Git 外 `.local/round26-delivery-receipt.json`。仓库内没有打包模型权重。
