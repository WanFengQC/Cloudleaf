# Round 16：中文上下文评分与精确搜索交付

2026-11-06。**整体partial，正式语言质量和5ms延迟目标均不放行，CL-010关闭。** 已实际实现、训练并接入两种显式字符上下文研究模式；四条历史目标均成为首选，但新来源开发样本存在退化，不能宣称泛化质量已经提高。

## 版本与范围

- 工作区：`C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf`；分支`codex/input-round14`。
- Round15基线：`5045e18981dadefd3881a05fd021b31712ac243e`。
- 最终实现及全套本地检查：`f2b5c5ee53882787c84b29180da6e835ffa98a27`；后续交付提交只归档证据/报告与原始字节属性，不修改产品代码。
- 包含本文的最终完整HEAD、远程同名分支、精确push CI及产物下载身份写入Git外`.local/round16-delivery-receipt.json`；不能在提交之前伪造本文的自引用SHA。最终交付以该回执和实际Git对象核对，不能把中间CI转绑。
- main本地/远程均为`a564fd47b0f054ff360592af3ccb8d744021160f`。PR1–5仍开放Draft，HEAD依次为`83a2666fa748b4ae9cba41a5880ba5353e71c9a3`、`59dabdc2f8a3dc87cbd7f19ed89401ee607f7954`、`3f26f34e00c0775124f87e150a021f99fb52c567`、`e0ab78c2786d652a3a19cb544b4dc83996e63264`、`cca77457ca3943ee5ad90d566a8d9ef6b8795cd2`。只普通push当前输入分支，不建PR6、不改变现有PR。
- 不开发翻译、不注册/激活/安装/卸载TSF、不启动8h/24h或真实应用验收。默认输入引擎与旧Structural/Lexical评分保留。

设计先行见[本轮设计](../input-round16-design.md)。先提交`162fa10`设计、`fbab290`缺失API红测、`4f6485b`缓存错配红测、`9664a1c`固定工作预算红测，再实现`1398755`；许可抽取错误由普通后续提交`8a0838b`修复。开发样本暴露退化后，先`5c28a45`定义唯一额外平滑对照和红测，再`01d9a90`实现，`f2b5c5e`固定格式。没有改期望答案、删除合法词条或修改模型计数来修正目标句。

## 旧失败定位

旧实现的完整格/路径、原始Lexical计数、Top10和所有目标相关路径在`input-round16-evidence/diagnosis`。探针执行的六个旧生产源文件逐字节匹配5045e18。

| 正文 | 完整相关路径 | 最佳结构分 | 对应词汇分 | Round15 Lexical位置 |
|---|---:|---:|---:|---|
| 我爱你 | 2 | -300 | 62514 | 24 |
| 我爱昵 | 3 | -200 | 23013 | 1 |
| 这是一个比较复杂的问题 | 672 | -700 | 167227 | Top50无 |
| 这时一个比较复杂的问题 | 1344 | -600 | 132075 | 1 |

`woaini`格234边，旧搜索8657扩展；复杂句格1766边，旧搜索65379扩展。两例旧Structural/Lexical均budget=None、beam_pruned=0、guaranteed=true。目标词条与完整路径存在，是每词-100优先导致精确Top-K排低/截除；不是缺词、缺图、Beam误剪或候选ID映射错误。不能靠增加一元信号解决跨结构分比较。

## 实际生产代码变更

精确文件清单见`input-round16-evidence/changed-files.tsv`，源/工具/二进制SHA见`identities.json`及各运行result.json。

