# Round 15：中文评分与输入性能

2026-11-06，整体 **partial**。继续在`codex/input-round14`，基线`75e0acee88d21647010a56a9d876c136c95e6d5e`；完整受测实现为 **`994704c8ce9b5780b8cc53a32ca5c1f731094b30`**。真实词汇研究改善了指定天气/目标长句的首选，仍有“我爱昵”“这时一个……”等错误，没有正式语言质量放行。默认输入引擎保留，五个原PR仍Draft，main仍`a564fd47b0f054ff360592af3ccb8d744021160f`；不建PR6、不操作TSF、不执行长测、不进入CL-010。

最终完整HEAD、其自身Windows/Linux **push** CI、原始日志与artifact SHA另存Git外`.local/round15-delivery-receipt.json`，避免报告自引用提交。旧CI和本实现提交不能代替最终报告HEAD的CI。源码、工具、数据、二进制身份分别记录在[证据索引](input-round15-evidence/index.json)。

## 实际候选与根因

同一123544条固定词库、Top10、PositionDag、原结构代价和资源预算，以下为真实执行；[完整Top10前后表](input-round15-evidence/candidate-comparison.md)和[全部路径/评分/逐轮原文](input-round15-evidence/round15-paired-traces/summary.json)保留。

| 输入 | R14原结构首选 | R15显式研究词汇首选 | 结论 |
|---|---|---|---|
| nihao | 你好 | 你好 | 保持 |
| zhongguo | 中国 | 中国 | 保持 |
| woaini | 握爱昵 | 我爱昵 | “我”改善，整句仍错 |
| jintiantianqihenhao | 今天天气痕諕 | 今天天气很好 | 指定研发例改善 |
| wozhengzaikaifayikuanzhongwenshurufa | 握正在炌法医宽中文输入法 | 我正在开发一款中文输入法 | 原目标不在Top10，现在首选 |
| zheshiyigebijiaofuzadewenti | 见原始Top10 | 这时一个比较复杂的问题 | 仍有消歧错误 |
| xian | 秈 | 现 | 无上下文的歧义，不定义为准确率成功 |
| xi'an | 锡安 | 西安 | 显式分段下改善 |
| qz | 无候选，原文回退 | 无候选，原文回退 | 不丢原文 |

独立只读格诊断发现，目标正文原来就有567条完整路径，最佳Structural分-700，1482条格边；旧PositionDag未超预算但Top50仍不含该正文。因此是**评分下的候选截取**，并非词条格漏词；只重排旧Top5无法恢复它。`woaini`、天气正文也都有覆盖，但未进旧Top50。[原始格诊断](input-round15-evidence/recall-diagnosis/output.tsv)没有用于训练。

原评分每词-100、末音节Prefix另-300；123544条频率全None，Weighted与Structural同分，同音平局最后按稳定哈希ID顺序。“我爱昵”只需“我/爱昵”两边，结构-200，预期“我/爱/你”三边为-300；本轮没有凭例句重设代价或删词，所以它仍失败。它也说明仅改善同分消歧不足以解决过度长词偏好和上下文。

另补18条原创研发轨迹、179事件，包含银行/重庆、多音字、短词、长词、缺字/未知后缀、中间删除、撇号、标点和取消；原实现与新Structural的候选/提交/工作统计仍一致，全部原文和新错误保留在[额外轨迹](input-round15-evidence/traces/round15-extra-lexical/stdout.tsv)。这些样例不是独立gold，不计算正式Top1/Top5/MRR。

## 实际评分与模型能力

复用已有`BigramModel`和`ContextRerank`，没有另建LM框架。新增`Ranker::Lexical`的比较顺序为：

1. 原Structural score降序，保持词数/前缀代价。
2. 独立词汇观测的`sum(log2_fixed(count+1))`降序，1024定点单位。
3. 原stable entry ID序列平局。

词汇项**不加进score、不当归一化概率**，不随意设置最终权重；缺失项保持无记录、无词频值，其评分项关闭，另计`lexical_missing`。这能在共同后缀下保持二元加法序，所以PositionDag仍可精确压缩position；Bigram/Context仍必须保留last-entry，原PositionDag拒绝历史评分测试保留。

