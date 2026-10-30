# 第三方依赖与资源登记

M1/M2 使用 Rust 1.90.0 标准库及 workspace 内部 crate，没有 crates.io、Git 或其他第三方 Rust 库依赖。工具链不是产品代码的来源，遵循 Rust 工具链自身的 MIT/Apache-2.0 等许可证。Python 3.11+ 仅运行开发检查与报告脚本，使用标准库，不参与输入路径。

| 资源 | 版本/来源 | 许可 | 分发要求 | 状态 |
|---|---|---|---|---|
| 项目 Rust 源码与手写拼音规则 | Cloudleaf 0.1.0，项目独立编写 | MIT | 保留 LICENSE | 已登记 |
| 原型词典 | `data/dictionaries/prototype.tsv`，项目手写示例 | MIT | 保留 LICENSE；见数据清单的 SHA-256 | 仅用于原型 |
| 输入固定语料 | `tests/fixtures/input-v1.tsv`，项目手写用例 | MIT | 保留 LICENSE；见数据清单的 SHA-256 | 仅用于回归 |
| 独立算法种子评估 | `tests/evaluation/algorithm-v1.tsv`，项目手写、未用于训练或调参 | MIT | 保留 LICENSE；见独立清单与划分规则 | 用于指标验证及基线消融，不是正式代表性语料 |
| 第三方词典、模型、字体、图标 | 尚未引入 | 无 | 引入前独立审查 | 未引入 |

新增依赖必须先登记准确版本、来源、SPDX 许可、署名要求、维护/安全审查结果与体积影响，再修改 Cargo.lock。`scripts/check_workspace.py` 将未登记外部依赖视为错误；零外部依赖时不下载审计工具或安全数据库。出现外部依赖后必须同时加入漏洞数据库审计，不能继续声称零依赖检查等同漏洞审计。

资源登记模板：名称、类别（代码/词典/语料/模型/字体/素材）、准确版本、上游 URL、取得日期、SHA-256、SPDX/授权文件、署名、修改内容、可分发范围、审查人及审查结果。不得把第三方数据改标为项目自编数据。

## Round 11独立翻译分支

新增`cloudleaf-translation`使用reqwest0.13.3、rustls0.23.46（ring）、tokio1.54.1、serde1.0.228、serde_json1.0.149、ring0.17.14和zeroize1.8.2。准确的136个跨目标传递包、版本/SPDX/registry来源在`config/audits/translation.json`及Cargo.lock；不是输入核心的新依赖，原六crate的生产依赖闭包保持本地。只在显式翻译worker执行HTTP，默认不联网，无随包在线服务或密钥。

原件242份及SHA见`licenses/translation/manifest.json`，包含MIT/Apache/ISC/BSD/Unicode/Unlicense/CDLA等实际包声明，保留上游全部随包LICENSE/COPYING/NOTICE类原文，不将这些库重新许可为MIT。jni、jni-macros、jni-sys-macros和Android verifier的crate未附原文，已按Cargo自带VCS固定提交取得8份上游MIT/Apache原件，HTTP来源/时间/内容SHA另见`upstream-acquisition.json`，不冒充随包文件。这些Android目标依赖不代表已实现Android平台。

技术审查核对了实际元数据、原件字节和已知漏洞，不代替完整供应链或法律审查。首次缓存版本rustls0.23.40/tokio1.52.3被RustSec拒绝，失败保留；改用上述补丁版后审计通过，没有忽略advisory或warning。开发审计继续固定cargo-audit0.22.2，与既有Windows方向同工具流程。

本地字典研究投影单独保留原来源的LICENSE/NOTICE和状态；CC-CEDICT/Unihan的既有批准范围不因代码实现扩大。未引入离线权重、虚构语义数据或人工gold，API生成译文不具有独立语言质量认证。

## Round 12 原生面板

新增 app 仅在 Windows 直接使用既有锁定的 windows-sys0.61.2/windows-link0.2.1，增加窗口/控件、DPI、凭据、内存、文件替换与剪贴板所需 features。无新增外部包或版本升级，MSRV1.90不变；原件已在上述242份清单中。系统字体通过 Windows 接口使用，没有复制或分发字体文件。没有引入浏览器运行时、GUI框架、模型运行库或权重；模型候选研究不是许可准入。
