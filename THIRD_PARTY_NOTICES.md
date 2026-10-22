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

CL-007 Windows 平台分支新增 Microsoft 官方 [`windows-rs`](https://github.com/microsoft/windows-rs) 绑定，直接依赖精确锁定 `windows 0.62.2` 和 `windows-core 0.62.2`。全部15个外部包的名称、准确版本、来源与 SPDX 表达式见 `config/audits/windows.json`；31份原始许可文本及 SHA-256 见 `licenses/windows/manifest.json`。绑定负责 COM/Win32 接口投影，未引入第三方输入法候选算法。平台模块以当前 Rust 1.90.0 实际编译验证；核心 crate 继续不依赖 Windows 绑定。

这些包的 MIT/Apache-2.0 双许可文本一并保留；`unicode-ident` 另须保留 Unicode-3.0 文本。源码包副本与 Cargo registry 原件逐字节核对，安全审计使用固定开发工具 `cargo-audit 0.22.2` 和 RustSec 数据库；结果与数据库提交身份见 CL-007 报告。安全审计只描述对应 Cargo.lock 和数据库快照，不能推断没有未来漏洞；开发审计工具不属于分发 DLL 的运行依赖。
