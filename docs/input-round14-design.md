# Round 14：中文候选与Windows宿主受控试验

2026-11-02。唯一维护者为用户，Codex负责实现与可复查技术验证。main、PR1–5及翻译工作区均不修改，不进入CL-010，不注册/激活/安装TSF，不运行8h/24h。

## 已核事实与调用链

共享基线`83a2666fa748b4ae9cba41a5880ba5353e71c9a3`；B搜索`59dabdc2f8a3dc87cbd7f19ed89401ee607f7954`、C数据`3f26f34e00c0775124f87e150a021f99fb52c567`、A平台`e0ab78c2786d652a3a19cb544b4dc83996e63264`。三个方向与共享merge-base均为`9af9bc1b2ebacc3ad76124cfa39d09de8eb77bd1`，生产input/types/pinyin API相同；共享新增测试必须保留。

默认链路：`Key → InputEngine::handle → refresh → pinyin::parse(最多32路径) → candidate::generate → Dictionary整词/末音节前缀 → InputFrame → select/Space/Enter → CommitRequest → 宿主三态 → ack/隔离`。它不能组合多个词条形成整句。

A已有真实COM服务、CandidateUi、ownership/FIFO/watchdog/recovery。`OnTestKeyDown → keys::normalize → native_probe → key_disposition`只读；真正接管由`native_key → projection.handle → edit session → run → engine.handle → same_semantics → render → update_ui`完成。候选Finalize绑定revision/id回流Host。不能从零重写或以模拟COM代替真实应用验收。

B已有schema2 Lexicon、Trie全输入格、Reference/ExactDag/PositionDag/Beam与阶段观测。C的Rust旧Dictionary仍为v1；C序列化index.json是Python索引，不能声称B直接读取。正式runtime经C verifier校验后一次建Rust Trie、Arc共享，按键不读文件。

## 文件整合范围与依赖

1. 首先按精确B提交带入`crates/cloudleaf-search`全部源码/examples/既有测试，及其测试fixtures、资源manifest与search审计政策。保留共享核心、旧测试、默认算法。按路径记录原Git blob，不用整分支覆盖或ours/theirs。
2. 复用C已取得的固定123544条包，Git外保存；带入必要来源锁/许可/批准范围及离线校验工具依赖，原1468隔离记录和unknown保持。新Rust载入入口对实际runtime SHA校验，包级许可/索引检查由已有C离线verifier执行；不下载新版本。
3. 最后带入A完整adapter/apps/windows-ime模块及原测试、15个精确Windows包/许可与审计入口；根Cargo/CI/政策逐项合并，不覆盖共享测试或B资源。
4. 本轮新增`cloudleaf-input`显式search后端、诊断/快照、CLI实际输入探针与集成回归；Windows只增量注入同配置引擎、真实容量计费及候选命中修复。

## 接口与行为约定（实现前固定）

- `InputEngine::new(Dictionary)`和默认构造保持旧候选路径。新增显式`with_search`，携带已验证、不可变Arc词典快照及受限search Config。默认服务不自动启用试验后端。
- 输入仍为64字节ASCII组合；Space保持选词，不把带空格拼音注记当键流。连续拼音、显式撇号分别测试，不扩展Unicode光标模型或偷偷改变模式规则。
- 直接构建B词条格，不先经旧32路径parser；默认试验选择PositionDag+Structural，保留其他方法可对照。输出Top-K≤50，context_pool须≥output_k；超预算或出错清候选并保全完整原拼音，不把部分路径当完备Top-K。未知局部片段暂为全原文回退，不伪称已实现混排。
- 仅试验分支将`Candidate.frequency:u32`改为`Option<u32>`。旧生成器与CLI对照构造用Some保留旧人工原型值；正式unknown为None，组合路径无观测整体频率也为None。平台不读取此字段重新排序。不会给unknown填0或伪造学习统计。
- Candidate ID由快照SHA、原始输入和实际路径/边界的SHA256生成，避免projection/engine修订号差异造成语义比较失败；revision仍单独严格校验。采用已在PR5审查过的固定ring0.17.14，仅hash，不引入网络/翻译。精确新增闭包另审计/保存许可，不复制翻译模块。
- 有界诊断按当前revision保留snapshot、方法、预算、score/features、entry ID和原文字节边界；来源/许可从同一不可变快照取回。诊断计时不进入EventResult，不破坏Host的确定性语义比较。每次编辑/取消/配置变化使旧诊断和候选失效。
- 更换快照仅在宿主加载阶段完成；核心显式替换须拒绝pending/failed事务，成功时保留原文、增加revision并全量刷新。失败不得丢原文或复用旧版本ID。
- 不实现未经验证的用户学习，不读取未批准现代统计数据。Bigram/上下文模块继续保留独立对照；本轮输入接入不凭空制造训练语料或启用无模型评分。

