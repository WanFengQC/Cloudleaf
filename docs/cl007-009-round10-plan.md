# Cloudleaf Round 10：设计记录与有界实施范围

2026-10-30。本文件在三个方向稳定后归档到共享分支；执行前已在本轮对话给出文件范围、依赖与验收条件，各方向设计提交先于实现。实际顺序C→B→A→共享，同一时刻只一方向修改工程；只读技术核对不代表独立真人审阅。main保持`a564fd47b0f054ff360592af3ccb8d744021160f`。

## 实际基线与设计先行

|方向|核对的R9起点|本轮先行设计/红测试|有界范围|
|---|---|---|---|
|C|`1c9a047c74aea01e504ed355601c0fe021b53f4b`|设计`adf798f4833670244bb8dc727a95d718ddb98e93`，新API红`19284c4`|独立语义reader/verifier/manifest与发布、升级、回退、恢复；原schema2默认行为不变|
|B|`2751b1512394e09c92bca2b4329456c3cd3c30de`|合同及旧compare反例`02fb8eceb541e842235e7daa90725398da39634a`|观测入口、配对计数门禁、probe输出和真实算法边界测试；不改搜索src或默认引擎|
|A|`1a05633c27c133bc0558b0fb955b0f916edb6b72`|设计`f940a6cdb7ed12a99e6c86875a2bee6bf541ddf0`、旧反例`241d4daea9f3199e8bbdfc1908bc379d89c542c6`|实际授权绑定完整运行身份、材料/执行分列、新pending验收材料；不改生产Rust或运行长测|
|共享|`e296c300ea1b1d41c6cdc36500f690dec0906964`|本记录、人工审阅补充与空白决定模板先归档|只审接口兼容、固定方向身份/原始证据/最终CI和汇总，不合入方向代码|

## 文件与依赖

C新增`tools/data_pipeline/semantic_bundle.py`和两份真实测试，原`pipeline.py/recovery.py`仅增加可选验证/迁移钩子；文档、操作入口和证据独立归档。候选pending允许null payload，production必须有当前内容审核与显式外部受信策略，文件一致不证明真人/权利。原词典schema2和当前输入流程不消费新sidecar。

B新增`scripts/search_resource_contract.py`、`check_search_resources.py`、Python入口测试、Rust预算/完整边覆盖测试；旧`evaluate_real_lexicon.py`复用入口，`resource_probe.rs`输出v2完整配置及原输入。使用已取得固定runtime与24资源探针，四方法同输入/配置/预算，4预算组、每例预热1/计时5，不重新下载或生成gold。

A仅改`scripts/check_windows_soak_admission.py`和真实工具回归、新pending材料；原runner已复用同一入口，reporter逐segment隔离无需重写。授权schema2绑定完整运行identity，只排除准入后加入的批准摘要以避免自引用。外部VM/恢复/人员/授权缺失时不启动子进程；不运行8h/24h、TSF或真实应用。

正式词频、语义内容和独立gold的依赖没有因基础设施实现解除。C的真实授权/内容与协议→B显式评估适配；A的隔离环境、恢复验证和执行授权必须由用户落实。共享PR只有用户实际决定并合并后，才另安排方向普通merge、新回归和新CI。

## 验收与风险

每个可修复实际问题先保留旧失败，再最小修复；本地独立保存fmt、Clippy/all-targets/doc/release locked/offline、Python、依赖/资源审计和专项的命令、返回码及原始流。代码、数据、工具、二进制分别SHA；最终push精确head须核Windows/Linux全部步骤与产物，报告提交自身身份通过提交后的外部收据固定。

主要风险是语义审批/签署真实性缺失、候选搜索预算截断及未证明的字节内存/延迟、旧授权可重填、未执行长期/真实平台验收和旧方向merge-base。不得以文件哈希、机械fixture、计数通过或CI解除这些风险。不降低预算、不删旧测试/历史/失败、不强推、不改变Draft、不合main、不进入CL-010。