| 文件范围 | 目的 |
|---|---|
| `crates/cloudleaf-search/src/character.rs` | 数字字符ID、观测转移预计算、未知回退、词条内部字符分缓存及两种平滑 |
| `model.rs`、`lib.rs` | 复用BigramModel/LanguageModelSnapshot，兼容扩展两个字符kind及显式Ranker::CharacterBigram |
| `search.rs` | 完整字符分解；Exact的充分状态/后缀排序界及有界完整路径枚举；Reference/Beam独立保留 |
| `lexicon.rs` | 不可变词典实例标识，防止预计算缓存套用到另一词典 |
| `cloudleaf-input/src/sentence.rs` | 新模型种类/词典实例入口配对；沿用模型SHA候选身份和revision/提交保护 |
| `train_language.rs`、`input_trace.rs`、CLI、Windows只读review | 显式加载/离线训练/模式分派，不增加按键IO或改变默认构造 |
| `character_model.rs`、`language_integration.rs`、Windows session/render测试 | 分段等价、EOS/UNK/历史状态、缓存错配、预算及实际Host提交隔离回归 |
| `scripts/prepare_context_corpus.py`、`tests/test_context_corpus.py` | 固定官方原件身份、来源关系/近重复/历史目标排除/内部划分，拒绝变更原件 |
| README、设计、通知、独立许可、CI | 使用说明/权利范围，Windows研究工具包更新为input-round16-windows |

没有增加Rust依赖或改变Cargo.lock。Windows生产提交/恢复/watchdog/COM线程模型不改；新增Host测试复用既有真实COM组件故障注入。

## 评分定义与搜索正确性

字符单位为未经简繁转换的Unicode scalar；正文`c1…cn`包含一次BOS和一次EOS。主研究模式`character_bigram`使用`P(y|x)=(Cxy+1)/(Cx+|V|)`，V含训练字符、EOS、UNK。完整分为每转移`log2_fixed(分子)-log2_fixed(分母)`之和，精度1024，完全不叠加每词-100、Prefix-300、Google一元和或人工词频。相同正文无论按几条词边切分，都有相同字符分；实际不同字数正文仍可能受生成概率长度偏好影响，不能声称解决一切长度偏差。

额外对照`character_bigram_witten_bell`只用同一真实语料：`T(x)`为已观测不同后继数，`P0(y)=(incoming(y)+1)/(所有转移数+|V|)`，`P(y|x)=(Cxy+T(x)*P0(y))/(Cx+T(x))`；无观测历史退回P0。没有调参或混入不同来源/单位的分数。归一化测试使用真正实现，定点舍入容差0.003。其开发表现更差，**没有推广为默认研究选择或生产算法**。

模型格式仍`cloudleaf-language-v1`，只增加明确kind；旧lexical/bigram字节语义不变。保留16MiB、200000行、100000句、1000000token、1100000总转移限制及BOS/EOS流量、连通性、引用、usage、train和SHA校验。模型只接受mechanical/research_only，词典frequency仍None。数值上界由上述真实入口预算约束，乘法和累计均在u64/i64范围内。

载入时编译数字ID和按前字符分区的稀疏有序转移表，预计算词条内部字符分；热路径无临时String token、log计算、网络、磁盘或Python。未来评分充分状态是`(拼音位置, 最后映射字符)`，不能仅按position合并。PositionDag明确拒绝字符模式，原无历史算法保留；Beam保留近似剪枝语义。

字符Exact使用反向DAG求每充分状态的最优后缀`(分数, 最小稳定ID序列)`，再用该完整排序上界优先枚举。当前第K条不同正文不可能被剩余上界超过时才结束。相同位置/正文保留最小ID代表，允许更优代表重开，跳过过期堆项。它不是只重排旧Top5；目标可直接从完整格竞争。后缀每状态/边评分、前向扩展均消耗原expansion预算，后缀标签、待处理路径及正文去重记录计入resident预算，没有提高限制。这里是工作计数门槛，不是allocator字节硬上界。

独立最终差分`differential/runs/final-f2b5c5e-both`：两种字符kind**各11160次完整Reference、22320次完整Candidate对象对照**，总22320/44640；均0失败、0Reference超预算、0Beam剪枝，Position拒绝各11160。覆盖3词典变体、12计数方案、62输入、K=1/2/5/10/20、前缀/撇号、重复正文不同分词、UNK、Unicode、EOS逆转。比较包括正文、分数、ID、原edge和全部Features。源码前后及精确Git blob SHA全部一致；Reference预算失败时仍不能叫最优，Beam剪枝后不能叫精确。

