# Round19：用户词库、本地选择学习与简拼预算修复

2026-11-13。结论 **partial，不放行正式产品，CL-010关闭**。真实存储、组合索引、候选来源、默认关闭的学习和管理窗口已经实现；原简拼预算失败显著减少，但延迟未改善。窗口只完成部分实际操作，用户已明确暂停桌面自动化，不能将逻辑测试当作剩余窗口验收。

分支`codex/input-round14`，基线`cf54c84d400b627dfa9b246b3800a82d65f061b8`。主实现`1190d0e4becb13d0e1be87ed7fc90ae4d8fc25ba`，面板焦点修复`84187386ea36e8fdb3cf88300c0d35aa86c91cb6`，Linux锁释放修复 **`e0d5cb26d9e173f6dd4999ec7628cc30e7394e11`**。最终报告提交完整HEAD、精确push CI、下载产物及报告内容哈希放在Git外`.local/round19-delivery-receipt.json`；不能用1190的旧CI冒称后续提交通过。

main保持`a564fd47b0f054ff360592af3ccb8d744021160f`，五个PR保留原分支、开放Draft；不创建PR6、不合并、不强推、不修改翻译、基础数据与历史证据，不注册/安装/激活/卸载TSF，不运行8h/24h。

## 设计先行与实际代码

`8dd3505`先约定存储/学习/候选，`d334f45`登记完整后缀界，`47ec379`依据118497320字节基础索引估算改为共享基础，`9c94c25`保存缺失用户层API反例，`6aba554`保存缺失输入来源/快照API反例；`d9433f7`记录实际面板焦点问题，`5691d8d`记录Linux CI锁失败。完整约定见[设计](../input-round19-design.md)，全部原始失败及命令见[证据索引](input-round19-evidence/index.json)。

| 文件 | 实际生产变化 |
|---|---|
| `crates/cloudleaf-user/src/lib.rs`、`store.rs` | 独立UserDictionary、CRUD、启停、版本化编解码、统计、导入/导出、OS写锁、代际保存、恢复、迁移、清除与故障注入。 |
| `crates/cloudleaf-search/src/layer.rs`、`lexicon.rs`、`lib.rs` | 基础entry/node数组Arc共享，用户尾段与复制发生变化的Trie节点；基础索引不重排，用户词进入原格。 |
| `crates/cloudleaf-search/src/search.rs`、`character.rs` | 独立偏好特征和完整全序；新图字符Exact稀疏精确后缀最佳优先，缓存重绑；Reference/Beam与默认关闭旧字符搜索保留。 |
| `crates/cloudleaf-search/src/model.rs` | 模型快照Arc共享；同基础用户层只重编译entry字符缓存，原始统计和来源元数据保留。 |
| `crates/cloudleaf-input/src/lib.rs`、`sentence.rs` | 原子资源预检/替换、用户状态身份、pending selection来源与明确重试、词典/模型绑定；普通成功事件不更新统计。 |
| `adapters/windows/user_worker.rs`、`user_panel.rs`、`isolated_input.rs`、`session.rs` | 单有界worker、真实管理面板、当前用户目录、owned文档Applied入口、旧会话取消、容量计费和双引擎发布；沿用候选窗口。 |
| Cargo清单/锁、`config/audits/user-dictionary.json`、CI、README | 新本地crate与现有ring；既有windows0.62.2只增加Shell功能。未引入第三方输入法核心或网络服务。 |
| 新增Rust测试 | 用户层、来源、重试、偏好、900展开反例、实际worker链路、存储/锁/恢复，保留全部旧测试。 |

## 用户文件、ID与真实管理

用户词与基础CC-CEDICT/Unihan独立，来源固定`user`，私有标识`LicenseRef-User-Private`不代表公开分发授权。每个(text,pinyin)读音对一个记录，允许同词多读音，ID为档案内单调`user:N`，修改保留ID增加revision，普通删除/清空词条不重用ID。显式清空一个损坏档案可建立新档案，须失效旧会话；不能把它说成原档案内ID从未重置。

实际格式为UTF-8、LF的schema2 TSV包络，头`# cloudleaf-user-v2`，记录generation、next_id、learning、learning_epoch、purge；entry表为`id/revision/enabled/text/pinyin`，独立preferences表为`id/fingerprint/count`，末行SHA256覆盖前面完整字节。schema1有实际迁移fixture，未来版本、CRLF、BOM、无效UTF-8、坏校验、重复ID/读音对、非法音节和超量记录明确拒绝。

