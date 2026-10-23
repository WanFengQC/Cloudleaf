# 第三方依赖与资源登记

M1/M2 使用 Rust 1.90.0 标准库及 workspace 内部 crate，没有 crates.io、Git 或其他第三方 Rust 库依赖。工具链不是产品代码的来源，遵循 Rust 工具链自身的 MIT/Apache-2.0 等许可证。Python 3.11+ 仅运行开发检查与报告脚本，使用标准库，不参与输入路径。

| 资源 | 版本/来源 | 许可 | 分发要求 | 状态 |
|---|---|---|---|---|
| 项目 Rust 源码与手写拼音规则 | Cloudleaf 0.1.0，项目独立编写 | MIT | 保留 LICENSE | 已登记 |
| 原型词典 | `data/dictionaries/prototype.tsv`，项目手写示例 | MIT | 保留 LICENSE；见数据清单的 SHA-256 | 仅用于原型 |
| 输入固定语料 | `tests/fixtures/input-v1.tsv`，项目手写用例 | MIT | 保留 LICENSE；见数据清单的 SHA-256 | 仅用于回归 |
| 独立算法种子评估 | `tests/evaluation/algorithm-v1.tsv`，项目手写、未用于训练或调参 | MIT | 保留 LICENSE；见独立清单与划分规则 | 用于指标验证及基线消融，不是正式代表性语料 |
| Unicode Unihan字符数据 | 17.0.0，[官方固定原件](https://www.unicode.org/Public/17.0.0/ucd/Unihan.zip)，2026-10-22审查后取得 | Unicode-3.0，原文见`data/licenses/Unicode-3.0.txt` | 原数据©1991-2025 Unicode, Inc.；随附V3许可©1991-2026，派生包保留完整copyright/permission与NOTICE | 原件8,518,517 bytes仅.local；字符kMandarin/kDefinition派生资源可复现；不当作完整词/短语/词频 |
| CC-CEDICT | 官方观察版本2026-10-17；[来源](https://www.mdbg.net/chinese/dictionary?page=cedict) | CC-BY-SA-4.0 | 署名、许可链接、修改说明、衍生数据同许可 | 网站禁止脚本访问，未下载/分发；仅本地导入器测试 |
| SUBTLEX-CH / wordfreq中文 | 见`data/sources/source-review.jsonl`精确证据及限制 | 未通过完整来源/分发范围审查 | 未批准前不纳入正式构建 | 阻塞，未下载/构建 |
| 第三方模型、字体、图标 | 尚未引入 | 无 | 引入前独立审查 | 未引入 |

新增依赖必须先登记准确版本、来源、SPDX 许可、署名要求、维护/安全审查结果与体积影响，再修改 Cargo.lock。`scripts/check_workspace.py` 将未登记外部依赖视为错误；零外部依赖时不下载审计工具或安全数据库。出现外部依赖后必须同时加入漏洞数据库审计，不能继续声称零依赖检查等同漏洞审计。

资源登记模板：名称、类别（代码/词典/语料/模型/字体/素材）、准确版本、上游 URL、取得日期、SHA-256、SPDX/授权文件、署名、修改内容、可分发范围、审查人及审查结果。不得把第三方数据改标为项目自编数据。

Unihan原件SHA-256为`f7a48b2b545acfaa77b2d607ae28747404ce02baefee16396c5d2d7a8ef34b5e`，许可原文SHA-256为`e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96`。来源锁位于`data/sources/locks/unihan-17.0.0.json`。修改仅为目标属性提取、原始证据保存、运行时去调规范化与索引；不推测词频/词性。Unicode许可仍为Unicode-3.0，不受项目MIT重新授权。项目编写的来源审查事实清单及导入工具采用MIT，各自许可证明确分开。详见[CL-009设计](docs/cl009-design.md)与[管线操作](tools/data_pipeline/README.md)。

本轮新增Kaikki发布者自身的zhwiktionary raw固定前8MiB真实文本摘录（dump2026-10-01/extracted2026-10-20），范围及SHA见`data/sources/locks/wiktionary-zh-20261001-prefix8388608.json`。完整gzip提前断流，未标全件完成或CRC通过。中文Wiktionary contributors按逐词条`attribution_url`链接署名，Kaikki/Wiktextract提取链保留；文本按CC-BY-SA4修改/分发并保留许可及修改说明，完整原文见`data/licenses/CC-BY-SA-4.0.txt`（SHA `28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5`）。媒体/引用实例不输出，可得额外权利标记隔离，逐页导入署名与fair-use例外未全部人工核清，不宣称绝无例外；仅供资源和一致性研究，非正式词库/语言质量验收。项目工具MIT不重新授权该文本。Google Books及Leipzig词频仅登记候选/阻塞，未下载或当作正式词频。