## 真实上下文数据与模型

来源为[Tatoeba官方周导出](https://tatoeba.org/en/downloads)的Mandarin detailed/base/tags、cmn-cmn及cmn-eng links，Last-Modified 2026-10-31；URL会更新，本轮以固定原件哈希锁版本。详细原件1727096B，SHA`374dc649bd61ed5597c58ad9f098edb831e7bbe4ea0831f82947bec1fe5c8bb8`，解压8648703B，SHA`43c4e88355e5897aa20a1b1e73852a1edb70b6cd51a426be4dda6f08ca545005`。五个包完整读取至EOF，bzip2 CRC和UTF-8通过。共89248源句/973 current owner，详细6字段均合法；owner不保证原作者。创建时间有未知哨兵，语料含简繁、众包翻译与学习例句，不是现代IME自然分布。

[官方条款](https://tatoeba.org/en/terms_of_use)与[CC BY 2.0 FR法律原文](https://creativecommons.org/licenses/by/2.0/fr/legalcode)支持在履行署名、许可链接和派生说明下复制/改编/使用；本轮仅本地研究。保留句ID、owner、句URL、时间、原件与派生链；官方规则和稀疏标签不证明每句原文权利及内容质量。生产派生模型分发、完整原作者署名处理及正式质量仍不批准。未下载音频；未把UD注释许可推定为其原文许可。完整来源审查、原始HTTP/许可证据见`corpus`。

处理器只读固定SHA原件，不简繁转换，提取2–128字Han连续片段；非Han截断，所以BOS/EOS代表保留片段边界而非必然完整语义句。隔离699原句：问题/版权等标签654、历史目标简繁子串43、同族1、隐私形状4，原因可重叠。85条cmn-eng悬空边另隔离，没有伪造缺失原文。原句/base/共享英文根、全文规范化及≥8字三字符shingle Jaccard≥0.8归族，83362族、最大19句。全量近重复参考1064644候选对，176边与优化完全一致。此标准不证明所有片段近重复、语义改写或全部跨语言翻译族已隔离。

在看模型效果之前冻结家族SHA的80/10/10划分，再按heldout>dev>train排除相同片段跨分区重复。train86737片段/716681字、dev11067/90722、heldout11146/90869；源原文黑名单与训练交集0、片段全文SHA跨分区交集0。全部属于tatoeba-community同一全局来源家族，只能称内部engineering划分，不是独立gold。保留集未用于模型选择或指标计算。两次实际干净输出的train/dev/heldout及lineage文件内容哈希一致；summary的执行时间不声称逐字节重现。

Rust既有离线训练器实际训练并通过InputEngine/Host预加载：

| 模型 | 真实训练材料 | 产物SHA-256 |
|---|---|---|
| 字符add-one | train86737片段，5082字符，143560转移键/803418次含边界观测 | `5f28834165fcf83b22458e701b3608b403cb9bf7a9be8ba3c686fd158c53d6b9` |
| 字符Witten–Bell | 完全相同train和转移计数，只改已定义平滑 | `516d78a23786470c5ba7c1c7ee45e82c80eaaccd7eb1e3b541faf55d8f93b44b` |
| 词级Bigram | 相同train中48705完整词典覆盖片段/260373词token，113003键/309078次含边界观测 | `19c08b9bee4024a85567b3d5fcd8252493887b748a6da035a291bd79ea782080` |
| 同覆盖字符add-one | 与词级完全相同48705片段/387858字 | `ca34af3b19eeb240090bf73a0c33e7d6738b9d30e94adbc3f0d79587f7b309f6` |

全字符训练正文SHA`1878767d1f0076011cac0be2c67afdfa0256a961030a80d91503a145d007590e`；词序列SHA`c93904a79833e4393389fd0405e1e04fb97d561f2e675ce3d028838ad93b3a11`；同覆盖字符SHA`391127315528a635b7f1d67d7178882e1618c8109784744a281ce2a11dde1fcd`。字符主模型重建与首建完全相同。词级用确定性全局最少词数/stable-ID平局分词，38032未覆盖片段逐条保留，经独立可达性重查误报0；它不是人工分词金标。两单位的分数不能直接比值，也不能忽略覆盖差异，所以另有同覆盖字符对照。

固定词典仍123544条/1468隔离，runtime SHA`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`，manifest SHA`5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5`；本轮来源/哈希verifier实际成功，已有CC-CEDICT/Unihan许可及schema2不变。Google一元研究信号只作旧Lexical对照，不用来捏造转移、token frequency、DF或Zipf。所有正式频率仍unknown/null，完整语料/模型不进入Git或CI发布包。

## 实际候选与退化

全Top10、路径分段、分数/Features/entry ID和模型SHA见`summary/candidate-comparison.md`、完整配对summary与原始stdout。两字符模型都得到下列首选，未把目标答案加入训练：

| 拼音 | Round15 Lexical | 字符研究模式 |
|---|---|---|
| woaini | 我爱昵 | 我爱你 |
| zheshiyigebijiaofuzadewenti | 这时一个比较复杂的问题 | 这是一个比较复杂的问题 |
| jintiantianqihenhao | 今天天气很好 | 今天天气很好 |
| wozhengzaikaifayikuanzhongwenshurufa | 我正在开发一款中文输入法 | 我正在开发一款中文输入法 |

新来源dev预先按SHA挑32不同句，2–24字，不因结果换样本：13 ready、19无法完整映射、0超64输入；拼音由待测词典读音自动导出，意图未经真人确认，所以不能报告正式准确率。13例共281事件，同输入/词典/配置/预算：

| 模式 | 源正文在首选的观察数 | 源正文进入Top10 | 最终帧预算失败 |
|---|---:|---:|---:|
| 旧Lexical | 7/13 | 8/13 | 0/13 |
| 全覆盖字符add-one | 6/13 | 11/13 | 0/13 |
| 同覆盖字符add-one | 6/13 | 11/13 | 0/13 |
| 字符Witten–Bell | 5/13 | 11/13 | 0/13 |
| 词级Bigram | 3/13 | 3/13 | 9/13 |

总选择分母始终32，19未映射不能消失。add-one相对Lexical改善3例但退化4例：`老婆婆被烧死→老婆婆被烧思`、`这条规定不许例外→这条规定不需例外`、`他从乔治那里拿走了报纸→他从乔治哪里拿走了报纸`、`我们战争胜利了→我们战争生里了`；`船进港了`等仍错。Witten–Bell又使`所谓陌生人`退为第二。**无法据此宣称新排序整体优于旧模式，产品质量目标未达成。** 完整原句来源/owner/URL、全部候选和未映射原因在corpus/word-contrast、summary/development-comparison及traces。

R14全部171事件和R15额外18例179事件保留复跑；后者覆盖多音、长词、错误拼音、中间编辑、原文回退等，但数字日期、现代技术、专业低频、超长组合仍不是均衡独立质量集，不能用这组有限来源自动代替用户要求的完整覆盖。纠错未实现，错拼仍保全原文；Prefix可能过度补全，`xian`字符首选“想”而非旧“现”，歧义不作gold。

## 性能、阶段观测与资源

最终实现6配置各5个全新进程，轮换运行次序，同171事件、Top10、图/扩展/驻留预算。每配置855事件，30进程所有事件和慢例都保留，无丢弃warmup。Windows11 build26300、i5-11400/12逻辑核、约32GiB、Rust1.90 release；测量窗口无已知并发编译，OS缓存不清、系统背景负载未完全控制。计时为InputEngine.handle含帧克隆，不含stdout、文件/模型载入、Host双引擎或TSF/UI。

| 模式 | P50 ms | P95 ms | P99 ms | max ms |
|---|---:|---:|---:|---:|
| Round15原Structural重编译 | 0.1014 | 0.3094 | 0.4030 | 1.0252 |
| Round15原Lexical重编译 | 0.7080 | 3.7976 | 5.4221 | 6.7705 |
| 当前Structural | 0.0998 | 0.3105 | 0.4236 | 0.9760 |
| 当前Lexical | 0.6883 | 3.6433 | 4.9025 | 6.1289 |
| 字符add-one | 0.7410 | **6.3411** | 14.3598 | 18.7251 |
| 字符Witten–Bell | 0.8075 | **6.1164** | 14.1054 | 19.3274 |

**5ms P95目标未通过。** 不用更快的单次试跑或删慢样例替换以上结果。首轮字符前向DP P95 20.5748ms、复杂句200000扩展失败；第一次仅score后缀界导致resident50000及P95 87.1347ms，正文去重后17.2569ms、同分ID上界后7.6656ms；这些均有原文。中间8a版本配对P95 7.33ms/max39.6565ms也保留，不转称最终成绩。

另在最终源码执行16策略×12输入×3次，共576组plain/profile配对，完整图/候选/Features/预算统计全部一致，计时嵌套校验通过。Reference四评分各8/12超预算；旧Exact两评分各3/12超预算；字符Exact两种均0/12；Position拒绝两字符各12/12，不能记为搜索成功；Beam四评分各10/12有剪枝。相同Top10/Beam64/16384边/200000扩展/50000驻留，无人为放宽预算。

最终add-one纯查询中位：我爱你0.4871ms，天气0.9300ms，开发长句1.7611ms，复杂句7.2511ms；复杂句54843扩展/9288驻留高水位。Witten复杂句10.8721ms、56451扩展/12105高水位。Scoring和桶去重嵌在Traversal内，不能相加当总时长；剩余成本主要仍在路径处理、同分与候选构造，不是建格缓存就能解决。详见final-phase压缩原始JSON与result。

每进程50ms采样，以下是整个CLI（词典、Trie、载入、模型和临时搜索）的观测，不是单算法内存或硬上界：

| 模式 | 5进程采样数 | working set最大B | private最大B | 每进程CPU秒范围 |
|---|---:|---:|---:|---|
| 当前Structural | 60 | 187613184 | 215339008 | 0.53125–0.578125 |
| 当前Lexical | 81 | 192069632 | 215441408 | 0.75–0.828125 |
| 字符add-one | 118 | 221413376 | 223023104 | 1.125–1.171875 |
| 字符Witten–Bell | 120 | 220684288 | 223059968 | 1.140625–1.21875 |

真实模型Host只读review两kind均返回0、双引擎候选一致；add-one约7.50MB固定模型估计，Host固定计费约126.45MB，未提高128MiB门槛。字节布局估计不等于进程RSS或搜索峰值。Host执行/投影的逐键双引擎延迟尚未单独测量。真实dev单进程add-one P95 11.0031ms，Witten8.0451ms；旧词级P95 150.4357ms且9个最终预算失败，也明确不采用为生产方案。

## 可靠性与已修复缺陷

独立源码审阅实际复现一项新P0风险：字符词条缓存套到另一词典，同条数可错分，更多条目会panic。旧probe退出101，正文“甲”改“乙”仍误用旧高分且guaranteed=true。先保存失败测试，再加不可变Lexicon私有Arc实例标识；Clone共享、move稳定，新加载对象须重新载入模型。配置/搜索入口一次验证，不逐边重复防御。同probe修复后返回InvalidModel、退出0，原始红绿日志和源码SHA均在independent-review。没有以长度或不稳定地址替代身份。

InputEngine新mode模型类型/词典SHA/实例绑定、换模型保留preedit、revision增加/旧候选拒绝、pending/failed禁止切换、未知拼音Enter回退均通过。Windows实际COM组件测试覆盖字符模型Applied写一次、Unknown隔离不自动重放、重复Space不再写、旧epoch回调两次无污染；原R15测试仍在。该测试不是注册后的真实Windows应用验收。

取消仍是原合作式Lattice/search API，没有接入同步InputEngine/Host跨事件抢占、没有deadline、没有按键后台线程；工作预算失败不叫取消成功。没有增量缓存，仍完整重算。用户学习/词库、纠错/模糊音/简拼/双拼、真实TSF全链路、安装发布和macOS/Linux适配仍是产品路线，不删除或冒称完成。

## 完整本地回归、失败与身份

最终实现f2b5c5e上原13入口逐条返回0，原始command/returncode/stdout/stderr/source SHA在`runs/delivery-*`，完整清单在`round16-delivery-results.json`：diff check、fmt、Clippy -D warnings、locked/offline workspace all-target tests/doc/release、workspace/许可、Python、RustSec审计、原基准/消融、input/search release examples。**Rust220 passed、2个原有ignored保留，Python67 passed。** 不是将209个旧测试删除后换成新计数。

34个Rust包/现有许可证审核、Cargo.lock SHA`d92ffb61650b7b476c7be6b8869b61db1e31c0d540e5f9fd2efad176b9bbccd2`不变；实际RustSec库1409公告、commit`26fd2d43f7280a1fb925a3743ec463017afe7119`，本轮命令0，未忽略漏洞。固定词典独立verifier和原版权哈希检查0。953个旧报告/数据/测试/许可路径Git blob不变，209个既有Rust test函数全部保留；完整证据见summary/preservation。

新失败测试分别保存API未实现编译101、错配assert101、600扩展Top50预算assert101、Witten格式缺失101，再实现转绿；没有绕过断言。许可全文抽取带CR/网页脚本时diff check返回2，PowerShell仍继续提交是执行控制错误，已普通后续修正并完整重跑。Shell转义、临时cargo路径、抽取断言、缺锁文件、错误cwd和85悬空边的失败分开保留，不能记成算法通过；工具错误摘录标明不能替代真实原文。

原始证据按`index.json`逐字节SHA归档；大差分/阶段/原始许可网页使用gzip，保留解压SHA/CRC和Git外原文，不把压缩后哈希当原文哈希。完整语料、训练模型、二进制、个人输入或密钥不加入Git。CI Windows包只含工具、DLL、设计及许可，不含runtime、模型或用户配置。

中间实现8a0838b的[push CI42172540330](https://github.com/WanFengQC/Cloudleaf/actions/runs/42172540330)在后续查询已成功，但**不作为最终HEAD验收**。最终交付提交必须单独检查Windows/Linux push jobs、全部步骤、3产物及SHA，下载原始日志/ZIP验证CRC/digest，写入`.local/round16-ci/final`和最终交付回执；未取得这些证据前不得声称最终CI通过。

## 放行结论与后续依赖

| 状态 | 结论 |
|---|---|
| done | 分词无关字符概率评分、合法真实上下文训练、显式输入/Host接入、精确搜索/枚举差分、缓存错配修复、全部本地回归与完整失败保存 |
| partial | 四历史首选改善、Top10来源正文覆盖改善；新dev首选反而退化，add-one/Witten均未达到5ms研发P95 |
| blocked | 独立人工IME gold、完整现代代表性/生产数据和模型批准、真实Windows环境/恢复/授权及平台可靠性退出门禁 |
| not implemented / not run | 真实应用/TSF/8h24h、同步宿主取消/deadline、增量、拼写纠错/模糊/简拼/双拼、用户学习/词库、安装及跨平台系统适配 |

有限审查中已复现缓存panic/错分已关闭；不能据此宣称全项目没有P0。仍有P1质量门禁：四条新来源退化、同音/长程语义不足、未知/繁体覆盖缺口、前缀过补全、语言数据代表性/许可发布边界，以及新增模式的P95和长尾。没有挑选Witten算法作为“最佳”或靠人工给字加分来隐藏这些问题。

下一步应针对全部保留退化与路径热点形成新的预登记方案，用更多独立来源/本人真实标注验证语义和覆盖，考虑具有证据的更长上下文或词汇/字符联合机制；不继续用本轮已见句子盲调权重。当前默认模式保留，**正式Top1/Top5/MRR全部null，正式语言质量不放行，CL-010继续关闭。**
