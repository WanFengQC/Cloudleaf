# Round18：统一拼音方案、简拼、小鹤双拼和有界容错

2026-11-12。结论 **partial，正式产品不放行，CL-010关闭**。已增加可运行的 Rust 输入能力并完成隔离 Windows 键盘操作；简拼与字符模型组合仍有实际预算耗尽，延迟与语言质量未达到正式要求。默认核心、全拼配置、词典、研究模型及翻译成果不变。

分支 `codex/input-round14`，Round17 基线 `794d32c1818c4e1dfd2e504ea4369abdb3621c2a`；本轮可运行实现提交 **`b948f986c4c9d00bec7cdb7845283c83e6a87970`**。最终报告提交的完整 HEAD、精确 push CI、下载产物和报告哈希由 Git 外 `.local/round18-delivery-receipt.json` 绑定；实现提交的 CI 不能替代最终提交的 CI。main 保持 `a564fd47b0f054ff360592af3ccb8d744021160f`，PR1–5 保持原分支和 Draft，不创建 PR6。

## 设计、源码与兼容边界

设计先行 `30b70bc`，API 缺失反例 `a9452c6`，来源平局设计 `7d8892f`，核心实现 `4a42b47`，可重放追踪/CI `52fb19c`，全音节负例 `b57f61b`，实际 F8 拦截观察后的本窗口按钮 `cd16d0d`，仅格式整理 `b948f98`。完整设计见 [input-round18-design.md](../input-round18-design.md)，原始 red/green、命令、返回码及完整日志见 [证据索引](input-round18-evidence/index.json)。

| 实际文件 | 修改目的 |
|---|---|
| `crates/cloudleaf-pinyin/src/scheme.rs`、`xiaohe.rs`、`lib.rs` | 统一方案配置、音节假设 DAG、来源和原始偏移、有界模糊/单编辑纠错、小鹤事实表；原 parse 保留。 |
| `crates/cloudleaf-search/src/schemes.rs`、`lattice.rs`、`search.rs`、`profiling.rs`、`lib.rs` | 音节图与现有拼音 Trie 求交，保留完整词条/多词搜索；解释成本、充分搜索状态、确定来源代表及显式阶段计时。 |
| `crates/cloudleaf-input/src/sentence.rs`、`lib.rs`、`examples/scheme_trace.rs` | 显式配置切换、revision/候选 ID、来源诊断、预算失败后保留原文；实际引擎/搜索追踪工具。 |
| `crates/cloudleaf-types/src/lib.rs`、`crates/cloudleaf-candidate/src/lib.rs` | 兼容扩展匹配类型；旧 Exact/Prefix 次序和默认成本为零。 |
| `adapters/windows/isolated_input.rs`、`input_review.rs` | 原隔离 Host 的方案设置、双引擎预检、K10 分页和 owned 纠错按钮；沿用未修改的session，不复制候选算法。 |
| 各 crate 新增方案测试、`cloudleaf-search/tests/search_contract.rs` | 实际接口负例、事务、规则、严格候选对照；旧测试仅补 Budget 穷尽分支，保留原断言。 |
| README、`THIRD_PARTY_NOTICES.md`、CI、`.gitattributes`、本报告/证据 | 使用方式、原件依据及限制、研发追踪产物、原始证据按字节保存。 |

逻辑链为 **原始按键 → 方案解释 → 音节假设 DAG → 现有词典 Trie 交集 → 完整词条格 → Reference/ExactDag/PositionDag/Beam → 原评分 → 输入帧 → 原提交事务**。没有另建缩写词表或复制词典。弧保存规范音节、匹配种类、规则、修改位置及代价；归一键位置和原始 UTF-8 字节范围分开，撇号边界不能被跨越。

默认 FullPinyin 且容错关闭直接走原格；旧候选身份、默认引擎、默认评分不改。新配置只能用于显式整句后端，legacy 核心拒绝方案配置。配置切换先预检两个实际引擎，保留预编辑/光标、增加 revision；非默认候选身份绑定方案/配置/规则、弧来源、词典和模型。未决、失败或 Unknown 提交拒绝切换；切回旧方案也不能复用旧 revision。

## 已实现范围及未实现部分

**简拼：** 任意多音节首字母与 zh/ch/sh 声母缩写、全拼/简拼混合、末尾音节前缀、不同分段、词语内缩写及多词句子都通过同一 DAG 和词典原音节边界。不是仅对 `zgr` 等例子硬编码。完整精确路径保留；资源有限，不保证任意长度的歧义句都能在预算内完成。实际字符模型超预算如下，组合功能仍 partial。

