# Round20：精确搜索成本、局部增量复用与整合准备

2026-11-16，基线`4d42363091361552ad525f47e3c6b1d581ae3a4c`，分支`codex/input-round14`。先核对AGENTS、路线、Round19设计/报告/回执及真实源码。工作区起始干净，保留所有旧测试、数据及历史失败。窗口自动化维持用户暂停，不启动GUI、TSF或长测，不改main和五个Draft，不进行分支合并。

## 已观察问题与范围

Round19在相同837个最终查询、K10、200000展开/50000驻留下仍有6次ResidentPaths失败，为两字符模型的简拼`n×32`各三轮；完整查询延迟未达5ms，Lexical与字符模型都存在退化。搜索状态保留完整正文、ID和edge向量，后缀也保存完整路径；需先实测识别分配、平局比较、驻留计费与后缀重复工作，不能猜测热点或提高预算。

搜索变更限定`cloudleaf-search`现有精确路径和必要测试，不改Reference定义或完整Candidate比较。不同字符历史、解释代价、偏好和路径来源不得未经证明合并。未完成Reference不能充作答案。原始6次失败、超时和新增失败均保留，预算失败仍有原文回退。

局部增量复用限定可证明兼容的已有计算，不把旧Top10当新输入答案；缓存身份、失效、容量与观测先补充到本约定，再实现。输入session、revision、raw/cursor、scheme、词典/模型/用户状态及排序配置的变化必须明确处理。复用仍生成当前revision的候选身份，取消不能发布半成品。没有实测收益的实验不默认开启。

`cloudleaf-input`负责局部计算复用及事务边界，Windows只补无HWND实际worker/Host协议回归；存储不重写，不自动启用学习。原Applied证据、Unknown隔离、后台文件操作与128MiB估算门禁保留。

专项回归直接调用真实LocalHost与Store/worker：确认只有Applied结果附带选择凭据，NotApplied重试须绑定新commit、Unknown隔离且不重放；填满实际有界命令/结果队列后派发应拒绝而不等待磁盘，shutdown排空既有任务并释放文件锁；导入无效文件后原代/原词条仍可继续操作。它们验证协议，不把构造sink的结果当作真实Windows写入。

## 本轮独立工作与验收

真实输入工作区仅主执行者写入。搜索优化、局部缓存及只读整合审查分别在独立临时副本完成并提交设计、反例和结果，主工作区依次审查应用；不并发修改共享接口。

所有对照固定原123544词典、研究语言模型、837最终查询、171逐键事件、用户层OFF/空/4/512/4096、K与预算，分开记录最终查询、Engine K10与Host K5，保留P50/P95/P99/max、完整性、资源与缓存观测。完整Candidate/Features/edge/来源/顺序严格比较，不能只比较正文。正式语言指标继续null。

实际修改后逐命令执行原13项检查，绑定源码、数据、工具和二进制SHA。普通提交推送后等待最终完整HEAD两平台push CI并核验日志/产物SHA与ZIP CRC，再生成Git外交付回执。

只读Git比较核实五个方向的实际HEAD、merge-base、历史与patch差异，必要时使用不改ref的merge-tree预览；输出`cloudleaf-integration-readiness.md`。未来人工窗口清单写`testing/round20-windows-manual-acceptance.md`，全部未执行步骤保持`not-run/user-paused`，不虚构按钮、人工批准或真实Applied。整体仍需各自原门禁，CL-010关闭。

## 局部复用具体约定

# Round20：跨键复用不可变Trie音节转移

基线4d42363091361552ad525f47e3c6b1d581ae3a4c。本子任务不修改真实工作区。

`schemes::from_phonetic`每个相交状态都会重新按音节字母遍历词典Trie；同一node与规范音节在同一次、追加、退格、中间编辑及不同方案中重复出现。缓存仅记录这个纯函数的下一node或不存在，不保存Candidate、旧TopK、正文或整句，不复用旧路径。

