# CL-008 Round 5：同算法可选阶段观测与真实质量前置约定

2026-10-27；起点`96949ddc32945e2e9dd9694f9ab7e6e1c4b0f65a`。先提交此设计和新增回归，再实现；Round1–4全部证据/测试及34,060旧样本保持原身份。默认输入核心、共享类型、算法、评分、预算及配置不变，不进入CL-010。

## 观测边界

现有宿主只测Lattice::build与search_graph合并墙钟，不能把它外推成某个算法阶段。归一化在建图内部，桶内sort/dedup穿插在每次搜索扩展中。新增研究crate内的可选`profile_query`及阶段记录类型，旧API保留签名/结果，默认Noop观察器不读时钟；使用同一实现而非复制一份算法。

阶段为normalize（含配置/输入拒绝）、lattice（Trie匹配/边生成）、traversal（路径扩展及其中桶处理）、bucket_sort_dedup（traversal嵌套子阶段）、final_sort_dedup、context_rerank、materialize（候选ID/正文输出物化）、query_total。每阶段保留attempt次数/原始纳秒；未执行为null，真实执行但空工作可为0。失败必须结束已进入阶段并保留错误，不用0替代未进入阶段；预算属于搜索结果而非错误，保留原结果/剪枝/失败分母。

`bucket_sort_dedup`嵌套在traversal；query_total包含全部阶段及观测开销。不能相加嵌套阶段，不以相减计算未观测的净CPU成本。每段使用Instant，记录时钟读取次数。大量桶排序的读钟开销可能明显，因此同配置另跑旧未观测入口并保留原始时间与所有结果全字段对照；不把观测版时间当产品延迟，不对本轮直接宣称优化。

独立工程宿主只载预冻结runtime，新增分阶段JSON输出，序列化/IO在query计时外。宿主另测报告构造时间，不能纳入搜索时间。原resource_probe和旧协议/报告保持原样。Python工具核对profile/未profile每例全部候选text/score/IDs/features/records、状态、waiting/fallback/预算/剪枝，无论是否完整均要求观测不改变行为；但跨算法质量等价仍只对完整参考给结论，不完整为null。

## 固定真实资源矩阵

完整CC-CEDICT镜像runtime SHA `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`，123,544条、全unknown。使用R4冻结九文件/NOTICE/LICENSE及原24无gold探针、12配置、30测量/5预热、不改任何搜索预算。每例保存采集顺序的全部阶段纳秒/调用数、未观测总耗时、结果/拒绝/预算、配置与峰值进程内存。计时样本与身份单独新目录，不覆盖旧矩阵。保持Reference、ExactDag、PositionDag、Beam的差异。

新增测试先在旧代码红测：正常/空/等待、非法输入归一化失败、图边/扩展/驻留预算以及Beam剪枝都有观测记录；观测结果与旧入口完全一致；阶段嵌套/缺失状态、采样数、错误和全部分母不可被分析器删掉。再运行全部旧/新Rust与Python、README格式/Clippy/doc/资源检查、机械基准与原型/83种子。

## 正式质量前置约定

本轮继续执行R4来源许可、全局family/near、来源内source_unit、规范输入/task摘要、70/15/15家族配额及至少2000heldout家族门禁；新增明确的真实数据准备契约与对抗性回放，人工标注、语义聚类、保管者真实性仍须独立审核。任何来源被换名、家族被拆号、训练/开发/heldout的预处理不同、标注来源未绑定、封存或参数不一致都不能靠“有非空候选”放行。

当前实计划依然缺独立人工gold和正式评估语料，必须exit2并保留语言指标null。完整授权词典只用于资源与机制实验，不能派生gold或以模拟声明补齐实际计划。本设计不授权下载新语料、调heldout、替换默认输入或放行生产算法。

## 文件范围与交付

研究crate新增profiling模块，并重构lattice/search内部共享实现接收Noop/可选观察器；旧公开入口不变，新增入口仅研究工具使用。新增工程example、Python评估/分析及语义测试、正式准备契约、本轮报告和README入口。无A/C或共享接口修改。冻结实现后记录精确commit、源码/二进制/数据/协议SHA、原始输出/失败，最终普通push Draft PR2，由根单独核对新精确push双平台CI。