上限：4096手工记录，每条1–16汉字、对应1–16合法音节，最长64拼音字母/80拼音字节；文件8MiB、偏好10000项、计数1000000。当前不是任意混合文字的用户词头编辑器。入口验证后修改事务副本，失败不改变原内存。重复导入自身可无变化；同ID异内容或同读音对异ID报冲突，不能静默覆盖。导入只合并词条，不替用户开启导入档案中的学习。

管理操作已通过实际存储和worker回归：增/改/删/启停、查询、清空、保存、加载、导入/导出、明确恢复、清学习后重启。导出采用create_new，已有文件不覆盖；导入与本地修改冲突有反例。**这些工程回归不等于每个按钮都已完成实际窗口验收。**

保存目录由Windows当前用户`FOLDERID_LocalAppData`取得，返回字符串按要求释放；依据[微软接口说明](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath)。默认`LocalAppData/Cloudleaf/user-dictionary`，本轮实际窗口显式使用`Cloudleaf/Research/Round19-A/user-dictionary`独立研发档案。文件明文、不上传、不进入Credential Manager、日志/Git/CI不包含私人词库。API和worker都不采集输入全文、聊天、窗口标题或应用名。

## 保存、崩溃一致性与恢复

Store使用Rust1.90非阻塞OS写锁，同一档案第二写者明确Busy。固定应用文件名、绝对路径、容量、上级路径和符号链接/reparse校验均有测试。写流程为create_new pending→写入→sync_all→重读校验→rename到未存在的更高generation→目录同步/历史清理；从不先覆盖唯一有效代。

正常保留当前/前代；最新损坏或不兼容返回RecoveryRequired，不当作空库。候选旧代经校验列出，只有明确恢复才写更高代并保留原件；较新的有效代存在时拒绝回退。迁移原件保留。保存前失败不清空内存；发布后未确认返回Unknown，需reload核对，禁止旧代盲重试。基础输入不依赖用户文件成功。

删词、改词、清统计及清空使用持久purge意图，成功保存清理应用管理旧代/临时副本；发布后中断，启动完成清理，避免从旧备份静默复活。外部明确导出的文件由用户管理，不自动删除；不承诺SSD物理擦除。Unix执行目录同步；Windows未证明物理断电时的目录持久性，**子进程中断不当作断电**。

实际故障覆盖：不存在/空/坏文件、坏备份、首存半写、BeforeWrite/PartialWrite/AfterSync/AfterPublish、StorageFull、旧版/未来版、重复/无效/超大、写者竞争、子进程中断与锁释放、导入冲突、删除和清学习后的重启。StorageFull是实际半写后注入同类I/O错误，不是填满机器磁盘。

最新Linux CI暴露过一次额外失败：8418738的`real_crud_save_restart_import_export_and_private_backups`在关闭/立即重开处得到Busy；1190曾通过不能证明其稳定。正常Store析构改为显式unlock再关文件，新增保留重复描述仍应释放所有权的回归，保留原失败全文和新提交CI。没有给open加重试、没有串行化测试或删除断言。

## 完整候选、快照与128MiB预算

用户词进入原字母Trie/音节交集和全输入词条格，参与全拼、简拼/混合、小鹤、模糊/纠错及多词/整句路径；没有追加在Top5后。原123544条基础词典、原ID/索引/来源、frequency None均保留，127354条补层仍是研究快照。

基础索引已约118.5MB，因此共享基础entry/node数组，用户层只追加entry尾段及复制改变的Trie节点。`entries()`变为具名只读集合，保留索引/iter/len访问；原context的二分查找改为按ID检查，不能假设追加集合全局排序。默认用户层关闭仍用原快照/候选身份，不重建索引。

构建、文件读取、模型重绑在一个后台worker；命令队列8、结果队列2，不每次选词开线程。UI只try_send和轮询。快照身份绑定基础SHA及完整逻辑用户包（generation/revision/learning_epoch/记录/统计），磁盘transport SHA另记，不将其混作同一个哈希。字符模型保持原计数、转移、UNK和来源元数据，只按新词条列表重新编译缓存与Lexicon身份；Lexical等模型用Arc共享，不套错原条目统计。

