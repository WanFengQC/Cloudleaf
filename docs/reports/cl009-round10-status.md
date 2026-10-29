# CL-009 Round 10：独立语义 bundle 基础设施

2026-10-30。分支`codex/cl009-data`，R9起点`1c9a047c74aea01e504ed355601c0fe021b53f4b`。设计先提交`adf798f4833670244bb8dc727a95d718ddb98e93`，新API红`19284c4`，核心`458fa65`；两项实际新增verifier缺口先红再修，完整被测实现 **`2472dffc854a5a0db511757949c4c11e6867c769`**。最终报告提交及自己的Windows/Linux push CI由共享R10汇总单独绑定，不复用R9成功或制造包含本报告的自引用SHA。

## 实际实现与文件范围

|文件|交付行为|
|---|---|
|`tools/data_pipeline/semantic_bundle.py`|离线build/read/verify、严格资源manifest/schema、命名空间旧词条目录、稳定语义ID/外键、有向关系反查、内容审核绑定、显式外部策略与审批文件门禁、生产发布/回退/恢复入口|
|`tools/data_pipeline/pipeline.py`|`checked_pointer/activate/rollback`增加可选verifier，activate可选transition；默认参数仍原schema2行为，复用原guard/owner lock/复制重验/不可变release/单指针|
|`tools/data_pipeline/recovery.py`|仅可选verifier透传至原pointer验证；原PID/guard/链接/孤儿隔离及显式恢复算法保持|
|`tests/test_semantic_bundle.py`、`test_semantic_bundle_recovery.py`|13项真实实现测试，含7个原发布/回退边界的自有子进程中断和首发无current恢复；机械声明不是真实审批|
|[设计](../cl009-round10-semantic-bundle.md)、[操作说明](../acceptance/cl009-round10/README.md)、管线README|严格格式、实际使用、兼容和未交付边界；本报告和`cl009/round10/`归档原始证据|

新格式为`cloudleaf-semantic-bundle-v1`与`cloudleaf-semantic-content-v2`；原v1语言类型侧包不自动升级，旧词典schema2完整目录仍由旧verifier检查，当前Rust引擎不消费新sidecar。未改Rust、共享接口、来源锁/批准清单、原schema2生产包或用户词库。

候选允许pending/null payload而不生成事实；生产拒绝未审内容、research_only、空payload、失效审核、越范围来源、孤儿ID、未知schema/reader版本、重复JSON键/ID及损坏资源。内容支持entry-POS/tagset、人工稳定sense、显式有向translation和phrase引用，不从源数组下标猜稳定sense或英语目标；反查只引用原relation ID，不新增逆向equivalent。当前真实资料没有获准英语目标catalog或义项对齐，本轮未补齐这些内容。

## 外部可信边界与两项新增缺陷修复

内容审核绑定完整content、固定目录/来源原件/实际许可权利文件、三份实际协议和migration；修改后旧审核失效。作者不能是两reviewer之一，两reviewer不得相同；reject/分歧需要第四个独立arbiter、理由和当前binding。生产必须由caller显式提供bundle/store之外的trust-policy path+expectedSHA，精确匹配manifest/content/binding/reader/source范围及实际外部approval文件；包内自批、hash、Git或CI不形成信任根。

**这是外部受信审批清单及文件声明的一致性门禁，不是密码签名实现或身份鉴定。**始终`cryptographic_signature_verified=false`、`human_identity_or_rights_proven=false`。正向临时夹具使用明确fixture人物/来源/审批形状，只验证工具，不制造真实人、授权或内容。真实负责人还须核材料与签署来源；本轮没有配置可信签名key/签名器或交付真实生产审批。

1. [bool红测](cl009/round10/red-boolean/result.json)：`458fa65`实际9测1fail。外部approval的reader_version=True，并重绑approval/policy文件SHA，因Python True==1被接受。红提交`822a5e6`后最小增加实际int类型要求，不能用Hash掩盖类型错误。
2. [首发恢复红](cl009/round10/red-initial-recovery/result.json)：自有子进程在lock阶段异常退出，无current；旧wrapper在无policy时仍隔离/移动遗留锁，1测1fail。红提交`8f8fb5a`后在rollback/recover进入guard前核外部policy文件/SHA/位置/schema；有current仍完整核目标。没有current时恢复仅处理已退出owner的孤儿，不批准release或创建current。第一次红运行HEAD为`822a5e6`加未提交bool修正，故另将[精确`8f8fb5a`旧模块直接载入重放](cl009/round10/red-recovery-commit-replay/result.json)保存Git模块/test SHA和真实exit1，未把工作区状态冒充精确提交。

新API初始ImportError/[exit1](cl009/round10/red-api/result.json)仅证明本轮能力尚未实现，不称原生产算法损坏。初次定向8测有一个fixture准备错误：协议变更子例未重置物理协议文件，污染随后migration子例；[原文件/失败](cl009/round10/fixture-error/result.json)保留，重置夹具字节后原断言通过，不删测试或放宽生产规则。

## 升级、回退与中断恢复

每个manifest同时固定旧词典和语义全部资源，只切一个指针。初版previous为null；升级绑定当前实际manifest，记录新增/删除/变化/拆分/合并ID的显式from/to迁移，不自动猜语义映射。目标损坏、旧审批漂移、不兼容reader或迁移缺失拒绝，原current保持。回退验证完整previous，允许当前损坏但不改用户数据。