## Windows可靠性增量

Host实际执行与projection须由同一配置构造，Arc只读词典不复制；旧native_key/render/三态/FIFO/回收机制保留。恢复成本不能继续只扫描PROTOTYPE_TSV，须由后端的明确输入/候选/文本/诊断容量推导，保持原模块总限额，超限拒绝接管。共享词典载入与字节身份在宿主创建前验证，不在TSF锁内读磁盘。

已具体发现`WM_LBUTTONUP`底部padding可命中当前页之外第6条；先用真实原生CandidateUi和生产核心帧做失败回归，再限制绘制行和客户区命中，保证不重新排序、不抢焦点。裁剪/空布局隐藏、DPI/多屏按实际覆盖标partial，不借机重写窗口。

## 验收、风险与依赖

先固定设计和导入来源，再保留失败测试、实施最小修复。执行旧默认全套回归、search既有Reference差分、真实输入编辑/分页/过期选择/提交、预算耗尽原文、损坏载入与换版本、原生非注册Host/UI与COM生命周期测试。适用Windows/Linux精确push CI另核，不创建第六PR，只普通push试验分支。

真实固定包manifest SHA`5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5`，runtime SHA`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`，index SHA`982c7ae244497d7f3cd68165e17f383655fe4c98470be5503182b6387011012e`。C verifier已只读重验，123544条全部unknown。已有B探针显示nihao首选你好、中国可见，但目标长串首选含错字、目标句不在前5；这是正式排序质量阻塞，不按期望样例补频率。

Reference/Exact真实长串可慢至约82ms或耗尽，活路径计数不是字节内存。接管两次计算需分别测量；预算是确定性资源限制，不冒称硬实时或用户取消。真实取消接口先限定独立搜索控制，不为同步输入新增后台调度器；未实现部分明确记录。

报告与回执记录源码/数据/工具/二进制、每命令返回码/原始日志、实际候选/路径/选择/提交、正式质量null及P0/P1。真实TSF/隔离环境授权、8h/24h、正式词频/独立gold、增量/纠错/模糊音/简拼/双拼/学习/用户词典继续按依赖推进，不删除最终需求。

## 实测后容量与取消范围补充

固定词库初次容器容量及树节点估计142035792B已超过旧128MiB Host门禁。先保留13条合法组件词条仍预留16个Entry槽的失败回归，再在装载完成后收缩entries/trie/letters/exact/prefix Vec，查询/排序/分数不变。实际容器预留减少23538688B；树节点是Rust1.90布局估计，不包括allocator私有开销，不可称进程RSS硬限额。Host按两份帧/诊断和共享一次快照估计计费；搜索瞬时路径开销另由工作预算及真实进程资源观测说明。

候选输出容量按最短合法末音节前缀消耗计算，不能仅按完整读音比例。该固定包每消耗字母最多6B输出，64字母为384B；原文和路径元数据另计。

本轮同步核心依靠确定性工作预算完成或原文回退，没有新增后台任务，尚不实现用户取消正在执行的同步搜索。后续可在独立search入口引入只读取消token，明确Cancelled与Budget不同状态，在格建立/路径扩展边界检查，保留原文及未承诺的partial候选诊断；接入前须保证取消不会把过期候选提交到当前revision，不能把当前预算中断冒称真实取消已完成。

2026-11-03收尾检查发现：Git换行转换还影响了两份数据许可原文，仓库副本SHA与原来源锁不一致，外部九文件包不受影响。先保留真实副本与固定锁的失败回归，再从C已核原件逐字节恢复并设置data/licenses禁换行。预检和工作区审计入口共用来源许可字节检查，不修改来源锁/授权范围；此修复不更改输入算法或词典内容。