**双拼：** 实现已核实的小鹤，静态版本 `xiaohe-flypy-up-2026-09-27-v1` 是 Cloudleaf 的固定规则身份，不冒称作者发布版本。依据作者 [规则文本](https://flypy.cc/help/up.md)、[键位图](https://flypy.cc/images/hejp.png)、[零声母图](https://flypy.cc/images/uplu.png)，技术核验见 [布局审查](input-round18-evidence/layouts/layout-review.md)。409 个现有合法音节全部映射为两键，`lo` 同时对应 lo/luo 的路径均保留。单键等待、零声母、ü/v、大小写、分隔和非法组合有测试；不把非法双键退回两个全拼字符。官网图文原件保留 Git 外，只保存来源身份和独立实现的功能事实，不打包图文或别家输入法核心。

微软官方材料未闭合完整键位/零声母，自然码材料存在具体冲突和缺项，**两者未实现**。小鹤+纠错当前明确拒绝；物理键盘入口仍是 ASCII/v，API 对 ü 的处理不等于已验收实际 Unicode/代理对键入。

**模糊音：** zh/z、ch/c、sh/s、n/l、in/ing、en/eng、an/ang 分别可关闭、正向、反向或双向；默认全部关闭。保留原精确解释；只从完整精确音节弧扩展，尚不对未完成音节前缀做模糊猜测。每条最多两次变换，局部备选最多32，整体弧/工作量另有上限。规则顺序规范化，等价配置取得相同身份。

**纠错：** 显式关闭，支持一次相邻键替换、漏一个字母、多一个字母、相邻转置。默认最长32原始字节、最大代价1、最多128纠错候选；图整体64字节、4096弧、200000工作量。完整路径累计编辑成本也不得超过1，不允许零消耗弧。只在输入没有完整精确音节路径时启用，与词典是否收录无关；合法但未收录的拼音不能被判成错字。前后可达性过滤只删除无法组成完整单编辑路径的死弧，未提高预算。超过限制仍可原文回退，不将局部候选冒称完整结果。

排序将解释成本 `(edits, fuzzy, abbreviations, prefixes)` 与既有语言分数分开比较：成本向量升序，再沿用分数降序、lexical_units降序、词典条目索引序列升序，最后在新图比较完整 edge 序列，确定合法来源代表。正文用于去重，不是该路径比较器的平局键。没有把任意编辑距离权重塞入概率。相同正文保留该全序的最佳解释；这只是可解释的研发策略，未证明语言质量最优。

## 反例、修复与完整候选对照

保留原始失败，未通过缩小分母或只比较正文来消除差异：

| 原始问题 | 修复及最终证据 |
|---|---|
| 新 API、Host 切换未实现，编译失败 | 先保留 compile-red，再实现真实接口和四项 Host 配置测试。 |
| 模糊规则顺序改变图顺序，而配置身份相同 | 规则规范排序；保存实际 red/green。 |
| 长纠错 `jintiantiqnqihenhao` 耗尽候选预算 | 精确前后可达性过滤死弧；同一断言 green，原预算不变。独立266输入、32484完整弧路径与未过滤参考一致。 |
| 第一轮592次完整 Candidate 不同，正文/分数/词条 ID 相同但来源 edge 不同 | 新图增加最终来源平局，完整 Candidate 比较继续含 edge/Features；不归一掉来源。 |
| 新 Budget 枚举令旧测试穷尽匹配编译失败 | 增加新预算分支的显式拒绝，不删旧断言。 |
| 实际窗口 F8 两次未切换纠错 | 保留失败观察，增加同一窗口 owned 按钮复用同一设置函数；不改系统快捷键。 |

最终新方案小图：1395输入 × 4方案 × 6评分器，**33480 个完整 Reference 查询、83700 次完整 Candidate 对照，0差异、0不完整、0 Beam 剪枝**。PositionDag 对上下文模型的16740次明确不支持不计入成功比较。旧入口字符模型 **22320 Reference / 44640 完整对照，0差异、0不完整、0剪枝**。首轮另有49图预算、294 Reference不完整、735优化对照不完整，全部保留。参考搜索与优化搜索来自实际 crate，并不在测试里复写业务逻辑。

所有409合法全音节及多条合法长输入在纠错开启时都不生成 Correction 弧；409音节同时开启7个模糊规则仍保留精确弧、合法音节和限制。**这验证工程误分类反例，不是正式输入误改率。** 2793个既有报告/数据/测试/许可路径及230个旧 Rust 测试函数保留。完整方案/默认候选身份、预算、上下文和配置失效测试均保留。

## 真实词典 Top10 与质量边界

下列为固定123544条词典、研究 Lexical、K10 的真实输出，不是人工 gold。完整正文、ID、Features、音节来源与预算保存在 [全部观测](input-round18-evidence/performance/observations.md) 和相邻压缩原始 stdout。

| 方案/原始输入 | Top10正文 |
|---|---|
| 全拼 `nihao` | 你好 / 呢好 / 你号 / 尼好 / 拟好 / 呢号 / 你毫 / 你耗 / 呢毫 / 尼号 |
| 简拼 `zgr` | 撰稿人 / 张国荣 / 中国人 / 在个人 / 中个人 / 这个人 / 在工人 / 种个人 / 之个人 / 者个人 |
| 简拼 `bjdx` | 北京大学 / 比较大学 / 不仅大学 / 比较对象 / 北京对象 / 不仅对象 / 不决定性 / 比较东西 / 背景大学 / 北京东西 |
| 简拼 `zg` | 中国 / 这个 / 职工 / 整个 / 主管 / 中共 / 最高 / 资格 / 主观 / 足够 |

混合 `zhonggr` 首选中国人，小鹤 `nihc`、`vsgorf`、`bwjkdaxt` 可产生你好、中国人、北京大学；`nh` 首选内涵，**你好不在Top10**。这些结果揭示短简拼歧义与排序不足，不能用非空率、机械差分或词条数量代替准确率。

123544条原 runtime SHA `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`；Round17补层127354条 SHA `c9ab8b56b9d960091a1c39c37b7c1df7882ff50de2a64767174af8f4244c219f`，原1468隔离记录未改。词频 unknown 不补造，既有研究模型及训练计数未改，无 heldout 候选评测、人工签名或新质量数据。正式 **Top1/Top5/MRR 继续 null**。

## 实际 Windows 窗口

使用同一 `--interactive-input-host`、真实 Rust 双引擎和候选绘制/命中类，仅向 owned 文档写入。交互入口 K10、每页5条，既有只读 review/无HWND benchmark仍K5。原始 UIA、document Value、时序、截图身份和二进制身份见 `round18-ui-*`；截图由自动化工具当场显示，未保存图像副本，不把候选截图当内部正确性证据。

| 实际操作 | 结果 |
|---|---|
| 全拼连续 `nihao`，Space | 你好一次上屏。 |
| F6简拼 `nh`，PageDown，数字2 | 第二页男孩/内核/内河/脑海/能耗，内核上屏。 |
| F6小鹤 `nihc`，再切全拼 | 小鹤显示你好；原文nihc保留、revision增加、旧候选关闭；Escape取消。 |
| F7 n/l开关，原文ni | 精确你保留；关开关保留原文并更新revision。 |
| owned纠错按钮，`nihso` | 显示你好；Left/Backspace从中间删s成niho，再插s恢复nihso；鼠标选择你好一次写入。 |
| Unknown故障：ni+Space，重复Space，F6 | 写后文档为你好你；Unknown隔离后重复Space不再写，F6明确拒绝。核对后开始新会话，空Space不重放旧提交。 |
| 新窗口混合 `zhonggr`，Enter | 首选中国人并一次写入。 |
| 切全拼、纠错关，`qz`，Enter | 无候选，完整原文回退，文档中国人qz。 |

首段操作被失焦打断时，n/i出现在**恢复材料区**而非文档；最初 UIA 名称与文档值混在一起容易误读，后续以 EDIT Value 和实际画面核对，文档未被提前写入。未将该中断片段算作连续输入成功。正常连续输入复测另存。两个最终自有窗口均正常关闭，未向其他应用输入文字。

首窗口绑定源码 `b57f61b` / 二进制 `eedd7c7eb7faffc9748d78dc40667346514dcd9dbeec3a2e720bafd3be49bfef`；纠错/Unknown及混合/回退窗口绑定 `b948f98` / `d2f14f36c1d79f29b282ff37f527527a070bced0dd2f4ff4e5a5443288402e4f`。旧窗口和新按钮验证分开记录。**未执行实际TSF、跨应用输入法、快速连发压力、粘贴/代理对键入、8h/24h或正式Windows验收。**

## 同条件延迟、阶段与资源

同一硬件/原词典/模型/171事件，每版本每模型5个新进程，每组855事件，无warmup剔除。15组默认输出一致，仅删除新增且严格为零/None的兼容元数据；不归一正文、ID、评分、路径、统计或非零解释成本。普通 Windows 桌面研发测量，非专用低延迟环境。

| 默认K10模式 | n | P50 ms | P95 ms | P99 ms | max ms |
|---|---:|---:|---:|---:|---:|
| Round17 Lexical |855|0.7351|4.2114|5.9277|8.1426|
| Round18 Lexical |855|0.7740|3.8933|5.2491|7.5222|
| Round17 add-one |855|0.7127|4.9480|11.1612|16.1028|
| Round18 add-one |855|0.7254|5.3363|12.7359|18.4859|
| Round17 Witten–Bell |855|0.7497|5.5452|11.8701|17.4100|
| Round18 Witten–Bell |855|0.8042|5.9186|12.1073|17.5397|

字符两模型P95分别增加 **7.85% / 6.73%**；默认关闭不运行解释图，但共用候选/成本元数据并非零开销，不能声称全面提速或5ms门禁通过。旧R17独立批次6.20/6.06ms与本次新旧配对不是同一采样，不直接混比。

| 新方案/模型，逐键K10 | n | P50 ms | P95 ms | P99 ms | max ms |
|---|---:|---:|---:|---:|---:|
| full/Lexical |420|0.1621|2.1245|3.2879|4.0325|
| full/add-one |420|0.2088|1.8573|4.3684|10.2307|
| full/Witten–Bell |420|0.1731|1.9958|5.3246|6.5835|
| 简拼/Lexical |420|6.5386|25.2546|31.5030|33.7253|
| 简拼/add-one |420|15.3052|22.9495|26.3408|34.9643|
| 简拼/Witten–Bell |420|14.3999|18.8248|21.5855|24.1881|
| 小鹤/Lexical |126|0.5123|3.4017|4.3235|4.3241|
| 小鹤/add-one |126|0.4232|6.0592|13.6949|14.1855|
| 小鹤/Witten–Bell |126|0.4584|5.8857|12.6405|13.5241|
| n/l模糊/Lexical |420|0.4641|2.6189|3.5425|4.6748|
| n/l模糊/add-one |420|0.4760|7.1153|14.0540|16.7864|
| n/l模糊/Witten–Bell |420|0.4318|6.8815|11.9228|14.6079|
| 纠错/Lexical |420|1.0639|5.8981|12.5943|18.1460|
| 纠错/add-one |420|1.3496|14.9875|21.3384|54.1398|
| 纠错/Witten–Bell |420|1.1191|13.1466|17.0993|19.0732|

120个延迟/profile进程rc0；**837个最终profile查询中90次预算耗尽（30个唯一案例×3轮）**，全部简拼+两字符模型，各15/20案例达到200000展开。原始失败正文和完整候选保留；输入后端在预算失败时不提供局部选择，原文可恢复。EVENT记录不逐键输出budget，因此90/837分母仅指最终profile输入，不推断中间每键都无失败。

解释、归一化、简拼、映射、模糊、纠错、交集、格、搜索、评分、桶去重、最终去重/物化均单独观测，全部 P50/P95/P99/max 见 [阶段原表](input-round18-evidence/performance/phases.md)。profile查询与普通查询先做完整输出相等断言，诊断时钟开销另计；scoring/bucket嵌套于traversal，不能简单求和。简拼主要耗时在遍历/评分，其次交集，未为了延迟提高预算或丢候选。

15个独立资源进程全部rc0，50ms采样；样本数14–54，whole-process working峰值约188–211MiB，private包含原件读取、预载索引、模型、搜索、帧与诊断，不是单算法内存、COM引用或Host预算。完整分布见 [资源报告](input-round18-evidence/performance/resources-summary.json.gz)。新弧来源容量在快照预算中计入，原128MiB内部预算未提高；不能用内部估算宣称RSS低于128MiB。

另运行既有真实双引擎 **K5无HWND Host**：4模型条件×3新进程×171事件，2052事件，12进程全部rc0。总处理P95 Lexical4.6398ms、add-one8.1425ms、Witten–Bell7.6991ms、研究补层add-one18.1594ms；core/projection/compare/transaction/frame/total完整分位数见 [Host原表](input-round18-evidence/host-benchmark/summary.json.gz)。该条件包含两引擎，不与K10单引擎混比。真实K10窗口另保存60个诊断样本，候选HWND更新P50/P95/P99/max为2.3903/9.0236/17.9452/17.9452ms，包含本次复合交互，完整阶段值见 [窗口样本](input-round18-evidence/interactive-window-samples.json)；不是固定事件集或系统级端到端性能。

## 回归、身份及最终CI

最终本地13项在 `cd16d0d9a8b8827ebcfd465440b370113fe4c927` 执行，工作树包含负例测试的纯rustfmt整理，随后以b948提交；生产源码不再改变。**Rust254 passed / 0 failed / 2 ignored，Python71 passed**，旧测试未删除，忽略项不冒称通过。

| 命令 | 返回码 |
|---|---:|
| `git diff --check` |0|
| `cargo fmt --all --check` |0|
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` |0|
| `cargo test --workspace --all-targets --locked --offline` |0|
| `cargo test --workspace --doc --locked --offline` |0|
| `cargo build --workspace --release --locked --offline` |0|
| `python scripts/check_workspace.py` |0|
| `python -m unittest discover -s tests -p "test_*.py" -v` |0|
| `python scripts/audit_windows_dependencies.py --audit-binary C:/Users/EDY/WebstormProjects/Cloudleaf/.local/audit-tool/bin/cargo-audit.exe --output .local/round18-verified-dependency-audit.json` |0|
| `python scripts/benchmark.py --iterations 100 --output .local/round18-verified-performance.json` |0|
| `python scripts/evaluate_algorithms.py --output-dir .local/round18-verified-algorithm-v1` |0|
| `cargo build -p cloudleaf-input --examples --release --locked --offline` |0|
| `cargo build -p cloudleaf-search --examples --release --locked --offline` |0|

每项独立命令/返回码/日志哈希保留，无链式最终退出码代替。锁文件不变、无新增依赖，无第三方输入法核心。cargo-audit0.22.2、许可文件身份及工具版本均保存。差分、性能、Host、工具、二进制和数据身份分开登记；性能的17个核心源码/锁文件哈希与最终实现一致，窗口按钮变化不冒称属于旧二进制测量。

本地输入工具SHA：`input_trace.exe add07d004c458aa811141920a1fa5589e142ed6a7ca418abb432a784c0971830`；`scheme_trace.exe b163b35e6b29fe01d2f881b6d18ede8ef398e34a83b8a4d53de7310a5915f723`。全部源文件/工具/数据/二进制哈希见 [索引](input-round18-evidence/index.json) 和身份记录。大日志确定性gzip，校验解压字节和CRC；下载CI ZIP校验CRC、GitHub digest、成员SHA。发布研发工具不包含词典、模型权重、用户配置或密钥。

实现提交已普通push：[b948f98的Windows/Linux CI](https://github.com/WanFengQC/Cloudleaf/actions/runs/43191081791) 两平台各22步骤成功，三个产物及原始日志ZIP已下载并验证CRC/digest。最终报告也采用普通push。**最终Windows/Linux push CI的精确SHA、两平台原始步骤和三个产物身份以交付回执为准**；报告不能自包含自身未来commit SHA，回执在最终push完成并逐项验证后生成。未将历史CI、PR merge预览或本地Windows结果代替Linux或最终HEAD证据。

## 剩余风险与Round19

| 状态 | 范围 |
|---|---|
| done（工程范围） | 统一解释/交集、完整候选成本与来源、全拼兼容负例、小鹤事实映射、事务配置、实际隔离键盘/分页/修订/回退/Unknown测试、严格小图对照。 |
| partial / P1阻塞 | 简拼+字符模型200000展开耗尽；简拼与纠错长尾和默认字符P95未达实时目标；短简拼候选排序、原语言质量不足。没有通过提高预算、缩分母或默认启用新方案放行。 |
| not implemented | 微软/自然码、双拼纠错、未完成前缀模糊、增量计算/跨事件抢占、用户词典持久化与学习、正式TSF设置/应用交互。 |
| external blocked / not-run | 独立人工gold与授权、正式模型/词频和质量验收、隔离系统准入/真实应用TSF、8h/24h稳定性。 |

本轮修复的来源平局与规则身份错误未遗留已确认P0；**没有足够证据宣称不存在P0**，系统层可靠性仍未验收。未采集真实输入历史，未写学习/持久用户数据。稳定词条ID、基础/用户层、成功提交后事件接入只是未来边界，不能把Mock确认当学习事件。

Round19可在保持这些限制的前提下推进用户词典独立层、稳定ID、迁移/备份/删除与可关闭的本地选择学习；先设计持久化与失败恢复，不默认收集全文。同时解决简拼搜索预算和延迟，保留本轮完整参考/候选来源对照；继续寻找独立语言质量证据。**不启动CL-010，不将研究补层、机械结果或隔离窗口成功声明为正式输入法验收。**
