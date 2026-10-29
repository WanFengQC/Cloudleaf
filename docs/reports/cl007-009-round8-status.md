# Cloudleaf Round 8：外部交接与固定候选原件落地

2026-10-30。[先行计划](../cl007-009-round8-plan.md)提交于`b5d78d0`，取得范围修订先提交于`f907455`。本轮完成可交给外部人员的具体材料、新离线报告器及其缺陷修复、固定统计候选原件完整取得。没有用新增工程测试数量替代长期、语言或真实平台验收。main仍为`a564fd47b0f054ff360592af3ccb8d744021160f`；四PR保持开放Draft，无合并、强推、TSF操作或CL-010。

## 精确交付与验证边界

|方向|R7实际起点|R8最终方向HEAD|新push CI|
|---|---|---|---|
|共享/PR #1|`e359e471a66599f0fa4fdc03dd9d802a242ebc3d`|本报告后续提交；生产接口未改|推送后独立核验，记录在PR #1及最终凭据|
|A/PR #4|`fa4f1fab93394f115d3a987a7f0201b8662c08cf`|`333dc90e0985a13f75629e29599f3df1d7d951ab`|[40915423624](https://github.com/WanFengQC/Cloudleaf/actions/runs/40915423624)|
|B/PR #2|`b4b5fb7f812c56f73a132cd9f16045b58a3121c2`|`f991a8128cd07d4af57eb773cc2e598cf7e5809a`|[40912373247](https://github.com/WanFengQC/Cloudleaf/actions/runs/40912373247)|
|C/PR #3|`3d573384ce1f31e437bdb4e7dc109cf438c261f4`|`ef6cf16c558f5521cd776e4805068a07f15aedd2`|[40915671708](https://github.com/WanFengQC/Cloudleaf/actions/runs/40915671708)|

各run的实际结论、完整head、push事件、Windows/Linux全部步骤、两个未过期产物及digest见[本轮方向CI原响应](cl007-009-round8-shared/direction-ci.json)。共享包含自身的最终SHA/完整diff及CI在推送后生成`.local/round8-final-receipt.json`并更新PR #1，不伪造自引用提交。R7成功另外存[基线](../reviews/pr1-round8-evidence/baseline-ci.json)，不转绑R8。

A/B/C表列新最终head均已逐项核验两平台成功，各有两个精确head的未过期产物。A旧失败run独立保留，不修改原结论；共享最终CI仍须本报告提交后单独核验。

|方向|实际本地检查|未重复或不代表的事项|
|---|---|---|
|共享|生产/公开测试/资源身份、人工模板/链接/方向兼容、fmt/资源审计/diff|文档及待填模板没有新增核心逻辑，不重跑未变R7本地64 Rust/14 Python和性能；新CI仍执行完整工程流程|
|A|最终31 Python全回归、审计/fmt/只读旧迹报告与覆盖拒绝；真实红测与旧CI失败保留|未运行新的native host/loader/采样、8h/24h或真实应用；未变Rust/COM/runner/预算|
|B|五项命令实际返回2/2/2/0/0；三正式门禁继续拒绝|没有新算法/性能矩阵/人工gold；正式指标null|
|C|第一阶段六项0/0/0/0/2/0；追加取得后四项0/0/0/0，失败尝试另存|无生产parser/新统计值/训练或发布；未重跑未变全包构建压力，未做物理断电|

命令、返回码、原文和源码/工具/数据/既有二进制身份在各方向报告及[独立字节审计](cl007-009-round8-shared/direction-artifacts.json)。未删除旧测试、放宽断言或资源预算。CI属于工程验证。

## R8-01：共享契约可交人工审阅，尚无人工决定

[最终人工清单](../reviews/pr1-round8-human-review.md)有14项未勾选检查和[待填决定模板](../reviews/pr1-round8-decision-template.json)。实际读代码、完整差异和既有真实公开API测试，覆盖只读预判/重复按键、错误状态、候选revision/ID、三态提交、UTF-16标量边界、分页和精确依赖政策。R8不改共享生产代码；AI独立复核未发现清单与实现矛盾，不能冒充真人签署或合并批准。

重复字符每次消费；core事务ID只在同引擎内唯一，平台仍负责session/epoch。Unknown不发可重试失败、不自动重放；取消core不能证明文档没有写入。导航边界帧未变则revision仍有效；UTF-16辅助不是通用字素编辑。共享0外部依赖的政策不替代A独立15依赖审计。完整142项main→R7共享目录/2,990,191B patch身份见[技术证据](../reviews/pr1-round8-evidence/review.json)，R8最终完整目录另存最终凭据。