预读/复制用户文件、当前worker字典、构建中的新层和仍引用的旧层分别预留现有账本；新增selection来源也增加Host计费。新快照帧/诊断超过原Host计费上限时拒绝发布，不扩大原128MiB门禁。STA共同预检两个引擎的pending/failed/Unknown、配置、模型和revision再替换，旧Arc安全持有、旧候选失效，原文/光标保留。保存成功但事务未决会明确显示“已保存，待事务核对后发布”，不能声称已立即生效。

这些是保守内部容量估算，不是RSS硬限制。4096层的裸搜索资源结果不能替代完整Windows Host准入；超限仍明确拒绝并保留基础输入。大层实际管理窗口发布未验收。

## 学习默认OFF、可信Applied与实验排序

新档案默认OFF。InputEngine只在明确Space/Enter/数字/鼠标选择候选时保存SelectionEvidence，绑定commit、选择revision、候选ID、词典/模型/方案与真实path条目ID/指纹；用现成entry_index取得来源，避免逐边扫描123544条。原文、标点合并/取消/失焦回退不产生该凭据。

普通`CommitSucceeded`只收尾清组合，不写统计。隔离Window先实际写本进程EDIT并读回核对，只有Applied才派发学习；Unknown/NotApplied不派发。benchmark和构造的Simulation被拒绝。明确重试保留原选择凭据并换新commit ID，旧确认拒绝；新会话/关闭有独立取消epoch。低层来源枚举不是安全授权能力，宿主真实确认入口才是边界；真实TSF入口本轮未接通。

事件使用进程/新会话/commit token，内存保留256个近期token，重复/旧generation/旧epoch拒绝。token不持久保存，重启使用新会话身份，不按文本反猜。删改/禁用/导入提升epoch，并核对user条目当前指纹，旧排队事件不能复活已删除统计。学习关闭/清除阻止旧队列；繁忙或队列满明确显示本次学习未记录，已经Applied的文字绝不重试。

偏好独立于frequency、结构和语言概率：比较解释成本→user_preference_units→原分数/lexical/ID/edge。手工用户词有1单位明确偏好；计数不足2无学习优势，之后floor(log2(count))最多8，计数饱和1000000。既有字段不覆盖，精确解释仍先于模糊/纠错，清学习恢复同一手工词库下原排序。此为显式研发规则，不宣称权重最优；关闭学习停止更新，已有手工词/偏好不自动删除，可单独清除统计。

实际逻辑测试证明统计影响排序、清除恢复原Candidate对象、多词路径/字符模型完整Reference一致，真实worker保存/重启/来源拒绝与删除链路也通过；**构造的Isolated来源标签仍只是逻辑测试，不冒充真实窗口Applied学习。**

## 简拼：90/837降至6/837，延迟仍未放行

原30个唯一×3预算失败全部保留。简单成本上限原型仅修复8/30，未采用。最终新图字符Exact使用按(position,last character,剩余编辑)的稀疏精确后缀界，完整比较解释成本、偏好、score、条目索引和edge；已观测转移与未观测公共基线分别处理，避免历史×边的笛卡尔展开。标签扫描、边、行与探测均计原预算，取消和原文回退保留。

同一123544词典、原全部837最终查询、K10、200000展开/50000驻留、三轮：**90→6预算失败，28/30唯一失败修复**。只剩简拼两字符模型的`n×32`每轮各一次，失败种类从Expansions变为ResidentPaths，未清零，无新增失败输入。过程rc0不当作所有查询成功。

新方案33480完整Reference / **83700完整Candidate比较**，用户层与偏好41850 / **104625**，均0差异/0不完整，合计188325；PositionDag上下文16740/20925明确不支持，不计成功。严格比较包含edge/来源、成本、偏好和所有Features，没有弱化为只比较正文。旧不完整Reference不作为正确答案。

固定20最终查询配对，每条件60样本，36进程/720查询；270组双方完整Candidate相同，Full180组统计也相同，仅删除新增严格零偏好字段。完整来源、初轮失败及分母见[搜索专项](input-round19-evidence/search/delivery-finish/summary.md)。

| 最终查询（格+搜索） | 旧P50/P95/P99/max ms | 新P50/P95/P99/max ms |
|---|---|---|
| 简拼Lexical |6.9366 /27.6448 /34.5205 /34.5205|11.1998 /40.8496 /113.3934 /113.3934|
| 简拼add-one |16.8544 /24.3197 /25.9239 /25.9239|9.9718 /51.8392 /84.5671 /84.5671|
| 简拼Witten–Bell |18.0153 /30.8579 /62.4605 /62.4605|10.8976 /51.9204 /60.8732 /60.8732|

