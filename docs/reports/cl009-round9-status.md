# CL-009 Round 9：固定 Ngrams 的只读诊断

2026-10-30，单人维护、Codex实施不代表额外真人审核。起点`ef6cf16c558f5521cd776e4805068a07f15aedd2`；设计和新API反例先提交`52c2daba6365b9f6674170c69eec6ce72aed38e0`，完整被测实现 **`72985c50904a41018058f1d05e3ce44902b58aa8`**。最终证据提交及Windows/Linux push CI由共享R9汇总单独绑定。没有下载其他版本、变更批准来源或发布产品数据。

## 修改与真实失败

新增`tools/data_pipeline/ngram_analysis.py`、`tests/test_ngram_analysis.py`及[先行设计](../cl009-round9-ngram-analysis.md)。CLI固定R8两份SHA，独立分析源文件、totalcounts和新输出；拒绝覆盖。流式读EOF/CRC，逐年原字段/标签形态分层，完整记录坏字段、重复、乱序、缺分母和单项计数冲突；异常例子每类最多5份行号/原行SHA，完整计数不截断。缺失不补0，异常或partial时结构缺项值为null。全局重复检测保存词头集合，内存O(unique tokens)，不是恒定内存。

[新API红测](cl009/round9/red/result.json)exit1是模块尚未实现，不是原生产算法缺陷；实现`3035197`后，[分母反例](cl009/round9/denominator-red/result.json)在`518259f`出现6测1error：缺少原始triplet_fields，只有valid计数会漏掉坏字段分母。修复`72985c5`在验证前记录原始字段数；不删旧断言或坏例，6项真实实现测试随全套95 Python通过。每次原stderr/stdout与返回码保留。

## 固定原件实际结果

[完整原始报告](cl009/round9/validation/analysis.json)绑定同一Chinese2019/`20200217/chi_sim`单个1gram分区和同版totalcounts。源gz **60,833,224B / `6ab29862a8c904c261bb9d5a02a342285c4dbabf79cee5c4f8818076fd44fb86`**；totalcounts **7,479B / `f4a136b63419c0f34dfbfcf76f501ac80a745ad535f5cc9bda2666189a35e9f3`**。原件只读，SHA未变；严格UTF-8、EOF CRC通过，解压195,878,750B，SHA`f5535ee7d0c0a5ba18bff3b4019d150da0e63265b8e2abdebb2d442db45b95f0`。

|实测项|结果与限制|
|---|---|
|行/唯一词头|508,674 / 508,674；不是正式词条或覆盖率|
|原始/有效三元组|19,074,163 / 19,074,163；所有登记诊断计数0|
|totalcounts与导出年份|464个唯一年，均1480–2019，分母中没有完全缺导出组的年|
|词头形态|215,764 other、292,899 tagged suffix、11 uppercase underscore；不生成POS事实或合并别名|
|结构缺项|216,950,573个token-year组合；只在完整无异常前提下按矩形分母计算，缺项不是观测0|
|耗时|CLI进程49.423秒、工具49.183秒，单次本机扫描；不比较输入法延迟或单算法内存|

各年field2/field3原始合计、三类形态分层、totalcounts四列及每个输入路径见原报告。field3跨词/跨年合计不能提供去重DF；全部field2含标签别名，不能冒充总token覆盖率。工具登记的结构约束通过不代表v3所有语义已验收，也不证明现代IME代表性。

## 统计方案与采用仍阻断

官方缓存直接证实的是版本、中文统计分词声明、具体分区许可及v1/v2的计数定义。v3词头后多三元组是实物观察；year/match/volume及totalcounts解释在报告明确为 **inferred-not-production-contract**，不是已审定v3协议。中文具体分词模型/参数/版本未知；FAQ40本书与旧版超过40次不能直接统一为v3截断阈值。未出现可能是筛选或截断，不赋0；2019末年的分母也不能当现代全语料分布。

