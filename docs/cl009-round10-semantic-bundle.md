# Round 10：独立语义 bundle 与受外部策略约束的发布器

2026-10-30。C 起点 `1c9a047c74aea01e504ed355601c0fe021b53f4b`。本轮用户要求先实现可单人完成的生产基础设施，再等待真实内容及授权；因此不再把 R8“先真实审核材料才实施”的计划顺序当作工程实现阻挡。真实数据发布门禁保持，不以夹具或代码实现替代人审。旧词典 schema2、v1 类型侧包和实时 Rust 输入行为均不改变。

## 1. 实际缺口与文件范围

- `linguistic_sidecar.validate_sidecar` 只有类型/引用和自带审核 ID 声明，缺作者、内容摘要及实际权利/协议材料绑定，不能直接改名生产许可。
- `pipeline.verify_release` 严格接受原词典 schema2/固定九文件；`checked_pointer/activate/rollback/recover_store` 使用该验证器，尚不支持独立语义 bundle。
- R8 的 20 条 CC-CEDICT 原义项队列全部 pending，没有人工稳定 sense、POS、翻译对齐或新授权。本轮不补这些值。

新增 `tools/data_pipeline/semantic_bundle.py`（离线 reader、严格 verifier、manifest、候选构建和发布入口），`tests/test_semantic_bundle.py`、`tests/test_semantic_bundle_recovery.py`，操作说明及 R10 证据。原 `pipeline.py`/`recovery.py` 只增加默认保持旧行为的可选验证器注入，让新入口复用原锁、复制、单指针、rollback 和显式恢复。不得复制整套并发/恢复算法；未指定参数的原 API 与所有原测试继续使用旧词典 verifier。

生产入口只接新模块自己的“严格内容验证 + 外部策略”组合，不暴露一个遗漏策略的默认生产路径。语义 store 必须由调用者另给，与原词典 store 分离，不迁移、改写真实 current 或用户字库。模型/网络、Python 和文件读取均不进入实时输入路径。

## 2. 版本化信封与内容

新 bundle 使用 `format=cloudleaf-semantic-bundle-v1`、`schema_version=1`，reader 支持版本固定为 1；内容使用 `format=cloudleaf-semantic-content-v2`、`schema_version=2`。不静默迁移 `cloudleaf-linguistic-sidecar-v1`。JSON 拒绝重复键、未知字段、bool 代整数、非有限数值和 BOM；UTF-8/LF、canonical 键排序与末尾 LF 沿用现工程规则。资源路径严格为清单内相对路径，禁止链接/父级跳转/路径越界，重用现文件系统门禁。

bundle 的 manifest 固定 format/schema/reader_version、所有实际资源 `path/bytes/sha256`、内容与审核绑定摘要、上版 manifest 身份及迁移文件身份。资源包括内容、审核、来源证据/许可/协议、迁移、NOTICE 与所引用的完整旧词典目录；目录中所有普通文件必须恰好对应清单，不存在未登记的“批准文件”。manifest 本身不自引用。清单资源上限和总量以现词典发布预算为依据，单个内容 JSON 有明确 64 MiB 上限，不能无限读外来数据。

内容严格字段如下；独立来源、协议与审核材料可以作为清单资源保存，但权利原件不由程序虚构。

|字段|合同|
|---|---|
|catalogs|每项 namespace/language、bundle 内旧词典目录、实际 manifest SHA 和 records.jsonl SHA；旧目录调用原 `verify_release`，形成 namespace+稳定 entry_id 目录。英语目标目录未交付就不能生成英语 target entry。|
|sources|稳定 id/version、原件 SHA、license_id、许可/权利审核文件 path+SHA、适用 scopes 和 pending/research_only/approved 状态；具体范围必须与实际权利审核材料及外部策略相符。|
|protocols|本轮语义处理、预处理、lineage 三份实际资源 path+SHA；不是 IME 三协议的自动迁移或同一接口。|
|records|每项严格 id/kind/payload/source_assertions/status；kind 仅 entry-pos/sense/translation/phrase。payload 可以为 null 的 pending 候选，不因此生成语义事实。|

每个 source_assertion 包含 source_id、固定源 unit/pointer 与原始片段 SHA，来源必须存在、适用 scope 正确；不把网页路径、数组下标或源位置自动当作稳定人工 sense ID。

- entry-pos payload：namespace/entry_id、tag/tagset/tagset_version。
- sense payload：人工稳定 sense ID、namespace/entry_id、language、definition、可空 POS 引用。
- translation payload：已存在的 source_sense_id/target_sense_id、显式 equivalent/broader/narrower/related；源目标不能相同，不推导逆向 equivalent。反查只返回同一 relation ID。
- phrase payload：稳定 phrase ID、namespace/entry_id、kind 与显式组件 entry 引用。不能由分词自动推断固定搭配。

