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
# Round 16 本地上下文研究材料

Tatoeba Mandarin Chinese官方2026-10-31周导出采用CC BY 2.0 FR，来源与条款见 https://tatoeba.org/en/downloads 、https://tatoeba.org/en/terms_of_use 和 https://creativecommons.org/licenses/by/2.0/fr/ 。本轮原件、来源/owner/句URL及确定性派生模型保留Git外，仅research_only，不随本工具包分发或自动批准用于生产。owner不保证是原作者；正式派生内容发布前仍须核实完整署名与原文权利处理。许可原文保留于`licenses/input-context/CC-BY-2.0-FR.txt`。仅处理中文文本，不含音频，不将学习例句计数称为现代词频、人工gold或输入法准确率。

## Round18 小鹤双拼事实规则

小鹤纯双拼的键位事实按作者[规则说明](https://flypy.cc/help/up.md)、[键位图](https://flypy.cc/images/hejp.png)及[零声母图](https://flypy.cc/images/uplu.png)独立实现，固定映射标识`xiaohe-flypy-up-2026-09-27-v1`。署名小鹤，规则页面原字节SHA256为`67b3069d9773b36b7e2b8153997158d83ed832cd4ec37963aea6d48d12524419`。不复制第三方输入核心、形码或词库；官网图文原件留Git外供核验，不推定这些素材取得再分发授权，不加入产物。微软和自然码完整原始规则未核实，本轮不提供猜测实现。

## Round 11独立翻译分支

新增`cloudleaf-translation`使用reqwest0.13.3、rustls0.23.46（ring）、tokio1.54.1、serde1.0.228、serde_json1.0.149、ring0.17.14和zeroize1.8.2。准确的136个跨目标传递包、版本/SPDX/registry来源在`config/audits/translation.json`及Cargo.lock；不是输入核心的新依赖，原六crate的生产依赖闭包保持本地。只在显式翻译worker执行HTTP，默认不联网，无随包在线服务或密钥。

原件242份及SHA见`licenses/translation/manifest.json`，包含MIT/Apache/ISC/BSD/Unicode/Unlicense/CDLA等实际包声明，保留上游全部随包LICENSE/COPYING/NOTICE类原文，不将这些库重新许可为MIT。jni、jni-macros、jni-sys-macros和Android verifier的crate未附原文，已按Cargo自带VCS固定提交取得8份上游MIT/Apache原件，HTTP来源/时间/内容SHA另见`upstream-acquisition.json`，不冒充随包文件。这些Android目标依赖不代表已实现Android平台。

技术审查核对了实际元数据、原件字节和已知漏洞，不代替完整供应链或法律审查。首次缓存版本rustls0.23.40/tokio1.52.3被RustSec拒绝，失败保留；改用上述补丁版后审计通过，没有忽略advisory或warning。开发审计继续固定cargo-audit0.22.2，与既有Windows方向同工具流程。

本地字典研究投影单独保留原来源的LICENSE/NOTICE和状态；CC-CEDICT/Unihan的既有批准范围不因代码实现扩大。未引入离线权重、虚构语义数据或人工gold，API生成译文不具有独立语言质量认证。

## Round 12 原生面板

新增 app 仅在 Windows 直接使用既有锁定的 windows-sys0.61.2/windows-link0.2.1，增加窗口/控件、DPI、凭据、内存、文件替换与剪贴板所需 features。无新增外部包或版本升级，MSRV1.90不变；原件已在上述242份清单中。系统字体通过 Windows 接口使用，没有复制或分发字体文件。没有引入浏览器运行时、GUI框架、模型运行库或权重；模型候选研究不是许可准入。

## Round 13 离线推理

独立`cloudleaf-offline`增加Candle0.9.2（MIT OR Apache-2.0）、SentencePiece-rs0.2.2（Apache-2.0）和Rayon1.11.0（MIT OR Apache-2.0）；精确77项新增包/来源见`config/audits/offline-runtime.json`，旧136包版本不变。当前总213包、382份许可原件以manifest为准，r-efi原件AUTHORS包含完整MIT条款及版权，保持原名。Candle nn/transformers等未随crate附许可的版本按固定上游VCS补齐，见`offline-upstream-acquisition.json`。没有把第三方运行时重新许可为项目MIT。

九个源包的限定修补在`vendor/offline`，来源及逐文件SHA见`config/offline-runtime-source-patches.json`。ZIP升级解决真实审计失败，GEMM/Pulp仅替换已停止维护的宏依赖为pastey0.2.3（MIT OR Apache-2.0），另适配Candle三处ZIP文件名返回值；运算源码不变。原件及其许可证保留，源重建和严格审计不忽略告警。

OPUS两个权重仅在用户明确授权后下载到Git外用于本地研发；不随EXE/CI包分发。原件和转换身份分别锁定，tokenizer原件不改。具体上游模型声明CC-BY4；en→zh镜像另标Apache2，差异及训练数据完整权利链unknown均保留，不能推断生产分发批准。Safetensors转换使用隔离的Torch/NumPy等开发工具，不成为应用运行时或随包依赖；完整工具版本及实际原件许可另存研发证据。Generated表示模型生成，不表示正式语言质量验收。

## Round 21 隔离整合

发布物使用合并后的 Cargo 1.90 锁，输入与翻译包分别按实际依赖闭包核对许可，RustSec 检查整个锁。共有包由工作区精确政策统一登记，两个发布物的许可闭包独立核对。九份 vendor 补丁及上游锁保持固定。

首次整合审计报告 `url 2.5.8` 命中 [RUSTSEC-2026-0592](https://rustsec.org/advisories/RUSTSEC-2026-0592.html)。Cargo 精确升级 `url 2.5.9`，其依赖 `form_urlencoded` 随之从 1.2.2 升到 1.2.3。两包声明 MIT OR Apache-2.0，上游提交均为 `ad1c66cbff6e318393d872641e30589ff9687b9c`。现用归档校验和记录在 Cargo.lock，许可原件及字节哈希记录在翻译清单。更新前的许可副本予以保留。