正向临时测试真实执行lock、copy-mid、cache-renamed、pointer-written、pointer-replaced、rollback-before、rollback-after七处子进程异常退出，owner锁与current/previous、隔离恢复及用户文件检查均通过。另有首发无current的缺policy拒绝和带外部fixture policy隔离恢复。旧schema2默认API/恢复全回归继续通过。未测试物理断电、真实操作系统安装、签署部署或跨版本用户库迁移；不能将子进程退出等同这些验收。

## 实际20项pending材料：可读候选，生产仍拒绝

仅复制现固定CC-CEDICT schema2九文件到Git外新目录，加R8实际20项待审请求的null payload、现存源断言/SHA/许可和pending协议/权利文件。没有填POS、定义对齐或translation，**semantic_payloads_created=0**。大候选约114MB不进Git；manifest和原路径/资源身份在[实际报告](cl009/round10/pending-real-materials.json)。

请求队列SHA`d7561fea9bf650edef74eb5f310a9f0586e536a7343364718c75192261d3080a`，原件ZIP SHA`fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`，旧facts SHA`168f4579a7d72c87c41a9b2bde0d5f65f1537c65b64e107ea60bbf409f7fd072`。候选manifest SHA`3fb8229edb1ccb2bf4ed26eec9c3ce5957f872f24d4eeb5ffb4e28b17b72dca9`；结构读取通过，但实际activate因pending来源/未审核内容/无实际审核材料拒绝，**目标production store未创建**，未提供外部policy。执行74.242秒含完整目录复制与重复校验，只是离线演示时间，不是实时输入延迟或资源基准。

CC-CEDICT/Unihan批准范围、Wiktionary research_only、Google候选状态及词频unknown不变。派生候选保留现LICENSE/NOTICE、固定来源和修改说明，不把数据重新许可成MIT。没有实际生产语义包或新统计观测。

## 全套检查、原始字节与身份

[初始检查](cl009/round10/checks/checks.json)前10命令实际均0：diff/status/rustc、fmt、locked/offline Clippy(all-targets)`-D warnings`、all-target Rust **60 passed**、doc、release、workspace依赖许可/资源审计和完整Python **108 passed**（原95+本轮13）。第11本地演示runner因直接运行.local脚本的sys.path不含repo而ModuleNotFoundError/exit1；原stderr和全部已过结果保留，第12原计划尚未执行。只修本地导入路径，[followup](cl009/round10/followup/checks.json)补跑实际候选演示0和真实pending标注audit2，不重复已通过全回归。实际共13次命令执行，不能把末项2或链式shell状态冒充全部通过。

|身份|SHA-256|
|---|---|
|新semantic_bundle.py原字节/本次Git LF一致|`2d453bab0d8e7780280391fd52203ee38d696c00a9b288e6ac77d8aef671fac0`|
|pipeline.py|`41ce3ce9e92229ff0b1e6f75fb41ca6f959b389bab32042b5763d6cc1d0cfc09`|
|recovery.py|`760f106fe5fca0cbd111d366c3f83a3fdf266c9cd84992860baf33b18bffd3b9`|
|source-v2|`541dcbdd8d1569c11cb62a94150b467b02855eebfd90af20ca7a00da60ec8df9`；不含tools目录，新模块另绑定Git/blob/原SHA|
|release CLI347,136B|`d53f54a25bd6b4fb8c114a06966010d4b98bf0597efb7e9c5e897e96c59e5b80`；build可复用缓存，未跑新基准|

完整test/模块/二进制/旧九包身份在[identity](cl009/round10/identity.json)，[46份原文封装](cl009/round10/raw-artifacts.json)可还原原始UTF-8字节，LF展示件分列[50项清单](cl009/round10/checksums.json)，[5个实际运行器](cl009/round10/runners.json)供复查。起点259份报告、旧测试、Rust/数据/来源锁和旧9包实物SHA均保持。builder_sha256读取工具原字节，双构建只证同固定工具/输入确定性；未测Windows/Linux成品哈希相等，不宣称跨平台bundle逐字节复现。

## done / partial / blocked / not-run

|状态|结论|
|---|---|
|done|独立reader/strict verifier/manifest/ID引用、内容审核与外部精确审批清单门禁、可复用发布/升级/回退/中断恢复基础设施；13项真实实现测试与60 Rust/108 Python回归；实际pending拒绝|
|partial|文件/来源/审核/策略的一致性可检查，不能证明真人/权利/语义真实性；只有临时机械正向审批声明，尚无真实生产包|
|blocked|真实语义独立双审/授权/目标英语catalog与sense对应、项目负责人可信生产审批及独立质量材料未交付|
|not-run/未实现|密码签名器/key部署/真实签署；物理断电、真实用户迁移、实际生产语义发布、默认引擎接入、长测、TSF与CL-010|

本轮两项新verifier/生命周期缺口已修，已审增量未留已复现未修P0/P1。外部权利/人工语义和正式质量仍是原发布门禁，不能靠CI或fixture解除。PR #3保持Draft，不合main、不强推、不启动CL-010。
