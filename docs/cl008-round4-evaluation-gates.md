# CL-008 第四轮：正式评估前置门禁与独立受限穷举

2026-10-23，起点 `1a7b006`。本设计先提交再实现。R3全图、固定13例超时、Beam损失及未知词频原样保留；不改默认引擎、共享接口、搜索算法、既有协议和阈值。

## 正式评估前置门禁

新增离线 `scripts/check_language_readiness.py`，只核对前置证据与完整分母，不打开封存答案、不运行语言打分。即使材料一致，输出的 `language_accuracy_valid` 仍为false，Top-1/5/MRR为null；它不代替正式评估、人工授权审查或盲测保管。当前没有独立人工gold和保管者，实际检查点必须拒绝，不能用测试fixture填补。

计划JSON绑定既有 `cl008-real-resource-protocol.md` 仓库UTF-8/LF字节SHA及注册时间（仅协议文档规范化换行，避免Windows检出CRLF改变同一Git文档身份），固定家族70/15/15、heldout至少2000家族。这是既有预登记门禁，不是现有数据已满足或比例/样本量具有统计最优证据。确定性整数家族配额采用最大余数法，余数打平按train/dev/heldout顺序；不按测试结果改变配额或门限。

来源分别登记lexicon/evaluation-corpus，含ID、版本、官方HTTPS URL、许可证正文与独立审核文件及SHA、允许评估的明确权限和批准范围；商业使用/修改/再分发各自保留布尔值，不把仅评估许可升级成可发产品。审核文件一致不证明审核者诚实或权利绝对无争议。gold声明必须绑定evaluation-corpus、独立人工标注协议/审核、非合成且非词典生成、未使用旧83 seed，并明确独立保管者；自填声明不能变成自动证明人工独立性。

不含明文gold的JSONL清单字段固定为：`case_id/family_id/source_id/source_unit_id/near_duplicate_group/split/normalized_input_sha256/task_sha256`。同家族、同来源段落/近重复组、规范输入或完整任务摘要不能跨split；案例ID不重复。规范输入摘要由保管者按ASCII大小写及ü→v规则制作；工具只能核对摘要一致性，近重复语义仍依赖人工分组。工具同时计算旧41事件与83已见seed的规范输入摘要，遇重叠拒绝，不靠改ID逃避。

封存记录绑定完整家族清单SHA、封存文件字节SHA、独立保管者/评估者、未打开声明与封存时间。只散列封存文件字节，不解析答案；加密真实性和独立性是保管方证据责任。参数冻结绑定参数文件、当前源码指纹、实际二进制、词库文件及SHA；冻结时间不早于注册/封存，并在运行前完成。可选预测结果必须绑定相同参数/源码/二进制/词库指纹，覆盖heldout全部case恰好一次；拒绝、预算、无候选和未实现能力保留，不能删分母。此工具仍不计算预测是否命中答案。

检查采用分节失败报告：来源、人工gold、家族隔离、封存、参数和预测完整性分别标记。提供当前缺材料的真实阻塞计划，不生成批准数据。测试fixture仅模拟文件格式与篡改/泄漏，不登记为真实许可或正式gold。

## 独立受限真实词条验证

新增 `scripts/evaluate_restricted_oracle.py`，直接核验既有Unihan evidence全部文件SHA；从其runtime按以下先冻结规则构造**新的受限子词库**：固定读音集合 `a/an/na/nan/xi/xian/shi/zhong/guo/bei/jing/xue/lv`，每个读音按原稳定ID升序保留前2条。原ID/读音/文本/来源/许可/unknown全部原样，未命中的键显式记录，不补词或选用Rust输出。最多26条真实记录，显著缩小同音分支；结果只适用于该子图，不解除原全图未完成Reference的限制。

固定无gold探针：空串、`ana/anana/xian/xi'an/shishi/zhongguo/beijing/xuexi/lü/zhongg/zhong'/zzzz/ni,hao`。它们测试分段、合法前缀、边界、Unicode、等待、未知和拒绝，标记dictionary-derived resource subset，不作为语言答案。不得看Rust输出后挑词、改探针或删除失败。

Python独立实现直接按词条读音字符串与边界匹配构造边、枚举全部从0至终点的路径，不调用Rust Lattice/Reference、不读取Rust候选生成参照；Structural为每词-100/末音节前缀-300，频率unknown特征为0且missing按路径数计。按整数score及完整稳定ID序列排序、同文本取最佳路径，再取5；保留来源records和全部特征。明确图边/枚举扩展上限，任一耗尽标记不完整、等价结论null，失败不伪装成空图。

复用现有release `resource_probe` 运行Reference/ExactDag/PositionDag/Beam1/8/64；另对Exact/Position分别运行扩展1、驻留1压力配置，保留明确预算失败。全部配置固定5预热/30测量，保存有序原始样本、独立进程峰值、源码/二进制/子词库/来源/探针/协议/独立参考指纹。Python参考单独记录5预热/30测量原始样本和工作量，不将Python与Rust墙钟差异称作产品性能收益。

完整独立参照且精确策略完整时，逐rank比较text/score/IDs/features/records和状态、等待、原文回退；差异保存后非零退出。Beam保留损失但不要求无损；预算/拒绝双方相同不算完整正确性通过。R3机械Beam损失不因新子图简单而失效。若暴露实质算法缺陷，先单独设计与最小复现，再改实现，本轮不为了产生改动改算法。

## 文件、检查与退出边界

范围为本设计、新两个脚本、对应Python测试、`config/evaluation/language-readiness-v1.json`和`docs/reports/cl008-round4/`，以及README入口。无C/A修改、无共享接口和数据格式变化。冻结实现后在新目录执行当前阻塞门禁、受限对照及README全检查，保存命令/原文/返回码、UTF-8/LF发布与原件双SHA。显式提交并push草稿PR #2，由根核精确SHA双平台CI。

完成的是前置门禁工具和受限独立结构验证。正式人工gold/合法语料、词频和模型、全图资源目标、纠错/方案/学习/增量、默认核心接入仍未完成；CL-008及CL-010均不放行。
