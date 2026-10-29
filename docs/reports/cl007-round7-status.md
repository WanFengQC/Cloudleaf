# CL-007 Round 7：生命周期审阅、长期准入和原始证据完整性

2026-10-29。起点`caefb7f2ac7c8584249c05359fcb174f87640886`；被测实现**`e50c1d88b34fbf824cd46d1f22c2035700cc75cf`**。后续提交只整理报告，不改绑测量。生产Rust、COM/Host回收、总预算和共享接口未变；没有注册、激活、安装、卸载TSF，没有向真实宿主注键或杀死真实应用。**8h/24h和真实应用均未执行/未验收。**

## 实际审阅及设计先行

已读取AGENTS、roadmap、产品范围、R6完整报告及实际`service/session/watchdog/owner_cleanup/recovery/ownership/budget/candidate`源码。Factory、Service、LockServer、各种Lease与最后预算凭证维持模块保活；Host/Rc/COM归owner STA。sink撤销前失效epoch，旧异步callback检查instance/epoch/context/sequence及一次调用标记。恢复flight提前保存纯值，Writing和Unknown不得改为NotApplied或自动重放。

worker任务在创建前登记全局JoinHandle目录，reaper必须同时满足构造结束、退休请求、独占锁、真实OS线程HANDLE退出，再join/退slot；无法取得证明保持保活。恢复receipt只有真实owner或原owner已经真实退出且monitor完成/回调归零/独占类注销等条件齐全才退纯资源，不跨线程Release COM或DestroyWindow。DLL卸载仍检查OBJECTS/LOCKS，不能由材料显式清除推断Host已经析构。

先行设计 **`7083c0fd65092b9ca6162df00c2ad4782a783229`**：[七类故障及部署恢复矩阵](../cl007-round7-reliability.md)，逐类列资源归属、允许回收条件、用户状态、输入是否继续、隔离/终止条件及具体测试入口。外部COM引用的直接回归为`lifecycle_review_unadvise_failure_defangs_host_and_pins_dll_until_external_release`，sink正常与部分失败分别为`lifecycle_review_repeated_activate_deactivate_releases_manager_and_sinks`和`lifecycle_review_partial_activation_failure_rolls_back_only_completed_steps`。其原始执行在[完整Rust输出](cl007/round7/workspace-tests.raw.json)，异常STA/恢复窗口/预算等也另执行release专项。

没有发现本轮可安全独立修复的新生产P0；不可回收Host/永久COM/owner卡死累积耗尽额度仍是部署级阻断，未用无界预算或跨线程释放掩盖。只定义了进程内隔离、工程自有进程回收和待实现的生产用户确认/正常退出恢复方案；生产故障控制器和文档事务跨进程恢复未实现。

|实际修改|目的|
|---|---|
|`scripts/stress_windows_faults.py`|原始逐键/周期/final分母与身份/顺序一致性审核；记录机器/架构/Python身份；8h/24h的外部批准准入和只读预检|
|`tests/test_windows_stress.py`|四项回归，覆盖合法JSON缺键/缺周期、重复/错序/上下文变化、完整/部分分母及隔离批准身份；保留全部旧测试|
|`docs/cl007-round7-reliability.md`、README、本报告及`cl007/round7/`|生命周期分类、执行清单、未完成部署策略、真实失败/全部命令和身份|

## 真实红测与工具修复

测试提交 **`7914589a03fedad0e50b0ab5598a609d450abfc2`** 先于实现。[原始红测](cl007/round7/red/output.raw.json)和[身份/命令](cl007/round7/red/identity.json)保留：5个实际失败断言显示旧`latency_summary`对合法JSON的缺键、缺周期、重复sequence、改变instance或cycle均返回`raw_errors=[]`；另两项分别是缺`trace_complete`字段和缺`execution_preflight` API，明确属于缺能力错误，不能充作COM丢字缺陷。

R6成功条件依赖子进程0、final时间/预算和JSON解码结果，未证明落盘逐键/周期分母与final一致。本轮在原始JSON入口审核固定诊断协议，检查连续sequence、cycle、instance/epoch及六键顺序、每周期6次写入/FIFO证明和唯一final总数。中断/失败保留部分分母，`trace_complete=false`；非bounded的机制通过必须有完整trace。保留旧bounded档的兼容行为，不把无逐键协议的旧档伪标成新完整审计。

为进一步核对实际记录，使用新正常段的原始trace另造五份**明确标记的损坏副本**：缺键、缺周期、重复sequence、上下文变化、错误VK。[旧新结果原件](cl007/round7/trace-mutants.json)显示旧caefb7f工具五例仍无raw错误，新工具五例均记录错误且不完整；旧工具完整字节/数据副本/SHA保存。这不是实际输入丢失，也不是语言gold，不能扩大成生产系统缺陷。

8h/24h现在必须有与精确源码/数据/工具/测试exe/配置/环境完全相同的外部批准记录，含隔离环境ID、负责人、快照、回滚和执行授权引用。批准内容规范SHA进入运行manifest，续跑也绑定；仍沿用真实进程创建时间和句柄、新segment、不拼接时长规则。工具只核记录一致性，**`declarations_independently_verified=false`**，不能替真人核隔离环境或授权真实性。fixture中的批准只测试工具结构，不是实际许可。

## 全量检查与实际工程短档

