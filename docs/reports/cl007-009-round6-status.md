# Cloudleaf Round 6：独立审阅、诊断保全与数据门禁修复

2026-10-29。继续完整正式产品路线。`main`保持`a564fd47b0f054ff360592af3ccb8d744021160f`，四个PR保持开放Draft；没有注册、激活、安装或发布系统输入法，没有合并main或进入CL-010。起始状态与范围先提交于[Round 6计划](../cl007-009-round6-plan.md)，用户补充的PR #1独立审阅另先登记[审阅约定](../shared-contract-round6-review-design.md)。

本轮做了实际源码审阅、工具缺陷修复、受约束的搜索优化和原始证据交付。短诊断不替代8h/24h，源断言不替代人工语言事实，声明核验不替代权限真实性，工程CI不替代真实Windows应用验收。

## 固定身份与新CI

|方向|起始检查点|本轮被测实现|最终交付HEAD|Windows/Linux精确push CI|
|---|---|---|---|---|
|共享 / PR #1|`a1b9b0989a3bf9267919675d83f2688dd6b275a5`|`ce2c16e8f53da9913f2624675c7807d07c044877`|本报告后续提交|推送后记录于PR #1和最终交付|
|CL-007 / PR #4|`7f581dfca43515105fca918ffd79e55f315f7729`|`271704e2db80f556fec477e4523e71456e42c632`|`caefb7f2ac7c8584249c05359fcb174f87640886`|[40705631876](https://github.com/WanFengQC/Cloudleaf/actions/runs/40705631876)|
|CL-008 / PR #2|`08b09277d3beeb90e284145a3423b9f00eca8d6c`|`9673c29151380c146ae6ecb16055072cda0f0f00`|`417dad662bfebedf3238f909476240427cc9100b`|[40706957058](https://github.com/WanFengQC/Cloudleaf/actions/runs/40706957058)|
|CL-009 / PR #3|`c21db7ca70e5f51f84ca8bd30944d6722473c32c`|`cdc73515ad5356844f1df4fc3ee30194c5a15ab6`|`52619a963070cd74c53d0b8d76fc40bf5b7a8327`|[40852885211](https://github.com/WanFengQC/Cloudleaf/actions/runs/40852885211)|

开始时本地/远程检查点逐项相等且工作区干净；上次中断留下的已提交设计/测试和A未提交runner草稿先核对、保留，再继续。基线CI的完整SHA、push事件、Windows/Linux两job/全部步骤和两个未过期产物digest已重核，[原始快照](cl007-009-round6-shared/baseline-ci.json)只证明Round 5。新最终提交另核[本轮CI](cl007-009-round6-ci.json)，不沿用父提交或PR默认merge快照。

本地各方向README全套锁定依赖/offline构建、fmt、Clippy `-D warnings`、Rust单元/集成/文档、Python和资源许可检查分别执行。Windows分支另做精确依赖/RustSec快照审计；公共六crate没有外部Rust依赖，不能将Windows依赖清单或其审计外推为所有代码安全。

|方向|Rust / Python|专项与实际状态|
|---|---|---|
|共享|64 / 14|12项完整命令均exit0；新增四项公开入口独立回归在原生产代码通过|
|A|157 / 21|10项完整命令、9项release专项通过；真实短诊断返回0/130/0/1，分别符合正常/受控中断/新段续跑/注入失败预期；8h/24h未执行|
|B|79 / 66|15项命令符合预期；两项正式语言门禁实际exit2、指标null；同机旧新完整矩阵结果另列|
|C|60 / 83|新任务/源断言/统计准入专项与旧回归；实际待填任务及Wiki准入继续exit2|

## PR #1实际源码审阅