候选实际诊断包含words/prefixes、frequency_units/missing、lexical_units/observed/missing、bigram_units、context_units、词典SHA、模型SHA、原文区间和来源。真实目标首选为7词、score=-700、lexical_units=126584，6边有观测、1边缺失；所有frequency仍None。结构信息、正文一元观测、转移、前文和个人行为没有混用。

版本化UTF-8 TSV reader/writer固定八项元数据、词典SHA、来源SHA、来源/许可/分词协议、`train`划分、`mechanical/research_only`及模型种类；拒绝production标记、重复/无效ID、错误计数、错误版本、heldout和超限。元数据只读。Bigram复用原add-one/BOS/EOS/UNK/1024整数log2，补离线确定性产物及BOS/EOS可达性；输入加载时完整SHA校验，运行时Arc只读。声明和哈希不证明权利或真人审核。

`train_language`实际完成原创机械序列训练→固定产物→真实输入reader/query，源SHA错误与覆盖输出均拒绝，原产物保留。[训练工具实测](input-round15-evidence/round15-trainer/result.json)。**真实数据本轮只启用Lexical，Bigram和Context只完成接口、模型往返及机械行为测试**：没有真实转移语料，不从一元计数生成BOS/EOS或bigram。Context仍是有限池重排，不宣称全图上下文Top-K。个人学习/持久化未实现，未采集用户文本。

`with_language`和`reconfigure_search`显式启用，模型/词典版本不匹配会拒绝；换模型/上下文使revision增加，保留原文、拒绝旧候选。pending、failed或Unknown事务不能用换模型解除保护。普通`InputEngine::new`和`Host::new`未替换。

## 数据来源与可用范围

仍用原九文件CC-CEDICT包，123544条accepted、1468条隔离，原件/许可/清单/索引再次完整预检通过；没有修改解析规则、频率或原有来源批准。runtime SHA为`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`；包manifest SHA为`5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5`。[固定包凭据](input-round15-evidence/round15-lexicon-receipt.json)。

只分析本机既有Google Chinese2019一元分区，未下载新版本。gz SHA`6ab29862a8c904c261bb9d5a02a342285c4dbabf79cee5c4f8818076fd44fb86`，totalcounts SHA`f4a136b63419c0f34dfbfcf76f501ac80a745ad535f5cc9bda2666189a35e9f3`；完整EOF/CRC、508674词头/19074163三元组和解压SHA均核对。仅严格匹配词典正文、合计2000–2019原field2；不简繁转换、不拆词、不接受POS后缀别名，同正文读音共享正文信号而非读音频率。

