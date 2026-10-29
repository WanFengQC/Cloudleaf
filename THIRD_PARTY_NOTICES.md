# 第三方依赖与资源登记

M1/M2 使用 Rust 1.90.0 标准库及 workspace 内部 crate，没有 crates.io、Git 或其他第三方 Rust 库依赖。工具链不是产品代码的来源，遵循 Rust 工具链自身的 MIT/Apache-2.0 等许可证。Python 3.11+ 仅运行开发检查与报告脚本，使用标准库，不参与输入路径。

| 资源 | 版本/来源 | 许可 | 分发要求 | 状态 |
|---|---|---|---|---|
| 项目 Rust 源码与手写拼音规则 | Cloudleaf 0.1.0，项目独立编写 | MIT | 保留 LICENSE | 已登记 |
| 原型词典 | `data/dictionaries/prototype.tsv`，项目手写示例 | MIT | 保留 LICENSE；见数据清单的 SHA-256 | 仅用于原型 |
| 输入固定语料 | `tests/fixtures/input-v1.tsv`，项目手写用例 | MIT | 保留 LICENSE；见数据清单的 SHA-256 | 仅用于回归 |
| 独立算法种子评估 | `tests/evaluation/algorithm-v1.tsv`，项目手写、未用于训练或调参 | MIT | 保留 LICENSE；见独立清单与划分规则 | 用于指标验证及基线消融，不是正式代表性语料 |
| 搜索机械符号映射 | `tests/search-mechanics-v1/lexicon.tsv`，项目自编13条符号映射 | MIT | 保留 LICENSE；独立清单校验 | 仅机制测试，unknown为空，不是产品词库 |
| 搜索合成转移计数 | `tests/search-mechanics-v1/train.tsv`，项目自编3条带重复数的符号序列 | MIT | 保留 LICENSE；独立清单校验 | 仅训练机制，不是自然语言语料或产品模型 |
| 搜索机械评估案例 | `tests/search-mechanics-v1/cases.tsv`，项目自编13条、dev/test家族隔离 | MIT | 保留 LICENSE；独立清单校验 | `language_accuracy_valid=false`，不报告正式准确率 |
| 第三方词典、模型、字体、图标 | 尚未引入 | 无 | 引入前独立审查 | 未引入 |

新增依赖必须先登记准确版本、来源、SPDX 许可、署名要求、维护/安全审查结果与体积影响，再修改 Cargo.lock。`scripts/check_workspace.py` 将未登记外部依赖视为错误；零外部依赖时不下载审计工具或安全数据库。出现外部依赖后必须同时加入漏洞数据库审计，不能继续声称零依赖检查等同漏洞审计。

资源登记模板：名称、类别（代码/词典/语料/模型/字体/素材）、准确版本、上游 URL、取得日期、SHA-256、SPDX/授权文件、署名、修改内容、可分发范围、审查人及审查结果。不得把第三方数据改标为项目自编数据。