**预算完整性改善不等于提速。** 新字符搜索处理了旧版预算放弃的完整路径，不能拿其失败时间作成功时延；Lexical也暴露退化。独立7轮归因没有证明共享索引是原因，不能随意归因。普通桌面并发条件有噪声，全部长尾保留，简拼延迟修复仍partial，不能为达5ms提高预算、减K、缩词典或剔除案例。

## 用户层规模与默认Engine/Host性能

OFF/空/4/512/4096合成词条×3模型，15新进程，各279个最终查询，共4185；5方案、固定原20/13输入、每进程三轮，ID升序、seed0。合成读音只构造索引歧义负载，不声称是真实语言读音，未进入基础数据。每层只有原6失败，合计30，无新增失败输入；OFF与空层837个完整SearchResult相同。新增词无公开词频。

增量估算：空1224B、4词503136B、512词935704B、4096词3995466B；4096时共127640条，原基础不变。简拼P95随规模/模型约24.79–69.25ms，4096层小鹤字符P95达11.22/8.45ms，不能宣称零开销。每方案/模型/规模P50/P95/P99/max、预算、源码/输入/二进制SHA见[规模实测](input-round19-evidence/search/delivery-finish/user-layer-observations.md)及相邻原始日志。

另外测默认用户层关闭的真正 **Engine K10逐键**：同一171事件×每版本每模型5新进程，交替旧新，合计5130事件；15组完整稳定语义一致，只有EVENT时钟及严格零新偏好字段归一，不去掉正文/ID/评分/来源/统计。旧二进制来自精确cf54的CI ZIP/member SHA验证，无warmup剔除。

| 默认Engine K10 | n | P50/P95/P99/max ms |
|---|---:|---|
| R18 Lexical |855|0.7142 /3.5406 /5.1016 /8.5387|
| R19 Lexical |855|0.6861 /3.6303 /5.0104 /6.2172|
| R18 add-one |855|0.6553 /5.2241 /11.8447 /16.7537|
| R19 add-one |855|0.6670 /5.2034 /11.0996 /15.1255|
| R18 Witten–Bell |855|0.7153 /4.9312 /12.8144 /15.4050|
| R19 Witten–Bell |855|0.7264 /5.1823 /10.8990 /14.1390|

默认P95变化+2.53%/-0.40%/+5.09%，两字符模型仍超过5ms。K5真实双引擎无HWND Host另测4条件各513事件，总2052，全部rc0；total P50/P95/P99/max为Lexical0.8928/3.7353/5.7476/9.8789ms，add-one1.1489/7.7418/13.4384/25.8784，Witten1.1525/7.3416/13.0478/20.9661，研究补层add-one1.6411/14.1440/22.7151/35.5062。完整分阶段见[Engine/Host实测](input-round19-evidence/engine-host-performance/handoff.md)。

42进程均rc0，资源50ms采样包括原件/预载索引/模型/查询/输出，working约184–216MiB、private约205–217MiB；用户层查询进程最高约235/238MiB。不是单算法分配或128MiB内部账本保证，不是TSF端到端，也不将默认关闭的Engine数据冒充用户层开启的逐键指标。

## 真实窗口：部分通过，按用户要求暂停

初版owned管理窗在自动化中仅随父窗返回，字段设置误落恢复输入框，未修改词库；保留原轨迹。修复为应用显式管理的独立顶层窗口，独立焦点与正常IME；UserState析构关闭面板/worker，原候选窗口和自有提交文档不改。

实际新窗口二进制`cb751c039598c1738f7159638c6b300c0b2bf80919ffd33b4761cb13ac4d07aa`、源码8418738，档案Round19-A：

| 操作 | 真实观察 |
|---|---|
| 打开管理面板 | 新档案学习关、generation0、0词。 |
| 填写“云笺 / yun jian”并添加 | user:1、启用、revision1、选择0，generation1保存；不是Mock数据库。 |
| 用户明确点击开启学习 | generation2、学习开；未把默认状态说成开启。 |
| 实际全拼`yunjian` | 同一候选窗首选显示云笺。 |
| 实际简拼`yj` | 同一完整搜索首选显示云笺。 |
| Space/连续键入尝试 | 遇外部焦点切换，原拼音进入恢复区，未将其计作Applied成功或学习。 |

