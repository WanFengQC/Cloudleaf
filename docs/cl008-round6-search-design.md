# Round 6：稳定有序候选桶与可解释资源对照

2026-10-28。起点`08b09277d3beeb90e284145a3423b9f00eca8d6c`。已读取实际roadmap、Round5报告/逐例阶段原文与搜索源码。默认输入核心、共享接口、词典、评分、工作预算与遍历次序不变；不进入CL-010，PR2继续Draft。

## 实际瓶颈与窄范围修复

Round5 PositionDag标准配置遍历阶段P95 6.8862ms，其中嵌套桶排序/去重3.5045ms；Exact/Reference也重复该操作。当前每次扩展对已经有序、unique且最多k项的桶追加一项，再全排序，并为BTreeSet克隆全部正文。该源码行为使每次插入反复复制此前已保留的正文，不是根据整个进程峰猜测的算法内存结论。

改为私有稳定有序unique插入：新正文若已存在，仅当compare严格更优才替换；完全相等时旧的先到路径继续保留。新路径按compare的upper bound插入，保证相等顺序与稳定sort一致，末尾截断k。正文仅借用比较，不为去重克隆正文。Reference的complete桶、Exact/Beam的状态桶、PositionDag的位置桶复用该概念；最终不保证有序/unique的跨状态合并仍使用原sort/dedup。

必须保留原expand_allowed、resident_allowed、extend、计费及遍历次序。即使新路径注定丢弃也先执行相同预算检查/extend，不能因优化改变expansions、resident高水位、预算触发点、Beam剪枝、部分结果或候选来源。仅改变桶维护实现；不删除Reference、修改评分或调高预算。相等IDs但不同edge/features必须保持旧先到，不能只核对Top-1正文。

## 先行测试和资源证据

先提交本设计，再提交测试/工程宿主并保存旧实现真实失败，之后才修改生产搜索。语义回归覆盖同正文更优/劣序/相等先到、不同正文compare相等、k截断、多次插入和评分/预算边界；沿用全部既有完整参考与profiling差分。

新增仅工程example的同线程System分配器观测，测预加载图上的search_graph调用，保存累计allocation/reallocation请求次数和请求字节。分配器原样转发System，只在显式测量窗口计数，不进生产库或默认核心，不做live/peak/RSS推断。固定自编12个同音词、6个音节、k50的PositionDag资源回归要求调用次数不超过`12*expansions+5000`；该宽松工程上限用于捕捉每次扩展重复克隆约k项文本的实际浪费，不是生产资源预算或语言指标。输入图与词典在窗口外构建，结果在窗口外序列化。保存旧/新完整SearchResult Debug字段及同一宿主源代码、二进制SHA；这份机械资源夹具不是独立gold。

在根协调的独占重负载窗口运行完整CC-CEDICT固定123544条、原24探针/12配置/30配对/5预热，runtime SHA`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`，预算不变。新结果逐案例对照Round5全部已观测字段，同时plain/profile逐样本全字段相等，所有非法/预算/剪枝保留。阶段嵌套和不同分母不相加；进程峰仍含词库、双图双结果、JSON，不能作单算法内存。旧与新测量日期/桌面背景不同，时间对比只为诊断，不单独解除资源或质量门禁。

## 质量契约与边界

与C对齐实际人工任务的输入意图、拼音、可接受输出、权利、来源家族、规范unit及全局near分组；未来转换须显式核验artifact/preprocessing/protocol/review。不从待测词典生成答案，不填真实人员或许可声明，不用heldout调参。真实独立材料未交付时现有两个门禁保持exit2，语言指标null，不替换默认引擎。

范围：`cloudleaf-search/src/search.rs`、内部语义回归、新资源example/工程测试、方向设计/README和新Round6证据。完整Rust/Python/Clippy/fmt/doc/资源审计、旧机械与83种子回归、固定真实矩阵分别记录冻结commit、源码/数据/二进制/工具SHA及原始失败。保留Round1–5全部报告/原始样本和既有断言。最终普通push，由根核对最新精确Windows/Linux push CI。
