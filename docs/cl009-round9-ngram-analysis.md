# Round 9：固定 Ngrams 原件只读分析合同

2026-10-30。先核实际 `ef6cf16`、R8完整取得记录和原件；共享计划 `547c308`。只分析 R8 的 Chinese2019/20200217 单个1gram分区与同版 totalcounts，不下载、不批准production、不改来源锁、登记、runtime、隔离读音或词频unknown。

## 输入与输出

新增离线 `tools.data_pipeline.ngram_analysis`。CLI要求明确原件/totalcounts/新输出路径，默认只接受R8固定SHA：gz `6ab29862a8c904c261bb9d5a02a342285c4dbabf79cee5c4f8818076fd44fb86`、totalcounts `f4a136b63419c0f34dfbfcf76f501ac80a745ad535f5cc9bda2666189a35e9f3`。不覆盖输出，不写输入，不产生词条或频率侧包。测试直接调用同一分析实现并明确提供机械fixture的字节SHA，不伪装真实语料。

先核两个完整文件身份，再流式解压至EOF核CRC/ISIZE、解压SHA和全部记录。逐行解析词头后多个逗号三元组，不套用v2四列。输出每年原始field2/field3诊断合计、词头标签形态分层和totalcounts四列；统计所有行/组、重复词头、重复/乱序年份、坏字段/数字、缺分母、field3>field2、单项超同年总计。异常计数完整、例子按类型最多5个且保留原行号/SHA，不首次异常截断扫描。致命gzip/UTF-8错误标partial，已读分母保留，不声称CRC通过。

缺失term-year只报告结构缺项数，不补0；重复词头不自动合并或删除。累计field3不是跨词或跨年去重DF；全部词头field2合计含标签别名等重复表示，不是语言覆盖率。标签后缀仅词头形态，不生成POS事实。检测全局重复需要词头集合，内存O(unique tokens)，不是恒定内存；每年聚合规模O(years)。报告实际运行秒数，不用整体进程峰值推测算法内存。

完整且无诊断异常退出0；身份不符、partial或诊断异常退出2，报告保留原因和分母。未知语义本身不改为工程成功或生产批准；无论退出码，production_approved=false，正式token/DF/Zipf与语言指标均null。

## 已知证据与未知语义

R8缓存官方info固定Chinese2019 persistent ID；具体索引列单分区/totalcounts与CC BY3编纂许可，非原书全文许可。总目录明确的是v1/v2定义；v3实际观察为词头后多组三字段，按数值形状及旧版说明推断为year/match/volume，**不是已审定的v3生产合同**。报告保留`field_2/field_3`名称和`inferred`解释。totalcounts亦保留四列，不因拟议解释伪造单位已验收。

官方说明中文统计分词，具体模型/版本/参数未知；FAQ至少40本书与v1/v2超过40次不能无依据统一成v3阈值。未出现可为未收录/截断，不能当观测零。实际年份范围、空年和分区情况只从固定原件报告，书籍/OCR/历史年代不代表现代IME分布。

## 独立统计方案，尚不执行生产统计

- token：将来在已批准的同版、同year/窗口、同分词/标签筛选协议下，真实match计数除以该协议对应全token分母；不能以筛选词头之和猜分母。
- document frequency：只在明确book/document定义与同年文档全集下保留原计数。无文档ID不能由跨年或跨词volume之和推导去重DF。
- Zipf：只有同词同源同范围、正且已批准的token比例p才可算`log10(1e9*p)`；不能从DF、缺词默认值或不明分母推导。

统计采用还缺v3字段协议、分词/年份/标签/截断与分母冻结、来源采用审核及代表性评估。CC-CEDICT/Unihan批准范围不变、Wiktionary仍research_only。语义发布器继续仅设计；下一步明确versioned bundle verifier/reader、内容审核失效绑定和可信签署，再谈现发布原语，缺真实语义双审/授权前不实施激活或虚构内容。

## 验收

设计与新API失败测试先提交；机械fixture覆盖完整v3、异常后继续、坏totalcounts、错误SHA、CRC损坏和输出不覆盖。完整固定原件实跑，不预设零异常。按用户要求执行fmt、locked/offline Clippy/all-target/doc/release、资源审计及全部Python回归；保留真实失败和新报告，不重复下载或改历史。最终Draft分支普通推送后另核精确HEAD的两平台push CI。