未来token方案只允许同版/同year或已冻结窗口、分词/标签规则及合法全token分母；document frequency需明确book/document全集和原计数，无文档ID不能跨年去重；Zipf只有同词同范围正且已批准token比例p时才可计算`log10(1e9*p)`。本轮没有生产执行这些方案，**token/DF/Zipf及正式Top-1/Top-5/MRR全部null**。

派生诊断保留Google Books编纂来源、固定版本/原件SHA、[具体许可声明](https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/chi_sim-1-ngrams_exports.html)、CC BY3许可链接和本次只读统计修改说明，不混标项目MIT，不授权原书全文。分析为研究用途，来源登记仍candidate-not-approved，不创建approved锁。CC-CEDICT/Unihan原批准范围与Wiktionary research_only不变。

## 全套回归、字节和保全

[12条实际命令](cl009/round9/validation/checks.json)前10条0，覆盖拒绝2、真实pending标注audit2；fmt、locked/offline Clippy `-D warnings`、all-target Rust **60 passed**、doc、release、workspace审计与 **95 Python passed**。没有用CI推断真实平台或语言质量。输出与预先合同逐项比较；允许analysis返回2的合同仅为保留异常报告，实际完整扫描另断EOF/CRC及status和具体返回码，本次实际为0。

[身份](cl009/round9/identity.json)：source-v2 `f0b36b8e36eae0522bed191ab0498c975037153bc6714717acd7cd480e7ae284`；它不含tools/data_pipeline，因此另明确绑定新工具SHA **`fb78438ca3e4f79cb245ecb89219038bda615aa2d7866c305c131598904f0849`** 和实现Git提交。release CLI347,136B/SHA`d53f54a25bd6b4fb8c114a06966010d4b98bf0597efb7e9c5e897e96c59e5b80`；构建可复用既有编译缓存，不称新性能产物或执行基准。原33份文件有[无损封装](cl009/round9/raw-artifacts.json)，[运行器](cl009/round9/runners.json)与[Git LF清单](cl009/round9/checksums.json)分列原始字节。

起点210份历史报告、所有旧测试/Rust/来源锁/角色计划/许可均未改，正式9包本机文件逐项SHA不变；runtime仍123,544记录、1,468隔离、词频unknown。本轮新增6项机械fixture只测真实解析，不是人工gold。未重跑大词库构建压力、物理断电、激活/回滚或TSF。

## 语义发布器的实际缺口与单人职责

`tools/data_pipeline/pipeline.py::verify_release`严格接受schema2、cloudleaf-lexicon-v2和固定资源集合；`linguistic_sidecar.py::validate_sidecar`只核类型/引用。R8新语义bundle尚缺对应reader/verifier、内容双审及协议变更失效绑定、可信签署根/manifest验证、兼容旧reader和单指针升级/回退/中断恢复测试。本轮不改生产入口、不生成POS/义项对齐/反向翻译或授权。原有原子发布/回退检查随95测试运行，不冒充物理断电。

|状态|当前结论|
|---|---|
|done|固定SHA/CRC全扫描、字段/年/分母/重复/缺项诊断、独立统计方案、分母反例修复及全回归|
|partial|结构完整，v3含义/分词/截断/现代代表性与统计采用协议尚未闭合；发布器仍只有设计|
|blocked|负责人来源/统计采用决定、真实语义人员与授权/双审、独立gold和heldout保管未交付|
|not-run|生产词频/语义发布、签署部署、用户库迁移、物理断电与正式语言评测|

用户本人可继续审定统计协议与合法来源、原创个人标注和整理语义待办；独立双审、仲裁/heldout及实际授权不能由Codex或同一人的多次自审替代。本轮已修复新诊断的分母缺口；未执行的语言/数据发布条件仍是P1级放行阻断，不宣称风险清零。PR #3保持Draft，不合main，不进入CL-010。

报告归档时README新增末尾空行导致diff检查2；规范文档末尾后为0，修正前全文和进程原字节见[追加检查](cl009/round9/publication-check/result.json)。没有重跑扫描、重取原件或修改生产逻辑，未用链式shell最后命令的0掩盖该失败。
