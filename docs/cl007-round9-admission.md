# CL-007 Round 9：真实环境材料的只读准入

2026-10-30。起点 `333dc90e0985a13f75629e29599f3df1d7d951ab`。R7已实现身份/声明预检，R8已实现独立segment报告；本轮不重复这些能力，不运行native压力、loader、8h/24h或TSF，不修改生产Rust/共享接口。

## 实际缺口与最小修复

`execution_preflight`原先允许只有五个非空字符串、isolated/approved声明和matching identity的长期批准。它不核版本/架构、快照校验值、实际恢复和授权文件；R8 pending环境表与runner未绑定。这是准入材料缺口，不是已证明发生了生产丢字。

新增离线 `check_windows_soak_admission.py --request <环境表> --output <全新JSON>`，只读取JSON及其明确引用文件、核SHA并输出缺件。可在无native binary/Win32/VM的情况下检查R8/R9环境请求。它不启动任何进程、不生成批准、不探测或恢复VM。

保留R8环境表结构，R9副本增加明确的材料格式说明。需准备状态 `ready_for_preflight/prepared`、环境ID、完整Windows版本/构建字符串、架构、快照ID和SHA、实际负责人/执行者；允许同一用户兼任，不要求虚构外部观察者。`restore_exercise_evidence`与`signatures_and_authorization_evidence`使用 `{path, sha256}`，相对环境表目录解析。文件内容和Hash必须实际存在且对应环境/快照/执行者；恢复记录还绑定快照SHA、真实执行时间和passed结果，授权记录绑定负责人、执行时间、允许的8h/24h档位。请求表本身不是执行或TSF授权。

runner原批准结构增加 `{environment_request: {path, sha256}}`，相对批准文件目录解析；复用同一个checker，核版本/架构与实际identity一致、档位在授权范围、原环境/快照/operator声明与环境表一致。保留原完整identity比较。缺件在 `prepare_segment`及child创建前拒绝；短诊断和旧有界档不新增环境要求。旧仅字符串批准从此不足，新批准需要重新准备；不会自动升级旧批准。

恢复记录格式为schema1/kind=`cloudleaf-windows-restore-exercise`，status=passed，environment_id/snapshot_id/snapshot_storage_sha256/operator/performed_at_utc。执行授权为schema1/kind=`cloudleaf-windows-soak-execution-authorization`，status=approved，environment_id/snapshot_id/operator/authorized_by/authorized_at_utc/allowed_profiles。时间使用含时区ISO8601。内容一致和SHA不能证明人签或物理恢复真实性，结果固定声明 `declarations_independently_verified=false`；仍需用户核验原件与真实环境。

## 文件与验收

- 设计先提交，再提交会在旧实现失败的“仅字符串批准必须拒绝”反例并保留原文，随后实现checker/runner接入。
- 真实R8 pending请求应只读拒绝；临时文件只用于验证格式、Hash、各必需字段、材料错绑和内容变化；不当作实际批准/回滚证明。
- 保留旧测试所有断言，正向fixture补充临时完整材料。继续复核R8真实中断/续跑按段输出，长期接受始终false，不新造长样本。
- 增加R9 pending环境副本及人工应用记录模板；R8模板和全部历史证据保持不变。
- 按用户要求执行fmt、Clippy/all-targets/doc/release locked/offline、资源/许可审计、Python全回归和diff。常规测试不是新的native压力或平台验收。保存命令/返回码/失败及工具、源码、数据、已有/新构建产物身份，normal push原Draft分支，新最终head另核两平台CI。