实际差异来自固定main与PR #1树、以及三方向相对其merge-base的Git差异；不是仅按文档推断。核心审阅对象为`crates/cloudleaf-input/src/lib.rs`、`crates/cloudleaf-types/src/lib.rs`，以及共享`check_workspace.py`/审计政策；设计、分支差异、关键Git blob/SHA和非破坏性merge-tree预览见[原始审阅](cl007-009-round6-shared/independent-review.json)。[第二视角](cl007-009-round6-shared/peer-review.json)为AI只读源码复核，不是外部人工批准或人工gold标注。

逐项检查了：KeyDisposition与handle的状态/错误优先级、只读预判不派发/不查询/不改变帧；select_candidate的pending/failed、revision、ID拒绝和同一提交路径；InputConfig页大小1..9、候选1..128与当前页数字选择；checked_add先于提交变更、取消后旧ID不复用；UTF-8越界/标量边界和UTF-16 code units。组合附加符和ZWJ边界不等于字素编辑能力。CommitOutcome三态由平台证明：Unknown没有核心确认事件，保持pending/隔离，不能映射成允许显式重试的CommitFailed。

新增`crates/cloudleaf-input/tests/round6_shared_review.rs`使用真实公开入口，覆盖配置、候选/编辑/焦点/模式、提交和Unicode反例；未复制排序逻辑，未修改共享生产实现。四项均在原实现通过，没有为制造红测而声称原核心有缺陷。设计`31db608`、测试`ce2c16e`及最终64 Rust/14 Python均可重放，原始命令/输出见[完整检查](cl007-009-round6-shared/validation.json)。在本轮已审范围内没有发现新的可复现共享P0/P1。

三方向实际merge-base都是`9af9bc1b2ebacc3ad76124cfa39d09de8eb77bd1`，不是当前共享head。四方向input blob均为`ae704e7affd9d267328e778b27872195755c8f9e`、types均为`2a35eec1d787deb979e096b62c834244c4723505`，所以当前核心接口内容一致，但方向没有包含全部共享收尾和本轮新回归。基线三次merge-tree预览无冲突；它只产生临时Git树，不改变任何分支，也不证明集成运行通过。

后续集成步骤：先由人审PR #1并决定合并方式，记录新main完整SHA；每方向保留原提交，分别正常合并明确的新main，核对merge-base、实际差异和所有冲突。共享接口或审计变化由共享方向先审，不靠每方向自行发明版本；冲突不删旧证据/测试。对每个整合后的新head重新运行本地全套、方向专项和Windows/Linux push CI/产物，再分别决定方向PR。当前没有执行任何main合并、rebase或强推。

## CL-007：证据丢失修复与长期准入

