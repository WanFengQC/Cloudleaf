# Round 15：语言证据评分与输入热路径

2026-11-03，从`codex/input-round14`的`75e0acee88d21647010a56a9d876c136c95e6d5e`继续。main、PR1–5、翻译分支与历史报告不动，不建PR6，不注册TSF、不运行长测、不进入CL-010。

## 实际问题与有界范围

Structural每词-100、末音节前缀-300；同音词没有观测信号时按稳定词条索引平局。真实123544条frequency全部None，Weighted与Structural完全同分。只读格诊断证明目标长句存在567条正文路径、最佳结构-700、格1482边，PositionDag无预算中断但Top50仍无目标；这不是词条格漏词，旧Top5上下文重排也无法恢复它。正式语言质量仍未通过。

现有BigramModel已实现有界ID序列训练、BOS/EOS/UNK、add-one和1024整数log2。优先复用，新增固定快照及来源/词典版本绑定，输入层通过显式配置预加载Arc模型，按键不训练、不读文件。ContextRerank保留有限召回池语义；Bigram/上下文不得使用只保留position的状态合并，必须保留last-entry，四搜索算法及原有拒绝测试不删。

本机固定Google材料仅一元分区和totalcounts，没有合法真实转移语料。先核对实际原件与许可范围，只有允许本地研究时才提取单独research_only词汇信号，固定时间窗口/原件SHA/词头严格匹配/统计解释。它不是production频率、DF或Zipf，不写回runtime frequency，不生成bigram，不以例句调参。缺失证据关闭相应项；未知不补0。无合格语料则完成模型工程与机械正确性，保持自然语言质量partial。

## 评分、身份与格式

沿用现有结构代价、现有manual Weighted与Bigram整数尺度，不宣称最终权重最优。新增词汇观测与转移/上下文分开诊断、独立开关，候选记录结构、词汇、BOS/转移/EOS、上下文贡献和固定模型版本。个人学习本轮不启用。固定模型使用版本化UTF-8 TSV、确定性顺序、正整数计数、固定词典SHA与stable entry ID引用、source/license/tokenization/train-split/scope声明及完整产物SHA；reader在入口校验一次、限制大小/计数/词条引用。声明和哈希只证明一致性，不替代权利审查或真人签名。未获得正式授权/质量门禁时只能显式research_only/mechanical。

修改模型或词典必须使旧候选revision失效；待确认或失败提交时禁止换模型/词典，原文和Unknown保护不变。普通构造仍是旧默认，实验模型必须显式传入。模型加载失败不得回退到手写原型频率。

## 性能与取消

R14真实阶段数据表明Traversal占主导，规范化和建格很小。实际代码每次扩展先克隆完整Path，随后多数被有界桶丢弃；先计算实际分数及ID全序，确定不可能入桶的扩展不物化路径，保持结果、稳定平局、预算与扩展计数。必须与Reference及旧实现逐候选/路径/特征差分，不能通过丢正确候选提速。Windows双引擎仍保留原事务隔离，不绕过same_semantics。

增量仅在能与全量格/搜索完全差分时启用：只复用不受编辑影响的词条格结构，任何字典/配置变化或预算截断回退全算；不能把旧TopK缓存称增量。若收益或正确性不足，明确未启用，不为形式增加缓存。缓存如采用只保存一个有界格，必须计入Host资源，不能隐式扩大预算。

合作式取消采用只读原子token、格建立/路径扩展/发布边界检查；独立Cancelled错误，与预算耗尽/墙钟超时/输入Escape/失焦分开。旧同步输入不引入每键线程或后台任务，平台异步revision接入本轮若未实现就明确未实现。取消不发布partial候选，未取消入口维持原行为。

## 修改文件、依赖与验收

按顺序修改：search的model/search/lattice及测试/examples；input的sentence/lib及实际整合测试；必要CLI实验选项和研发探针；只读离线研究提取脚本/来源说明；本轮报告和原始输出。无新Rust外部依赖，不修改共享类型/正式词典/生产数据授权，不扩展翻译/UI/学习。生产源码由一个写入者应用，其他审查只读或在temp准备补丁。

先提交设计，再保留旧实现失败测试/缺失API编译失败，实施最小修复。验收包括旧13条逐命令回归、实际评分/模型格式/身份/禁用/unknown数学测试、Reference/Exact/Position/Beam对照、取消区分、增量与全量（若启用）、Windows Host原回归。真实固定词库重跑R14同171事件并补输入，同输入数据/配置/预算比较Top10/路径/分数/预算/延迟；旧错误输出不覆盖，正式Top1/Top5/MRR保持null。最终普通commit/push后核精确HEAD Windows/Linux push CI、日志和产物，报告`docs/reports/input-round15-status.md`，Git外回执`.local/round15-delivery-receipt.json`。