[10条完整检查](cl007/round7/checks.json)全部exit0：locked fetch/offline debug与release、fmt、Clippy `-D warnings`、**157 Rust**（另2项默认ignored在release显式执行）、doc、资源/许可证、**25 Python**以及精确Windows依赖/RustSec审计。[完整18条专项/准入调用](cl007/round7/diagnostics.json)逐命令保留返回码和原始输出：9项既有release loader/COM基准/预算/恢复/R3–5及原型专项exit0；实际Cargo no-run取二进制1项0；8h/24h预检和直接启动守卫4项均exit2；四短档0/130/0/1。拒绝不算长期验收通过。

真实release DLL loader完成20次自有COM生命周期检查，句柄113→113，`registered=false/real_application_acceptance=false`。这是实际DLL加载/卸载与自编COM manager，仍不是Windows TSF注册或真实应用测试。源码未改变这条生产路径，不能把重新执行旧回归说成新功能。

|短档|实际结果|原始证据|
|---|---|---|
|正常4秒|runner/child0，80周期/480键，trace完整|[562事件、39采样、42检查点](cl007/round7/complete/segment-0001/summary.json)|
|2.5秒受控中断|runner130，child1，22完整周期/132已落盘键，无final，保持部分|[155事件、16采样、19检查点](cl007/round7/resumed/segment-0001/summary.json)|
|同身份新段续跑4秒|runner/child0，新segment独立80周期/480键，未合并时长|[562事件、38采样、41检查点](cl007/round7/resumed/segment-0002/summary.json)|
|第3周期注入失败|runner1、child101，3周期/18键，无final|[22事件、10采样、13检查点和panic原文](cl007/round7/injected-failure/segment-0001/summary.json)|

两个完整段各验证32/32旧Unknown恢复材料且无自动重放；两中断/失败段未执行尾部，成功率和自动重放统计保持null。四段长期完成标志均false。8h/24h两份实际[预检](cl007/round7/preflight-8h.json)与[24h预检](cl007/round7/preflight-24h.json)均blocked；直接启动也拒绝，两个计划运行目录均不存在、未创建测试child。当前没有真实批准记录。

正常段480键的admission P50/P95/P99为29,100/208,200/515,300ns；到周期drain末为172,000/581,200/1,039,700ns。前者包含诊断snapshot克隆，后者不是单键TSF文档确认时间；末处理会读取/排序延迟数组，不是恒定内存。39次资源采样峰句柄145、线程7、working set15,192,064B、private commit6,066,176B，OS峰working set15,806,464B。4秒不能证明长期趋势或真实UI延迟；进程退出后的32KiB工作集不证明先前COM安全释放。桌面背景负载未完全控制，不对比R6宣称性能改善。

## 精确身份与历史保全

|身份|SHA-256|
|---|---|
|实现源码`cloudleaf-source-v3`|`dad282f1abd9085f418569b99647e297380ef8a1fdfe9b1faf90617a74de6aa5`|
|压力工具|`8acc7e0745d3e9ecba818d2c12dd2a99a22442a5cf1fd42da98166f6a937f1c1`|
|Cargo实际release测试exe|`c7fe4b34c197871bb8d02acb37a005f93ac658800712a015a544312a7e54a0b6`|
|真实release DLL / 599,552B|`204fbb8ee725562a449ba6bcac730ab113944350e92c7a2d6ddd97fd6e73b552`|
|原型数据|`d4a87e958b23097253a4601b9ae373f02964e3e37729bb7f560e20835d5bb5f0`|

测试exe/DLL与R6字节相同，因为生产Rust和测试host均未改；本轮新验证的是新Python工具及其完整性/准入，不伪造新native二进制。OS/机器摘要/架构/CPU/Python身份见各manifest，[全部源文件/资源/历史身份](cl007/round7/evidence.json)和[依赖审计快照](cl007/round7/dependency-audit.json)分别记录工具/数据库SHA。8个关键生产文件的实际字节SHA均固定。

起点**346份**历史报告未变、无旧测试删除，生产Rust及依赖锁/共享核心无差异，64/41/83资源原SHA不变。原始UTF-8输出通过`.raw.json/raw_text`无损重建，展示LF与原字节分开计SHA；[checksums](cl007/round7/checksums.json)核全部发布文件且排除自身，[运行器源码](cl007/round7/runner-sources.json)可重放。收尾单独审计曾因shell未补Cargo PATH打印WinError2，链式shell整体0没有被当作该审计通过；[准备失败及独立重跑](cl007/round7/publication-preparation.json)明确原字节未单独保存的限制，补正确PATH后实际独立exit0、原输出另存。本报告之后的最终交付HEAD和Windows/Linux push CI由共享Round 7汇总独立核对，不沿用R6或默认PR merge结果。

|状态|退出条件/剩余依赖|
|---|---|
|已实现|七类生命周期/部署恢复设计、机器/解释器身份、长测记录准入和只读预检、原始逐键/周期/final审计|
|已通过测试|5个原始完整性反例修复、完整工程回归、实际DLL loader/故障专项、短档/中断/新段/失败证据；没有新生产P0修复宣称|
|长期实测|8h/24h未启动/未完成；必须获批隔离环境和执行授权，分别连续28800/86400秒，不能拼接|
|真实应用|未验收，未注册/激活/安装/卸载；需签名/位数/快照/可逆回滚、实际应用矩阵和人工批准|
|仍阻塞|永久Host/COM/owner占用后的生产部署恢复、真实输入结果核对和连续长期证据；全局外部COM引用数仍unknown|
|不放行|CL-007退出条件未满足，PR #4继续Draft，不合main，不进入CL-010|
