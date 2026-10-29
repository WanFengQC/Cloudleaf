# Cloudleaf Round 7：证据完整性、评估映射和审核失效修复

2026-10-29实施，2026-10-30收尾核验。完整正式产品目标不变。四个独立分支保持Draft，main仍为`a564fd47b0f054ff360592af3ccb8d744021160f`；本轮未合main、强推、注册/激活/安装/卸载TSF、向真实宿主注键或杀宿主，也未启动8h/24h或CL-010。

[先行计划](../cl007-009-round7-plan.md)在`4f19e96ad37c5c0dfb4877286f073f3f6fabe23f`提交，四方向起点均clean且本地/远程相等。实际读取AGENTS、路线/范围、Round 6报告和实际源码，不将文档能力当已实现。源码、数据、工具、二进制与报告提交分别绑定；本轮没有新合法人工材料或现代词频。

## 固定身份、检查和新CI

|方向|Round 6起点|本轮被测实现|最终交付HEAD|精确push CI|
|---|---|---|---|---|
|共享/PR #1|`c48584d411eceb82dcc6b8a30df9a33d9e995762`|`4f19e96ad37c5c0dfb4877286f073f3f6fabe23f`；生产代码仍`ce2c16e`|本报告后续提交|推送后在PR #1及最终凭据记录|
|A/PR #4|`caefb7f2ac7c8584249c05359fcb174f87640886`|`e50c1d88b34fbf824cd46d1f22c2035700cc75cf`|`fa4f1fab93394f115d3a987a7f0201b8662c08cf`|[40903941929](https://github.com/WanFengQC/Cloudleaf/actions/runs/40903941929)|
|B/PR #2|`417dad662bfebedf3238f909476240427cc9100b`|`2a2bd9a7ea9df73b647ccdba609d9aae8f0ad935`|`b4b5fb7f812c56f73a132cd9f16045b58a3121c2`|[40904643427](https://github.com/WanFengQC/Cloudleaf/actions/runs/40904643427)|
|C/PR #3|`52619a963070cd74c53d0b8d76fc40bf5b7a8327`|`e96ed5dc96c6cc0446f3446838fab9305047c1bc`|`3d573384ce1f31e437bdb4e7dc109cf438c261f4`|[40903386874](https://github.com/WanFengQC/Cloudleaf/actions/runs/40903386874)|

上述方向新head的Windows/Linux push事件、完整SHA、两job/全部步骤和两个未过期产物digest逐项核对，原响应在[本轮CI](cl007-009-round7-ci.json)。起点四个R6成功也重新核对并单独存[基线快照](../reviews/pr1-round7-evidence/baseline-ci.json)，不改绑为新版本验收。共享最终CI在本报告提交推送后独立核验，避免报告引用包含自身的未知提交；完整最终凭据在`.local/round7-final-receipt.json`及PR #1。

|方向|Rust / Python|实际检查范围与拒绝状态|
|---|---|---|
|共享|64 / 14|12命令exit0；locked/offline build、release、fmt、Clippy、单元/集成/doc、Python、资源审计和diff|
|A|157 / 25|10全量命令0、18专项均符合预期；四短档0/130/0/1，长档预检/启动守卫均2|
|B|81 / 76|17命令符合预期；base/preparation/handoff三个正式门禁实际2，语言指标null|
|C|60 / 89|11全量命令0，6专项0/0/2/1/2/0；pending审核/冻结及Wiki拒绝不写成正式准入通过|

没有削减旧测试、预算或失败分母。通用工程CI不能证明真实TSF、正式语言准确率、权限真实性或物理断电恢复。

## 共享PR #1：仅合并前审查准备

[独立审查材料](../reviews/pr1-round7-merge-readiness.md)覆盖相对main的实际可执行差异、状态/错误优先级、只读KeyDisposition、候选revision/ID、提交三态、UTF-8/UTF-16标量边界、分页配置及精确政策。完整main→R6共享差异112文件/2,337,028B patch和逐文件Git blob/SHA已归档；本轮只是审阅材料，生产接口、候选算法和原型资源没有变化。最终main→交付全目录及patch身份另保存在最终凭据，包含报告自身。

公开API回归驱动真实InputEngine/Dictionary；重复字符Key应逐次消费，合法自动连发不是重复回调。重复只读预判不变状态，旧提交确认不重复上屏；core ID只在一个引擎内唯一，平台仍须session/epoch隔离。实际改变帧的编辑/导航使旧revision失效；导航已到边界且帧未变时保留当前revision。UTF-16辅助不代表已有字素编辑。Unknown不发送失败确认以免允许重试；取消核心状态也不能证明原文档未写入。

AI只读第二视角指出上述导航措辞及workspace/外部审计范围两项P2表述，已按真实代码收紧，未改生产代码或伪造红测。[复核记录](../reviews/pr1-round7-evidence/peer-review.json)明确不是人工合并批准。本轮已审范围没有新的可复现共享P0/P1；精确最终CI通过后可进入人工合并审查，仍不自动合并。

