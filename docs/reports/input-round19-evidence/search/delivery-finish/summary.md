# Round19 搜索专项复核

同一123544条原词典、固定研究模型、原20个最终案例，K10，200000展开/50000驻留预算。每条件3新进程，无warmup/长尾剔除。以下是单次最终查询的格+搜索时间，不是逐键Engine、Host或TSF延迟。

|方案/模型/版本|n|预算|P50 ms|P95 ms|P99 ms|max ms|
|---|---:|---:|---:|---:|---:|---:|
|full/lexical/old|60|0|0.0307|1.5580|2.6870|2.6870|
|full/lexical/new|60|0|0.0422|1.5232|2.5599|2.5599|
|full/addone/old|60|0|0.1277|1.4961|1.8251|1.8251|
|full/addone/new|60|0|0.1519|1.7733|1.9994|1.9994|
|full/witten/old|60|0|0.1273|1.3655|1.4077|1.4077|
|full/witten/new|60|0|0.1357|1.5647|1.7361|1.7361|
|abbreviated/lexical/old|60|0|6.9366|27.6448|34.5205|34.5205|
|abbreviated/lexical/new|60|0|11.1998|40.8496|113.3934|113.3934|
|abbreviated/addone/old|60|45|16.8544|24.3197|25.9239|25.9239|
|abbreviated/addone/new|60|3|9.9718|51.8392|84.5671|84.5671|
|abbreviated/witten/old|60|45|18.0153|30.8579|62.4605|62.4605|
|abbreviated/witten/new|60|3|10.8976|51.9204|60.8732|60.8732|

36个进程全部返回0，720最终查询；270组双方完整结果的全部Candidate相同。full条件还比较完整SearchResult统计，仅删除新增严格零值user_preference_units，180组全部相同。旧版本不完整的90组不被计为正确性证明。

新方案严格机械差分：1395×4×6=33480完整Reference，83700完整Candidate比较，0差异/0不完整。实际用户层和偏好：1395×5×6=41850完整Reference，104625完整Candidate比较，0差异/0不完整。PositionDag对上下文模型分别16740/20925次明确未支持，不计为成功比较。

CharacterModel::rebound仅重新编译词条字符缓存并绑定新Lexicon实例，观察计数、全部转移与未知映射不变。缓存重绑与固定概率的单元测试通过。用户偏好正负例验证开启后排序生效、解释成本仍更优先、清除后恢复候选对象、完整Reference相等；未生成真实个人词库或人工gold。

初次成本上限便宜原型只修复8/30，未采用。最终采用稀疏精确后缀最佳优先：同首字符组保留完整成本/偏好/score/ID/edge最佳尾，已观测行和未观测基线分别比较；预算计入标签扫描/边/行/探测，取消检查保留。Reference、Beam与默认关闭字符搜索未替换。

资源采样是完整进程（原件缓冲、词典、模型、查询、输出），不是单算法内存或128MiB内部门禁。普通Windows桌面，不承诺5ms实时验收。完整资源样本和返回码见paired-final各result/summary。

原R18全部最终案例复跑：837次实际查询，预算失败6；唯一失败为[['abbreviated', 'addone', 'limit-budget', 'Some(ResidentPaths)'], ['abbreviated', 'witten', 'limit-budget', 'Some(ResidentPaths)']]。不是将不完整Reference视为正确答案，也不以进程返回0冒称每查询成功。

本轮未提升预算、减少Top10、缩小词典或删除输入。原limit-budget n×32仍失败，预算由Expansions变为ResidentPaths，明确保留；不能宣称清零。
