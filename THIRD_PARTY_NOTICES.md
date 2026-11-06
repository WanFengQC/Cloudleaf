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

## Round 14 独立输入试验分支

按精确CL-007提交复用已有15个Windows绑定依赖与31份许可原件；新增ring0.17.14（default-features=false）仅用于启动时词典SHA和短候选路径ID的digest，闭包19包42份许可，固定于`config/audits/input-snapshot.json`及`licenses/input-snapshot/manifest.json`。ring根许可引用的once_cell两份与fiat许可均按实际嵌套路径保留。总34包，完整原件和严格RustSec须绑定本轮锁，不引入翻译、网络或模型后端。

CL-009固定CC-CEDICT包保留CC-BY-SA4、来源、署名与修改说明，仅使用已审词头/读音/原始gloss范围；全123544条频率unknown。大数据留Git外，本轮不重新下载或分发新数据，不改Wiktionary research_only或现代词频准入。新的Rust Trie只是索引重建，不获得额外数据权利。

## Round 15 显式本地词汇研究

只读取CL-009既有固定Google Books Chinese2019一元分区，© 2019 Google，统计编纂声明见[官方索引](https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/chi_sim-1-ngrams_exports.html)，采用[CC BY 3.0](https://creativecommons.org/licenses/by/3.0/)。本轮派生仅严格匹配固定词典正文、合计2000–2019原field2并投影至stable entry ID；保留原件SHA、许可原文、署名与修改说明。派生TSV留Git外，不入CI包，源码和报告不包含原书文本。

这仍是`research_only`、来源登记仍`candidate-not-approved`：field2解释inferred、中文分词版本/截断和现代代表性未验收，不能作为生产token frequency、DF、Zipf或词语转移；缺失不补零。Google的统计编纂许可不解除CC-CEDICT词典的CC-BY-SA4范围和义务，也不证明书籍权利链或独立语言质量。本轮未新增Rust外部依赖。