Google[官方索引](https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/chi_sim-1-ngrams_exports.html)声明统计编纂CC BY3.0；结合[许可§3/4](https://creativecommons.org/licenses/by/3.0/legalcode)允许复制/改编及署名、许可、修改说明条件，本轮只作显式本地研究。冻结许可原文、Google署名和修改说明保留，同时保留CC-CEDICT的CC-BY-SA4范围。未取得原书权利链或生产统计协议验收，来源登记仍`candidate-not-approved`。许可txt一次HTTP403及原错误也保留，没有冒称新下载成功。

研究sidecar为3034818B，SHA **`36dc4d9057bc70e3494fe397da09e8ab6f187c435bd735b66a7734f9d414fce5`**：53642正文、56644词条ID有观测，66900 ID缺失不补零。干净双构建signal/LICENSE/NOTICE逐字节相同，当前工作区再构建也相同。[来源/构建报告](input-round15-evidence/language/report.json)、[双构建证据](input-round15-evidence/data-verification/verification.json)。完整派生TSV不进Git或CI包。

field2含义仍inferred，分词版本/截断/现代代表性未通过；totalcounts不用于归一化，token frequency、DF、Zipf均null。没有正式现代词频或真实转移语料，无独立gold/人工双审/heldout封存，正式语言指标继续null。Unihan及Wiktionary原范围不变。

## 性能与资源

旧热路径每次扩展先克隆完整Path，再被满桶丢弃。现在先计算实际评分和完整ID序，严格劣于桶末项才跳过物化；Reference不剪枝，扩展/预算/驻留统计原样保留，不以丢候选提速。旧优化前后1764组差分、最终Lexical 4320次对Reference比较均一致；四算法真实词库Structural/Lexical共288个plain/profile配对完整结果、路径和统计相同。[阶段原始数据](input-round15-evidence/phase-comparison/measurements/result.json)。

同12输入、Top10、16384边/200000扩展/50000活路径，Beam宽8；两种评分各自Reference有8/12、Exact有3/12预算耗尽，Position均0；Beam均有11/12剪枝。全部预算/失败留在分母，Reference不完备时不称绝对Top-K。目标长串Position查询中位Structural0.1858ms、Lexical2.6624ms；Lexical带观测时主要耗在Scoring，Traversal包含它和排序，不能相加。未来应先优化词汇查询/定点分计算和分段评分，不能直接放开PositionDag历史状态。

固定实现、Rust1.90 release、同171事件、每配置5个新进程交错执行，无丢弃预热，所有855事件保留；OS缓存未清除，背景负载未完全控制，50ms进程采样。InputEngine.handle计时含frame复制，排除stdout、平台和磁盘，不是端到端/硬实时验收。

| 配置 | N | P50 ms | P95 ms | P99 ms | max ms |
|---|---:|---:|---:|---:|---:|
| 原75e0ace结构 | 855 | 0.9823 | 5.7926 | 8.1067 | 11.3637 |
| 新结构完整重算 | 855 | 0.1087 | 0.3542 | 0.5531 | 0.9289 |
| 新研究词汇完整重算 | 855 | 0.7829 | 4.0631 | 5.7808 | 13.7093 |

新结构候选/ID/提交/工作统计与旧实现逐轮一致；研究模式本次P95达到5ms研发目标，但仍有长尾，不能泛化为正式词库或Windows响应已达标。初次试跑与编译重叠导致较慢，单独保留在trial目录、未合并进本表。另一组179事件只是补充诊断，不混入171事件对照。

整CLI进程working set采样峰：原188059648B、新结构187584512B、词汇192864256B；private峰分别197025792/200404992/215453696B，采样点92/67/90。包含runtime字符串、Trie、模型、引擎和临时路径，不是单算法内存；没有长期趋势证明。实际数据及硬件（Win11 build26300、i5-11400/12逻辑核、约32GiB RAM）见[完整测量](input-round15-evidence/round15-paired-traces/summary.json)及本轮原型基准环境记录。

词典和模型容器估计分别118497288B、7593827B；真实Windows Host共享一次模型、两引擎独立执行，固定费率126537581B，epoch文本1966080B/固定660352B。旧128MiB固定/64MiB文本门槛未增加；这是保全/持久结构计费与树布局估计，**不是进程RSS硬限额**，也不覆盖所有搜索瞬时内存。

## 增量、取消与Windows边界

**增量计算未实现、无旧Top-K缓存**。实测建格只占约1–3%，主要耗时在路径扩展/评分；为此增加自引用格缓存和Host生命周期成本缺乏收益依据。本轮仍全量重算，用完整参考差分验证优化，不声称做了增量与全量差分。词典/模型/上下文切换都有版本失效回归。

新增合作式`Cancellation`和`build_cancellable/search_graph_cancellable`，在normalize、格、扩展、结果发布边界检查；Cancelled为独立错误、不返回partial，预算耗尽仍原Budget。五项真实边界测试含Scoring中途触发、四算法、空/等待和发布前取消，旧无token入口保持原行为。**尚未将其接入同步InputEngine/Host的外部取消、墙钟deadline或异步revision调度**；没有每键线程/任务，不把预算耗尽、Escape或失焦伪称搜索取消完成。

真实原生Host/COM组件测试验证模型改变首选、双帧一致、Applied一次、Unknown隔离/不重放和旧epoch回调两次无污染，全部旧候选/分页/空白命中/生命周期测试保留。[原生专项](input-round15-evidence/runs/language-native-test/stdout.txt)。另外实际`--review-input-host RUNTIME SHA MODEL SHA`用全真实词库读到正确目标首选且两引擎一致；该入口没有系统context、原生提交或真实应用操作。[输出及费率](input-round15-evidence/runs/windows-language-real/stdout.txt)。本轮未注册/激活/安装/卸载TSF，未进行8h/24h或真实应用验收；Round13翻译成果不改。

## 修改、验证和失败证据

[设计](../input-round15-design.md)`6c01819`先行，语言API红测`7b4f60f`、模型反例红测`ed63232`、输入模型API红测`3addc3e`先于受测实现。实际生产修改范围：

| 文件 | 实际变化 |
|---|---|
| search/model.rs、lib.rs | 复用Bigram，固定模型reader/writer/身份/预算与独立词汇比较 |
| search/search.rs、lattice.rs、cancellation.rs | 延迟路径物化、评分诊断、合作式取消，四算法保留 |
| input/sentence.rs、lib.rs | 预加载Arc模型、模型/上下文切换和revision/事务保护 |
| CLI、input_trace、train_language | 只在显式研发入口加载/输出自有文字，离线训练不进入输入路径 |
| Windows session/input_review及测试 | 同模型双引擎、共享资源计费、只读真实词典检查及原生事务回归 |
| extract_ngram_signal.py、测试 | 只读固定源投影，来源/CRC/字段/缺失检查，不写生产数据 |
| README/NOTICES、CI、Git属性 | 明确实验范围和许可，构建研究工具，归档原始字节 |

完整[文件清单](input-round15-evidence/changed-files.tsv)截至实现SHA，后续只有本轮报告/证据。无新Rust外部依赖，Cargo.lock SHA仍`d92ffb61650b7b476c7be6b8869b61db1e31c0d540e5f9fd2efad176b9bbccd2`。完整13命令逐项返回0：fmt、Clippy-Dwarnings、locked/offline全目标Rust **209通过/2 ignored**、doc实际0例、release、工作区/许可、Python **63通过**、严格RustSec、旧基准/消融和研究工具构建。[命令/返回码](input-round15-evidence/round15-full-results.json)。34个外部包/73份Rust许可、冻结数据许可均核验，RustSec1409项数据库下0漏洞/0warnings；不沿用旧CI结论。[审计原文](input-round15-evidence/round15-final-dependency-audit.json)。

原始失败及修复均保留：缺失模型/输入API编译101；新增模型reader曾接受训练头夹带计数及BOS/EOS外独立计数环，真实2失败后修复；公有metadata也改只读。旧baseline首次归档遗漏include_str用原型数据，构建失败后从同75e0ace补原始数据，未改旧实现。PowerShell引号/PATH、补丁准备及证据脚本WinError206发生在工具准备；后者改为比较变更路径与原文件集合，避免超长命令行，后续实际命令独立记录，不算通过或算法失败。正式候选仍错的样例、编译重叠试跑、预算/剪枝及max长尾未删除。全部R14及更早历史报告/证据保留。

## 剩余退出条件与下一步

| 状态 | 实际结论 |
|---|---|
| done | 固定模型工程、可解释研究词汇/Bigram接口、实际模型切换、候选事务兼容、已证实的热路径优化及适用差分/本地回归 |
| partial | 指定同音例/长句改善、P95研发样本达目标；自然中文质量和真实端到端仍不足 |
| blocked | 合格现代统计/真实转移语料、独立gold与正式语言评估、生产统计/模型批准；A既有长期/真实Windows环境与可靠性门禁 |
| not implemented / not run | 增量、真实转移模型训练/采用、同步宿主取消/deadline、用户学习、纠错/模糊/简拼/双拼、DPI多屏完整性、长测/真实应用/安装发布 |

有限审查和差分未发现另一项已复现新增P0/P1；不据此宣称全产品无缺陷。自然整句/分段消歧仍为P1质量放行阻塞，“我爱昵”和“这时一个……”不能留给正式用户。下一步优先取得可复现授权转移语料与独立开发/gold，比较具有统一数值尺度的分段和转移评分，并预登记消融；随后优化实测词汇评分热点/双引擎开销，再推进增量与纠错。不得按本轮例句或heldout调参。A/B/C退出条件未满足，**CL-010继续关闭**。