三方向merge-base仍`9af9bc1b2ebacc3ad76124cfa39d09de8eb77bd1`；input/types Git blob一致只说明当前内容相容。没有擅自改基底/合方向代码。只有用户实际人工合并PR #1后，才fetch实际新main、逐方向普通merge、审冲突和原测试、重跑新head专项及两平台CI。精确步骤和风险在人工清单，不以mergeable或预览替代集成。

## R8-02：可复核资源报告和隔离验收交接

[A最终报告](https://github.com/WanFengQC/Cloudleaf/blob/333dc90e0985a13f75629e29599f3df1d7d951ab/docs/reports/cl007-round8-status.md)与设计`7044ce6`交付`report_windows_soak.py`、必要真实实现测试、环境JSON、部署/恢复条件和9项真实应用矩阵。只读重放R7原短迹，分段核原件/检查点/身份/时间，描述句柄、线程、working set/private commit首末/峰值/采样间隔/OLS每小时斜率及原延迟和恢复分母，不自动裁定长期合格，不拼接中断段。

本轮新工具发现并修复：缓存summary不能覆盖原始final持续时长；缺final的中断段持续时间应null。四段实际native时长仍4.0329512/null/4.0284962/null秒，资源合格门禁null，8h/24h完成false，非新运行结果。source-v3最终SHA`0dc201a4aa9e70263639b14b317c9c9f12782472a2860eb1937dbe950c623b57`；reporter SHA`2b48f22e4d37cbd0bb40298067b6a4058b6f78f80a46d80e51e8b64040d936c7`。旧exe/DLL身份保留且本轮未执行；短迹斜率不能推断长期泄漏/合格。

中间head`05dd085`的[真实push失败40913130729](https://github.com/WanFengQC/Cloudleaf/actions/runs/40913130729)为Linux成功、Windows证据测试两失败。原94,499B日志和失败分母保留。Windows checkout将三个JSONL转CRLF，新reporter按物理SHA误拒；先提交CRLF红测`ffe03ee`（6测1fail），实现修复`8d955a5`仅对固定三JSONL、且CRLF→LF后**精确匹配原expected SHA**才解析；仍分别记录物理/重建身份，任意内容变动不接纳。最终31测通过。旧R8全部37证据文件保持，不把旧失败改标通过。

用户已明确环境/VM/系统版本/架构、快照及实际还原证明、负责人、执行/TSF授权都未准备。环境JSON和9应用实际字段保持空，8h/24h运行ID未生成。部署故障隔离、永久占用预算后的真实文字恢复、连续长测、可逆注册方案和真实应用结果仍属退出条件；当前不启动长测、不注册或激活。

## R8-03：真实人员可操作的gold交接

[B最终报告](https://github.com/WanFengQC/Cloudleaf/blob/f991a8128cd07d4af57eb773cc2e598cf7e5809a/docs/reports/cl008-round8-status.md)和设计`ce5d2ea`交付操作README、13项缺件清单、上下文/意图家族及handoff待填模板。角色授权唯一主表位于C的`data/plans/round8-external-handoff.json`，绑定C提交`3d02431`及blob SHA`bd0a09e748289f79d489d6eea9b88f88135542201144381e483d797901c45539`，C最终该内容未变。角色名不等于真实人员，模板不允许直接当已批准packet。

真实作者→两名独立reviewer→必要仲裁；heldout只在独立保管者私有环境冻结/封存，需真实加密、key/访问批准及审计。C原任务13字段/三协议绑定、来源家族/近重复合同、B实际context/intent-family审阅、旧70/15/15与heldout至少2000家族门禁保留。不从待测词典生成答案，不向开发环境打开heldout。

现12任务仍pending，5角色及授权空、13交付缺件pending。三个既有正式门禁真实exit2；Top-1/Top-5/MRR仍null。研究搜索/排序、R5/R7配对原件不改，不换默认引擎。真实训练语料、独立gold、单算法资源证据、纠错/模糊/简拼/双拼/学习/增量及生产接入仍未完成。

## R8-04：具体许可与候选原件取得，生产采用未放行

[C最终报告](https://github.com/WanFengQC/Cloudleaf/blob/ef6cf16c558f5521cd776e4805068a07f15aedd2/docs/reports/cl009-round8-status.md)包含唯一角色/授权主入口（5角色null、9材料pending）、20项真实CC-CEDICT原义项待审队列、署名/完整CC BY-SA4许可、逐源采用结论和生产语义发布器设计。逐条对原zip源行/事实包SHA，不编POS、对齐、反向翻译或人工审核；该队列是现已准范围原义项的审读实物，**不是独立IME gold**。

既有Unihan/CC-CEDICT缓存绝对路径和完整SHA/CRC已核，授权范围与语义质量分开。Google Books Chinese2019沿用既有候选，精准核[官方具体索引](https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/chi_sim-1-ngrams_exports.html)的CC BY3编译许可声明和[官方条款](https://creativecommons.org/licenses/by/3.0/legalcode.en)，不以一般FAQ猜bulk权利，不授权原书全文。

先保存metadata-only检查点`0f65339`，再先提交有界取得设计`fc539e9`：全新Git外目录、仅两个确定URL、72MiB压缩/1GiB解压上限、超时、失败partial保留、不重试。实际65.047秒完整取得：

|数据身份|实际字节 / SHA-256|
|---|---|
|固定`20200217/chi_sim/1-00000-of-00001.gz`|60,833,224 / `6ab29862a8c904c261bb9d5a02a342285c4dbabf79cee5c4f8818076fd44fb86`|
|同版`totalcounts-1`|7,479 / `f4a136b63419c0f34dfbfcf76f501ac80a745ad535f5cc9bda2666189a35e9f3`|
|gzip解压完整内容|195,878,750 / `f5535ee7d0c0a5ba18bff3b4019d150da0e63265b8e2abdebb2d442db45b95f0`|

HTTP200、长度一致、SHA完成；gzip读EOF完成CRC/ISIZE和严格UTF-8，508,674行。实际每行词头后为多组三个十进制字段，共19,074,163组，tab字段2–457；不是v2四列逐年格式。totalcounts464条四字段记录。这关闭原件身份/完整性/实际形状缺口，**尚未批准字段语义、分词、时期/截断、分母或生产采用**。未计算产品token/DF/Zipf，词条123,544及1,468隔离/unknown完全未改，来源登记/批准锁不变。Ngrams仍candidate-not-approved，Wiktionary仍research_only。

第一阶段6命令与原18份文件/21项Git清单原样保留；追加四检查全部0。追加环境PATH缺cargo审计1、文档EOF空行diff2均保留原始失败和修正前源码/字节，修正准备流程后通过，未改生产或审计规则。没有伪造取得stdout字节：取得JSON为实际执行记录，终端输出仅工具转录；追加验证stdout/stderr另无损封装。

发布器当前仅设计：版本化POS/义项/有向关系内容、三协议与真实双审/仲裁绑定、可信签署根及manifest，复用原guard/不可变release/单指针/previous/recover，保护用户字库，兼容旧reader。新bundle verifier/reader/生产发布器/签署部署均未实现；真实语义审查、统计口径及正式内容不具备，不能发布。

## 保全、依赖与最终决策

[最终文件/兼容/历史清单](cl007-009-round8-shared/direction-changes.json)和独立审计逐项核Git LF发布与原始UTF-8封装/本机字节；共享113/A465/B398/C153份R7起点报告不改，旧测试和64/41/83原型资源保留。准备和审计自身失败也单列，不把成功命令覆盖失败分母。数据/工具/源码/产物分别记录，无新二进制性能宣称。

依赖是实际环境/快照恢复/负责人授权→长测→可靠性准入→另批TSF/真实应用；真实角色/授权→来源协议/语义双审及独立gold→封存→正式质量/资源→生产集成。共享人工决定不代替任何这些步骤。

|最终问题|本轮结论|
|---|---|
|PR #1达到可考虑合并的审阅条件？|技术材料已齐，可提交真人审查；最终共享head须有自己的两平台push CI。尚无真人签署/合并批准，不自动合并。|
|CL-007距真实应用验收？|获批可还原隔离环境、不可回收资源实际部署恢复、各自连续8h/24h完整证据、可靠性准入及另批可逆TSF/真实应用验收；不放行。|
|CL-008具备正式独立语言评测？|不具备。实际人员/授权/gold/近重复与上下文审读、heldout独立封存及真实训练/资源条件缺失；指标null，不换默认核心。|
|CL-009具备正式词频/语义发布？|不具备。候选原件完整性已落地，但统计/分词/采用、真实语义审查和生产发布器仍缺；unknown保持。|
|剩余P0/P1？|本轮实际复现的新诊断报告器缺陷已修，已审改动未新增未修复可复现P0/P1；生产不可回收资源/真实文字恢复、语言质量及正式数据条件仍是发布阻断，未执行条件不宣称无风险。|
|允许CL-010？|不允许。A/B/C原退出门禁仍未满足。|

下一步交给负责人落实外部环境和真实人员；针对已固定Google原件审定统计/分词及采用协议、完成20项真实语义审读。已有pending模板不得被当作授权或验收结果；后续工程仅为这些具体外部流程及可复现缺陷服务。