缓存完整键为不可变Lexicon的Arc identity、起点node、规范音节。不同raw/cursor/session/revision/scheme/模型/偏好可能改变音节图及路径排序，但不会改变给定词典节点的字母转移。每次仍从当前完整音节图构建所有边并运行现有完整搜索；原相交visits/预算检查位置不变。因此不跳过来源、编辑成本、搜索排名或候选身份生成。缓存有无命中不改变统计分母和错误类型。

词典切换按Arc identity清空；InputEngine的资源、模型和配置替换构建新backend；方案变化和clear_context显式清缓存。revision仅因追加/退格变化时保留纯转移；候选仍按新revision生成，旧candidate ID按原规则拒绝。会话内raw变化不清缓存，不保留原始输入文本。

固定4096个direct-mapped slot，每个24字节（node,next,syllable编码），约96KiB及对象余量，第一次非默认方案查询才分配。碰撞只覆盖旧缓存，不删当前搜索状态；未命中永远走原Trie。规范音节为解释器已验证的最长6个ASCII字母，base27编码u32无冲突，不在内部重复校验。缓存开启/关闭/clear有累积hits/misses/evictions/invalidations，不写输入日志。

默认全拼直接走原Lattice路径，不访问/分配缓存。先独立验证实际收益，收益不足不默认开启。Host必须预留两引擎最大缓存字节，原128MiB门禁不变；root修改session.rs，本子任务不触碰Windows适配。

文件：search新增transition_cache.rs及测试、lib.rs导出、schemes.rs增带缓存构造入口；input sentence/lib少量接线、clear与只读统计/开关；独立新集成测试和研发benchmark example。search.rs和character.rs由另一子任务独占。

严格验证缓存ON/OFF：全部Candidate/Features/edge/排序/fallback/预算和SchemeStatistics一致；追加、退格、中间修改、全拼/简拼/小鹤、模糊/纠错、用户词/偏好和模型/资源切换；同node不同词典不得串用。真实固定词典同机配对测量跨键复用、命中与淘汰，保留所有时延、失败、SHA，不声称语言或TSF质量。

## 精确字符搜索具体约定

# R20 字符精确搜索：以真实完整后缀候选建立剪枝门槛

基线4d42363091361552ad525f47e3c6b1d581ae3a4c。仅修改search.rs的interpreted_character_exact；不重写格、评分器、Reference或默认旧字符搜索。

实际固定原件20个简拼输入观测：后缀计算0.4–4.6ms，随后路径枚举约1.5–33ms；n×32在labels2593、seen23743、pending23659、materialized29857、166194 expansions时触发50000驻留。大量完整Path和签名在尚无第K个完成候选门槛时生成。原始profile见profile/*.stderr，测量含临时阶段钟，绝不作为最终生产性能。

已有精确后缀不仅是分数界，还保留实现该界的完整edge链。对每个起点首边，按其末字符/剩余编辑查同一后缀，把首边和后缀通过真实scorer.extend/finish物化为一条合法完整路径。用原insert_unique维持至多K个实际完成候选。这些候选是可实现的第K名门槛，不是取代搜索的预测结果。之后仍运行原A*；只有某前缀的完整最优后缀全序不优于当前真实第K名时才剪。该界已经包含解释成本、用户偏好、模型score、稳定ID及edge来源；不合并不同历史，不删除同正文的更优来源。

首边及每个物化尾边均计expand_allowed、检查cancel，种子当前和新副本计resident_allowed。200000/50000与K10保持原值。若预算中止仍返回明确budget/fallback，不以种子部分结果伪装完成。种子只从已计算有效后缀产生，所以语言模型/编辑可行性不另猜；真实Candidate所有Features由原scorer产生。

回归：24个同拼音汉字、各自重复三次的机械字符训练；输入nnn。原实现在同样256驻留预算不完整，新实现应完整；Reference以同样预算枚举全部24^3路径并比较完整Candidate对象，两种字符模型都验证。保留旧900展开反例、188325组原差分与原837固定查询。实际n×32大词典只能验证正确性界和无budget，不拿不完整穷举作gold。

后续局部优化可在复制Path前比较借用的完整后缀界，避免明显劣于门槛的正文/Features分配；须复用现有评分规则，单独保留红绿和严格差分。没有证据时不引入大缓存或更改resident计费。