三个方向实际merge-base均为`9af9bc1b2ebacc3ad76124cfa39d09de8eb77bd1`。四方向input/types内容一致，但方向尚未包含共享全部审阅回归/收尾。初始审阅JSON中的方向head为采样时中间提交；最终交付的差异和无破坏merge-tree预览另存[方向修改/兼容](cl007-009-round7-shared/direction-changes.json)，不能把旧快照当最终head或预览当集成验收。

## A：原始轨迹审核与长期执行准入

[CL-007报告](https://github.com/WanFengQC/Cloudleaf/blob/fa4f1fab93394f115d3a987a7f0201b8662c08cf/docs/reports/cl007-round7-status.md)和先行设计`7083c0f`逐项审阅Host/STA/COM/HWND/sink、watchdog、恢复纯值、module lease和卸载。七类矩阵分别列归属、可回收证明、用户状态、继续输入、隔离/终止和具体测试入口：正常、owner退出纯值可达、外部COM引用、owner永久阻塞、watchdog失效、预算耗尽及无法证明安全。Unknown/旧上下文/固定预算和生产Rust均未改变。

真实红测`7914589`先捕获旧原始审核器接受合法JSON缺键、缺周期、重复序号、会话或cycle变化；另缺字段/API错误单独标记，不冒充COM丢字。现在逐键/周期/唯一final核身份、六键顺序和写入分母；五份明确损坏的真实trace副本旧工具仍raw_errors为空，新工具全部拒绝。只证明诊断完整性问题已修复，不称生产文字曾实际丢失。

本轮增加机器/架构/CPU/Python版本与exe身份，8h/24h外部批准须绑定完整运行身份、隔离环境、负责人、快照、回滚和授权引用；预检只写新报告，无批准时拒绝且不创建运行目录/子进程。程序只校声明，不能代替真人确认环境或授权。续跑仍新segment，不拼接时长，不重放Unknown。

实际四短档：正常4秒80周期/480键、562事件/39采样；2.5秒中断22周期/132键、155事件，保留部分；新段独立80周期/480键；第3周期故障18键、child101/runner1。两完整段trace_complete=true，中断/故障false，各完整段32/32材料恢复核对，未做尾部的段成功率null。所有长期完成标志false。release DLL loader实际20轮句柄113→113、registered=false；不是系统输入法或真实应用验收。

正常短段admission P50/P95/P99=29,100/208,200/515,300ns；39资源采样峰句柄145/线程7/working set15,192,064B/private commit6,066,176B。包含诊断开销，4秒不能证明长期趋势，不与R6宣称改善。末端分位数读取排序数组不是恒定内存。源码v3 SHA`dad282f1abd9085f418569b99647e297380ef8a1fdfe9b1faf90617a74de6aa5`，工具SHA`8acc7e0745d3e9ecba818d2c12dd2a99a22442a5cf1fd42da98166f6a937f1c1`；test exe/DLL与R6相同，生产Rust未变，身份见方向原件。

**未实现/阻断**：生产部署故障控制器、不可回收Host/COM耗尽后的真实文字恢复、连续8h/24h、真实TSF和应用矩阵。工程进程回收不等于跨线程Release或生产宿主可安全终止；真实注册方案及长期测试仍须隔离环境与人工批准。

## B：任务交付、错误回退声明及评分观测

[CL-008报告](https://github.com/WanFengQC/Cloudleaf/blob/b4b5fb7f812c56f73a132cd9f16045b58a3121c2/docs/reports/cl008-round7-status.md)先行设计`8265966`、真实反例`ba17cbf`和样本预登记`cea9a7f`先于实现。错误预测的fallback_preserved原仅检查bool，rejected/budget/unsupported即使false也可被接受；现在三种错误声明必须保全原文回退，失败案例仍在冻结分母。这是评估入口缺陷，不能把布尔声明校验当生产输入结果证明。

新增B私有离线task-handoff审计，核C的13字段任务摘要/完整source binding、三协议review binding、train/dev意图/拼音/可接受输出/键轨迹、固定来源/家族和独立双审/仲裁。context_review明确none或已确认左侧上下文，所有split的公共意图组只含ID，不打开heldout答案。C v1没有这些独立context/intent-family字段，必须由真人另交，不自动生成。旧B来源审核/70:15:15/heldout至少2000家族/加密保管和参数冻结门禁保持；匹配声明不证明真人、授权或语义近重复。

研究观测新增Scoring：extend内特征/整数/模型转移及finish/EOS，排除Path克隆/文本拼接，嵌套于traversal；ContextRerank仍原阶段。默认Noop不读钟，预算/路径/排序/候选语义未改。新阶段schema2明确保留旧九阶段名，不改写R6 schema1。新增桶边界和完整结果差分继续保留Reference/ExactDag/PositionDag/Beam，不替换默认引擎。

本轮诊断预先登记24探针×12配置×10 plain/profile配对、3预热，实际2,880对/5,760被测查询；288组完整Result Debug及原字段与R6最终结果相同，0差异。保留全部预算/剪枝/拒绝、逐例P50/P95/P99、候选数、阶段和预算原件；n=10的P99不足以证明稳定尾部，不与R6的30次样本作改善或资源放行比较。阶段嵌套不能相加，读钟开销和桌面背景负载有明确限制；单算法分配/live/RSS仍未测，不能用进程峰补值。

现有生产原型排序不能把v2 unknown偷填0后称同正式数据基线；该适配尚缺。加权机制、Bigram/有限候选上下文重排已有研究实现，但真实合法训练语料/冻结模型/独立gold仍缺。三个实际正式门禁exit2，Top-1/Top-5/MRR继续null；纠错/模糊/简拼/双拼/学习/增量及生产接入未完成。

## C：三协议审核失效和逐源证据矩阵

[CL-009报告](https://github.com/WanFengQC/Cloudleaf/blob/3d573384ce1f31e437bdb4e7dc109cf438c261f4/docs/reports/cl009-round7-status.md)先设计`f2aa76f`，旧已提交实现的真实红测`65a5d75`在preprocessing/lineage两子例均为AssertionError。现审核和生效仲裁绑定注释/预处理/lineage三摘要；旧审核缺新字段明确拒绝，不自动代签。task/source摘要与八字段family-index不变，freeze manifest另记binding schema1。原生产导入/音节/格式/发布回退和123,544 runtime/1,468隔离/unknown保持。

新增source_review从八条既有登记、三个固定来源锁及许可/镜像证据核身份，再展示选定Wikipedia R6阻断快照，共9源。官方/镜像、版本、取得时间/原SHA、许可原文SHA、商业/修改/分发声明、署名、例外和处理/分词协议分列；未取得或未知字段null。Unihan/固定CC-CEDICT既有批准范围保持，Wiktionary片段research_only，SUBTLEX/wordfreq/Ngrams/Leipzig和Wiki条件未齐不升级production。矩阵不新增许可批准，也不代法律真实性审核；没有重新广搜或下载新语料。

精确archive中矩阵两构建逐字节相同，23,295B/SHA`b9a6b8425799e2b7ddaabba9585c8a883f1a9eab96650a0b527957551445cd61`；archive SHA`f180b3d0d7007114d51e6c96427120131fc1f4f8e2d08537e681c752774593ad`。旧9包文件SHA不变，runtime SHA仍`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`。本轮89 Python真实执行旧确定性构建/损坏/原子升级/回滚/用户字库保护及七点临时fixture中断恢复；不是旧全包380秒压力重跑，不是物理断电或跨版本用户词库迁移。

token比例、DF文档计数和从同源正token母观测派生Zipf的单位/分母保持；没有观测不补值。类型侧包已有entry/POS/sense/有向翻译/短语引用校验和研究源断言，**正式内容绑定审核、可信发布清单/签名升级及生产语义发布器未实现，真实语义/短语授权审读未交付**。人工12任务仍pending，实际audit2/freeze1/Wiki2，指标null。

## 原件保全、任务依赖和最终门禁

[逐方向文件与最终兼容清单](cl007-009-round7-shared/direction-changes.json)、[Git发布字节及本机原件独立核验](cl007-009-round7-shared/direction-artifacts.json)、[历史保全](cl007-009-round7-shared/preservation.json)及方向原始日志分别绑定代码/数据/工具/产物身份。展示LF和原CRLF字节分开，不能只核展示SHA后宣称原日志一致。共享106/A346/B319/C114份起点报告、旧测试和64/41/83资源保留；本轮准备错误另列，不能把链式shell整体0当某项审计成功。

依赖仍为C实际授权/任务协议→B显式适配→真人独立质量门禁；A生命周期与获批真实环境独立验收；共享只有人工合并审查准备。没有真实人员、授权、隔离环境或封存材料时继续阻断，不能靠代码制造。

|最终问题|决定|
|---|---|
|PR #1可考虑合并？|本轮已审源码和回归范围可进入人工合并审查，需最终精确push CI通过；未人工批准/自动合并，不代表方向集成通过。|
|CL-007距离真实应用还缺什么？|不可回收资源部署恢复、获批隔离环境、连续8h/24h、签名/位数/可逆注册撤销方案与真实应用矩阵；当前不放行。|
|CL-008正式语言评测条件？|不具备。缺真人合法gold、语义/近重复双审、真实上下文和heldout封存/保管及正式训练/资源门禁；指标null，默认引擎未替换。|
|CL-009正式频率/语义发布条件？|不具备。来源矩阵仅重放，现代观测/许可例外/作者/分词、人工语义内容及生产发布器仍缺。|
|剩余P0/P1？|本轮已复现的诊断/评估/审核工具缺陷已修，已审范围未发现新的未修复可复现P0/P1。A永久占用预算与真实文字恢复、B质量/资源、C合法真实数据仍是发布阻断；不能由此宣布无风险。|
|可进入CL-010？|不允许。三方向原退出条件都未满足，工程通过不替代长期、真实平台、独立语言或正式数据验收。|