[方向报告](https://github.com/WanFengQC/Cloudleaf/blob/caefb7f2ac7c8584249c05359fcb174f87640886/docs/reports/cl007/round6-soak-status.md)列出完整生命周期审阅及修改文件。Factory/Service/LockServer和Lease负责模块保活；Host/Rc/COM归owner STA；watchdog全局持有JoinHandle，只有真实OS退出证明等条件齐备才回收纯资源；recovery清理权限、window/class和预算分别判断。DllCanUnloadNow仅请求零等待回收，OBJECTS/LOCKS非零仍S_FALSE。Unknown、旧instance/epoch/sequence、迟到/重复回调和FIFO保护未放宽。

旧压力控制器只支持180–600秒，事件/资源直到退出才写summary，中断会失去整轮证据。本轮设计`0f204c1`、真实缺API红测`f5ff615`后实现`271704e`。修改`stress_windows_faults.py`、`test_windows_stress.py`、工程`render_tests.rs`及仅cfg(test)模块计数；不改生产COM回收、总预算、共享接口或默认算法。

新8h/24h档固定连续28800/86400秒，默认一秒一个六键周期，不能缩短时长保留长期标签；另有明确非验收的短档。流式stdout、原始事件/资源JSONL及周期fsync检查点持续保存，最新检查点原子替换而历史仍在JSONL。续跑核对源码/数据/工具/二进制/配置身份和OS进程创建时间，只创建新segment；不按裸PID杀进程，不把两段时长合成连续通过，不重放旧Unknown。不能杀Word/浏览器等真实宿主来“恢复输入”。生产部署级隔离/代理及不可回收Host事务恢复仍未实现。

实际四短档结果：4秒正常80周期/480写、562原始事件；2.5秒受控中断exit130、176事件已保留；原身份新段又80周期/480写；第3周期注入panic，child101/runner1、22事件和原文保留。两完整段尾部各32/32材料核对成功，中断/失败未执行尾部，恢复成功率为null。所有长期完成标志false。原R3/R4/R5、共享预算和release DLL loader等9专项保留并通过，没有删除旧断言。

完整短档采样峰句柄146、线程7、working set15,605,760B、private commit6,144,000B；4秒不能证明长期趋势。480个自编事件admission P50/P95/P99为36,600/199,400/1,408,700ns，包含诊断snapshot克隆取证；确认延迟到整周期drain末，非单键真实TSF延迟。Host Rc、模块Lease/Lock、自有STA成对IUnknown AddRef/Release返回数分别标单位，外部COM总数null；均不能推断全局真实COM引用数。末尾计算分位数会读取/排序延迟数组，不声称恒定内存。

实际测试exe SHA`c7fe4b34c197871bb8d02acb37a005f93ac658800712a015a544312a7e54a0b6`，release DLL SHA`204fbb8ee725562a449ba6bcac730ab113944350e92c7a2d6ddd97fd6e73b552`，源码v3 SHA`ad8760e1fc7dc56f9af924c37095bf61aa137852462768a560a7fa15e82bf193`。[102项Git字节/30份原文](https://github.com/WanFengQC/Cloudleaf/blob/caefb7f2ac7c8584249c05359fcb174f87640886/docs/reports/cl007/round6-soak/checksums.json)另独立复核。

**长期和真实应用未放行。**8h/24h尚未启动或完成；任意异常退出/永久COM引用/锁/Host累积仍可耗尽预算。真实TSF准入还缺部署恢复策略、隔离VM/用户/快照/负责人、签名/位数/备份/撤销演练和获批注册方案。方案目前是受阻草案，未执行管理命令；记事本/浏览器/办公软件/多行编辑器、只读/密码/DPI、端到端自然输入均未验收。工程COM和DLL loader不能替代。

## CL-008：完整字段取证及稳定桶更新

[方向报告](https://github.com/WanFengQC/Cloudleaf/blob/417dad662bfebedf3238f909476240427cc9100b/docs/reports/cl008-round6/README.md)保留R5的8,640对和原失败分母。审阅R5阶段数据确认桶排序/去重占重复开销：Position traversal P95 6.8862ms，其中嵌套桶3.5045ms；不从185MiB比较宿主峰推断单算法内存。

发现阶段宿主遗漏edge_indices、finite_pool_only和完整raw_fallback，无法用原JSON逐字段证明跨版本优化等价。真实红测`aed4b29`捕获缺result_debug，`f1696fa`只在计时外补完整Result Debug。先冻结这个同源宿主的旧算法exe，再在`9673c29`将已排序unique桶的反复全排序/BTreeSet文本克隆替换为稳定有序插入：同文本取严格更优、完全相等保留先到，等序插入上界；保留遍历、扩展/驻留预算检查顺序、最终排序和全部错误/剪枝语义。修改只在研究crate和回归/报告，未接默认引擎。

同机原24探针×12配置×30配对、5预热，原数据和所有预算不变，旧新进程按配置交换次序。旧新各8,640组plain/profile配对（合计34,560次被测查询），288组跨版本完整Result Debug和原JSON字段相同，包含edge_indices、features、raw_fallback、finite_pool_only和预算统计；原R4已报告字段也相同。Reference14例/Exact6例预算失败、Beam1/8/64各16/16/15例剪枝保留，每版175组失败/预算/剪枝建索引。标准Position总体plain P95旧5.7603→新3.0970ms，但逐例仍2例P95>5ms，资源门禁未全面通过。Beam1总体P95为1.1968→1.2517ms，未改善；不会只报告有利变化。

新源码v2 SHA`c39bc6f8ca56ee59489be8af39eedef0b37b5aafe979e844706b4467b8fec456`；旧/新阶段exe SHA分别为`30f3775ff89aa210ab8cd588b303075e7e862ef0576fb399ef468bbef48f8274`和`484ef282c50e0809499ff2afbf6514b0ba80ffc35351cb2b29726d6f0816d49f`。实际同源旧算法为`f1696fa125d27507be0d328f04717b9760163417`，不是把不同取证宿主的R5时间直接当本次前测。完整122项Git字节、43原始日志和7结构化原件另独立复核。

桶语义回归在旧算法本来就通过，性能优化不是虚构的正确性bug；真正红测对应完整取证字段缺口。没有为分配测量引入unsafe allocator或绕过workspace forbid；线程分配流量/live/RSS均未测，进程峰不能补作这些指标。阶段嵌套不能相加，观测耗时包含读钟/记录开销；桌面背景负载未控制。

独立人工gold、来源家族语义/近重复审阅、双审/仲裁、封存heldout、现代频率及正式模型仍未交付，两正式门禁继续实际exit2，Top-1/Top-5/MRR为null。C的八字段family index不能自动满足B任务摘要、来源审核和封存门禁。没有用待测词典生成独立答案，没有heldout调参；纠错/模糊音/简拼/双拼/学习/增量和生产引擎替换仍未完成。

## CL-009：人工工作流与真实源断言

[方向报告](https://github.com/WanFengQC/Cloudleaf/blob/52619a963070cd74c53d0b8d76fc40bf5b7a8327/docs/reports/cl009-round6.md)记录新离线`annotation_workflow.py`、`source_assertions.py`、`corpus_gate.py`及测试/协议。生产123,544 runtime、1,468隔离、音节规则、schema2/3、TSV v2和发布/恢复实现未变；token比例、DF、Zipf和unknown/null原语义继续，未伪造现代词频。

人工工作流交付12个真实待填任务槽及可执行审核/冻结工具，要求意图、拼音、可接受输出、按键轨迹、场景/来源与授权，双审独立于作者，分歧需独立仲裁；全局family/near、lineage+unit、规范输入/意图隔离，split由保管者明确分配。当前用户没有提供实际人员、授权、答案或封存材料，12任务均pending，实际冻结拒绝；不把工具/fixture当人工完成。

审阅中修复三项P1门禁/取证缺口，均先保留失败：替换协议仍沿用审核；同source_id更换原件/授权仍沿用审核；公共packet再次包含heldout答案。现在审核/仲裁绑定协议和完整来源记录摘要，来源变更使审核失效；公共packet只列来源/协议/ID，答案仅分split文件。未提交草稿的失败同时保存完整工具字节和SHA，不伪装已提交旧生产。文件分开不等于加密或访问控制，真实保管者和正式封存仍缺。Wiki仅四串SHA可伪放行的问题也通过实际证据文件/结构/revision、作者历史和提取/分词协议校验修复；声明核验仍不证明真人权限。

使用既有批准研究范围的Wiktionary固定片段（46,788,375B/66,593完整行，SHA`ca4df3453261f9f12f1223deb11a26d979078ea6e1af870b44da2d3dcf4e2998`），严格继续2–16字Han及明确Mandarin/Pinyin筛选。导出原POS、源义项、显式英语翻译、原行/JSON pointer及研究lookup，不猜义项对齐；英语反查引用同一有向源断言，不新造反向equivalent。两次归档隔离构建的5产物字节一致：591研究词条、887源义项、241英语翻译声明，另18条权利标记隔离、111缺明确拼音、65,873范围外；源行/位置逐项核对。不是将全部1,328个中文源行无条件导出。断言JSONL为626,590B/SHA`1979ad15a351339edfccd492c154b41007f15f8fc590d4ec8ca53b199795430b`，lookup SHA`5403311f125de0e8468576519bcced614d2785084cbfb09f856310558806e04b`；精确源码归档SHA`d15e239ae3e30506612fdcf790d9a481977d042b94630c063b021364a67dbe47`。

该片段完整gzip/CRC和所有页面例外未核，research_only不变，不称正式完整双语词库，不满足生产sidecar双审。Wikipedia只对已选自然数page342/revision94816134取得官方原件；原件SHA、时间/内容模型/正文SHA1核对，发现模板、引用、媒体和quote风险，原文留本地不随Git分发。页例外/完整作者/提取/分词未齐，实际准入exit2，token/DF/Zipf均null。没有重新广搜或下载3.4GB dump。

B/C显式映射记录了case/source/unit/family/near/split/normalized_input/task字段与来源、预处理、标注协议、review和封存缺项；相同字段形状不证明摘要语义兼容，更不解除B原70/15/15和heldout至少2000家族等门禁。真实POS/稳定义项/双向翻译事实、短语及授权人工gold仍未交付。旧正式包和回退机制只读校验及旧全套回归保留；没有重复无关的7点压力来扩大成绩，也没有把已有子进程测试冒充物理断电。

## 证据保全、风险与最终六项决策

[具体修改文件](cl007-009-round6-shared/direction-changes.json)、[独立产物复核](cl007-009-round6-shared/direction-artifacts.json)、[历史保全](cl007-009-round6-shared/preservation.json)、[原始完整命令](cl007-009-round6-shared/validation.json)及各方向原始日志分别绑定实现/数据/工具/二进制身份。Windows CRLF原件与Git LF展示件有双SHA/无损封装，不以展示件冒充原字节。共享75/A242/B196/C79份起点历史报告与64/41/83资源保留，没有删除旧测试/失败或改写R1–5。

|问题|本轮最终决策|
|---|---|
|1. PR #1可考虑合并？|在实际共享源码无新可复现P0/P1、64 Rust/14 Python及最终精确push CI均通过的限定范围内，可进入人工合并审查；没有自动合并。三个方向旧merge-base和后续新CI仍须处理，不能称集成通过。|
|2. CL-007还缺什么？|连续8h/24h完整证据、不可回收Host/COM耗尽的部署策略、隔离Windows环境与负责人/快照/可逆撤销、人工批准注册方案，以及真实TSF/应用兼容和长期输入；当前准入阻断。|
|3. CL-008正式质量条件？|不具备。缺合法独立人工gold、真实双审/仲裁/全局语义泄漏审核、封存与保管者、现代模型/统计及正式资源门禁；语言指标null，默认核心不替换。|
|4. CL-009正式词频/语义发布？|不具备。研究源断言和待填工作流不是生产词频或语义发布；页例外/作者/分词、真实观测和人员授权/注释仍缺，旧unknown不补值。|
|5. 剩余P0/P1？|本轮已复现的取证/门禁问题已修复，已审范围内未发现新的未修复可复现P0/P1。仍有发布级阻断：A任意异常/永久引用导致不可回收预算耗尽和宿主事务恢复；B独立质量/资源完整性不足；C权限/语义人工真实性与封存未交付。不能把“未发现新缺陷”当这些门禁通过。|
|6. 可进入CL-010？|不允许。A/B/C各自退出条件均未满足，完整产品、Windows系统交互、语言质量和正式数据尚未验收。|

共享只提交审阅回归、计划/汇总/证据与路线入口，不擅自合并方向代码。后续实施先解决上述有事实依据的阻断；8h/24h必须真实连续完成才报告通过，真实系统注册/激活/安装仍需单独获批。四个PR维持Draft。