随后快照明确出现外部窗口，停止发送按键。用户选择 **“先暂停窗口自动化，继续代码和CI”**。因此真实Applied学习计数、重启后实际窗口加载、OFF后再次选择、编辑/删除/导入导出/恢复/清除按钮、鼠标及Unknown/NotApplied组合验收 **仍未执行完**；不拿存储测试或worker构造来源标签替代。独立源码测试确实覆盖对应逻辑，但完整产品窗口门禁仍阻塞。

截图仅由工具现场显示，保存UIA、时序、截图身份和launch二进制SHA，不保存其他应用画面或私人内容。停止后自有窗口可能仍开着，不声称已关闭；未继续向桌面发输入。原始轨迹见`round19-ui-first`和`round19-ui-final`证据。

## 全套回归、CI与身份

1190的13项、8418738焦点修复后的13项均本地通过，但841的Linux CI另发现文件锁问题，不能用本地绿覆盖。锁修复后的最终13项全部通过，绑定`e0d5cb26d9e173f6dd4999ec7628cc30e7394e11`：Rust **281 passed / 0 failed / 2 ignored**，Python **71 passed**，doc测试通过。结果单独保存为`round19-locked-results.json`及`round19-runs/locked-*`，每条命令/返回码/完整stdout/stderr/源码SHA独立记录。

实现提交e0d5的[精确push CI](https://github.com/WanFengQC/Cloudleaf/actions/runs/43395291024)已验证Windows/Linux两job各22步骤成功/按条件跳过，保留3产物及完整日志。841的失败运行与日志另外保留，不因修复而改写成通过。最终报告提交仍须有其自身的push CI及独立交付回执，不能复用实现提交结果。

1. git diff --check；2. cargo fmt --all --check；3. workspace Clippy all-targets locked/offline -D warnings；4. workspace all-targets测试；5. doc测试；6. workspace release；7. check_workspace；8. Python unittest；9. 精确依赖/许可证/RustSec审计；10. benchmark100；11.算法基线/消融；12.input examples release；13.search examples release。

新crate使用现有ring/pinyin，没有新增registry包、MSRV仍1.90.0/Edition2024。完整词典/模型/补层SHA、源/工具/测试/二进制各自登记；不伪称一个SHA同时代表逻辑用户包、实际磁盘代文件和CI程序。较早search差分源CRLF与当前LF逐行一致，但字节哈希不同；规模实测10源码另有当前精确字节绑定，不能混写。

历史3767个报告/数据/测试/许可路径、256旧Rust测试函数保留；原128340次R18证据不覆盖，也不重复加到188325新比较中。最新Windows/Linux必须绑定报告最终HEAD的push事件、两job原始步骤及产物；最终ZIP验证CRC、GitHub digest、每成员SHA，研发产物不打包用户词库、模型权重或私人配置。精确最终状态见Git外交付回执，报告不伪造包含自身的未来commit SHA。

## 放行判断与Round20

- **done（工程范围）**：独立真实存储、来源命名空间、CRUD/持久化/导入导出/迁移恢复、共享组合Trie、模型缓存重绑、候选来源、默认关闭学习及排序、严格对照、预算失败减少。
- **partial / P1**：简拼延迟退化、6/837剩余预算失败；用户层开启的Engine逐键/完整Host大规模准入未覆盖；默认字符P95未达5ms；普通桌面测量非专用实时验收。
- **用户暂停 / 未完成**：完整真实管理窗口、可信Applied学习与重启/清除/恢复闭环。不能因此删除门禁或编造通过。
- **外部阻塞 / 未实现**：独立人工gold和正式语言质量、正式生产数据/模型、完整TSF系统交互/安装、8h/24h、macOS/Linux系统适配；微软/自然码、双拼纠错等旧缺口继续存在。

本轮未发现其他已复现P0可据此放行，但没有证据声称所有系统级P0已排除。正式Top1/Top5/MRR继续null。用户学习是个人偏好，不变成公开词频或中文准确率证据。

Round20优先完成暂停的实际窗口闭环，继续简拼成本/偏好完整界的延迟定位，审查当前五个Draft及输入分支的真实merge-base和接口依赖，形成可审阅整合方案。没有自动合并批准或隔离系统环境授权前，不执行TSF。**main保持不变，CL-010继续关闭。**