集合 ID 唯一、所有引用完整且类型正确；entry 引用须在固定目录，sense 引用须在本包。pending payload=null 可供候选 reader 保留，但任何 pending/research_only/空 payload 都拒绝生产发布。原义项字符串不是英语目标义项。频率观测暂不进入此语义格式，继续复用已有独立频率设计且保持 unknown，不为本轮增加未经批准统计内容。

## 3. 内容审核、外部信任与生产门禁

内容审核独立资源严格登记作者、两位不同且非作者的 reviewer 及 decision/reason；出现 reject/分歧时要求不同于作者和两审核员的 arbiter、decision/reason。每份审核绑定当前完整内容摘要、目录/来源原件/实际许可与权利材料、三份协议和迁移摘要，所有改变使旧审核失效。程序可核文件/摘要/声明，不证明 ID 对应真人、签字真实或语义正确；不代签、不重新计算后把旧审核当新签署。

生产必须显式提供 bundle/store 之外的 trust-policy 文件及其 expected SHA。包内自行携带的 approved、manifest/hash、Git SHA、CI 或自签 key 不形成信任根。策略记录明确格式版本和负责人批准的固定 manifest/content/review-binding/version、可用来源/原件/范围及实际批准材料的 path+SHA；仅对精确允许项生效。批准材料还须绑定相同的 manifest/content/binding 和批准身份。用户未提供真实可信材料时，生产流程在任何 store 写入之前拒绝。

这是一条外部受信版本清单/审批材料的一致性门禁，不是密码签名实现。报告固定 `cryptographic_signature_verified=false`、`human_identity_or_rights_proven=false`，不使用含混的“production_verified=true”。无新密码库、无自研密码算法。真实密钥、签名工具/权限和身份真实性仍由项目负责人落实；将来可以在同一外部授权边界接经过审查的签署系统，本轮不伪称已经交付。

候选构建/reader 可以输出结构检查成功和具体 production blockers；生产 activate/rollback/recover 始终调用严格 verifier 与所给外部策略，不允许把候选 verifier 误用为可发布 verifier。策略缺失、失效或实际材料未齐时，仍可检查候选，但不调用发布写入。机械正向夹具只演示验证逻辑和临时目录，不代表真实权限、人工内容或已批准产品数据。

## 4. 原子升级、回退、恢复与兼容

复用既有 `publish_lock/store_guard`、不可变 hash release、复制后重验、单个 `current.json` 和 previous 方式；manifest 同时固定词典与语义内容，不切两个指针。新 reader 在离线路径一次校验并返回索引/内容，默认输入引擎不加载或消费。

升级需新 manifest 显式绑定当前上一版 manifest SHA。首次版本 previous 为 null；非首次必须匹配当前指针，避免拿任意候选当兼容升级。迁移记录显式列 from_ids/to_ids 和理由；旧 ID 的删除、添加、payload 改动/拆分/合并必须有对应映射，旧/新 ID 分别存在于各自包。未变内容可保留 ID；不是工具根据释义相似度猜迁移。迁移身份纳入审核绑定。回退验证 previous 的格式、内容和策略许可，允许当前损坏但目标必须完整，不覆盖用户字库或删旧 release。

复用 recovery 的 OS guard、遗留 owner PID 已实际退出证明、路径/孤儿核对及隔离迁移；活 owner/PID 复用或权限不明拒绝恢复。注入的 bundle pointer checker 在正常恢复/回退前完整验证相应目标。测试真子进程中断在现锁/复制/缓存/pointer 边界，不用 mock 复制产品逻辑；不能称物理断电保证。原 schema2 API 默认行为和原恢复测试全部保留。

## 5. 先失败再实现的验收

先提交本设计及必要失败用例，再实现代码。真实实现测试覆盖：独立 reader、schema/版本/未知字段/重复键，稳定 ID/孤儿引用/有向关系，pending 可读而生产拒绝，许可范围与实际材料不符，作者自审/同人双审/无仲裁，内容/来源/协议/迁移变动导致旧审核拒绝，缺外部策略/包内伪策略/清单漂移拒绝，损坏文件/混版本/不兼容 reader，双构建字节一致，正常升级/回退与损坏目标，用户文件保持，子进程中断恢复。

现获批 CC-CEDICT 的 20 条真实待审材料只能演示 pending 与拒绝，不能填POS、义项对应或生成翻译。原 schema2 完整构建/发布/恢复、所有 Rust/Python 回归、fmt/Clippy/release/工作区和依赖审计必须执行，保留命令/返回码/失败原文与源码/数据/工具/二进制 SHA。新最终提交取得自身 Windows/Linux push CI，不能继承 R9。

交付区分：工程 reader/verifier/manifest/发布回退恢复能力、模拟审批材料测试、真实 pending 拒绝，以及外部仍缺的真实双审/授权/可信审批与质量验收。无实际生产语义发布、默认引擎替换、长测、TSF、main 合并或 CL-010。
