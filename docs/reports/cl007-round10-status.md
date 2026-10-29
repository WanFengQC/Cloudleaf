# CL-007 Round 10：执行授权绑定完整运行身份

2026-10-30。分支`codex/cl007-windows`，起点`1a05633c27c133bc0558b0fb955b0f916edb6b72`；先行设计`f940a6cdb7ed12a99e6c86875a2bee6bf541ddf0`、红测`241d4daea9f3199e8bbdfc1908bc379d89c542c6`，完整被测实现 **`e58d3ae6d8671a5f8f822aa2b0d0d80f37b93596`**。后续只归档报告及原件；最终报告提交SHA和新Windows/Linux push CI由共享Round 10汇总另核，不伪造自引用HEAD或继承R9 CI。

## 实际缺口与修复

Round 9实际授权文件只绑定环境/人员/时间/档位，完整identity比较在可重新填写的外层批准表。新反例保留授权文件原件与SHA，分别更换commit、source、binary、data、tool或配置，同时更新外层identity；旧实现六个subcase全部错误地放行。这是准入一致性缺口，不是已经观测到生产丢字或真实未授权长测。[设计](../cl007-round10-execution-binding.md)先提交后实施。

|文件|具体修改|
|---|---|
|`scripts/check_windows_soak_admission.py`|实际授权要求schema2、完整`execution_identity_sha256`，规范JSON绑定全部本次身份，仅排除准入通过后加入的`execution_approval_sha256`避免自引用；旧schema1不得自动升级|
|同checker报告|分列`materials_consistent`、`execution_identity_binding_checked`与`ready`；无实际identity时即使材料一致也拒绝执行，单独CLI不授予本次运行准入|
|`tests/test_windows_stress.py`|新增旧授权不变、外层六类identity替换反例；原正向fixture绑定同一identity，其余断言保留|
|`tests/test_windows_soak_admission.py`|正向fixture明确使用schema2及identity，原字段/文件失败测试也传identity以免因缺identity一概失败；新增独立材料不放行、旧schema/缺摘要拒绝与canonical/自引用边界3项|
|`docs/acceptance/cl007-round10-*.json`及steps|新增4份pending模板和实际应用步骤，复用R8九类矩阵，不填写人员、批准、运行ID或结果；R8/9原件不改|
|README、设计、本报告及`cl007/round10/`|新行为入口、实际命令/原始失败/身份与未完成边界|

runner本来已在segment/child创建前不可绕过地调用同一checker，无须另改流程；短档、生产Rust、COM/STA/Host、watchdog、Unknown语义、资源预算、共享接口和资源reporter未改。SHA只核材料/身份一致，`declarations_independently_verified=false`；不能证明真人签署、物理恢复或TSF获准。

## 可重放失败与全部检查

[旧红测原文](cl007/round10/red/stderr.txt)为11项工具测试、6个subcase失败、exit1，均为`AssertionError: True is not false`。执行HEAD为设计提交加新测试工作区，测试及runner SHA、命令和返回码在[red/result](cl007/round10/red/result.json)，随后固定同一测试提交`241d4da`。临时fixture明确非真实VM/授权，不冒充人工批准。

[11项逐命令记录](cl007/round10/validation/checks.json)：diff/status/fmt/Clippy(all-targets)/Rust all-targets/doc/release/workspace audit/Python/依赖审计均0，实际pending材料检查按预期2。Rust **157 passed、2 ignored**，旧手动长期测试仍ignored；Python **42 passed**（原38+4），包括原reporter中断/续跑按段回归；文档测试无失败。构建/Clippy/test均`--locked --offline`，Rust1.90.0/Edition2024、Windows11 AMD64/Python3.12.10，实际版本记录见[environment](cl007/round10/validation/environment.json)。没有删除旧测试、绕过断言或降低预算。

pending实际CLI报告11项缺件、`ready=false`、材料与identity检查false、`child_started=false`、系统操作与长期验收false，见[pending-admission](cl007/round10/validation/pending-admission.json)。没有创建运行segment或长测ID。RustSec沿用仓库cargo-audit0.22.2和15个Windows依赖许可原件，完整身份/警告分母在[dependency-audit](cl007/round10/validation/dependency-audit.json)，未引入新依赖。

归档工具首次把563份历史报告路径全部传给Git，触及Windows命令行长度WinError206。不是产品测试失败；首次工具转录及部分输出保留本地，冻结修正前脚本，另目录重放同一核验逻辑并保存实际stdout/stderr/code1，见[publication-failure](cl007/round10/publication-failure/result.json)。随后只改为一次`git diff --name-only <base> -- docs/reports`并与旧文件集合相交核验，成功归档；已通过的产品测试未重复跑。原失败、原脚本与重放脚本均归档，不将重放字节冒充首次未捕获的进程流。

## SHA与原件保全

|身份|SHA-256|
|---|---|
|实现source-v3工作区原字节|`54005144cbfb1939953958c99771940b9a166426606f78e7601b3dd9b1e664c1`|
|新checker|`045e9de59ac47e1506e969d9afc8c55cfc1f1e62d2b71f75c9a5906532492d36`|
|未改runner|`8ac58e1bb51f133b2022e887a32c0c994cccc13bd434736861fda4164278e1cc`|
|未改资源reporter|`2b48f22e4d37cbd0bb40298067b6a4058b6f78f80a46d80e51e8b64040d936c7`|
|原型数据|`d4a87e958b23097253a4601b9ae373f02964e3e37729bb7f560e20835d5bb5f0`|
|release CLI|`ea27f277b39e36454c250b2bb2bddf16b02c1012b543693009e38d0166c4c4d9`|
|release DLL|`204fbb8ee725562a449ba6bcac730ab113944350e92c7a2d6ddd97fd6e73b552`|

其余release产物大小/身份见[identity](cl007/round10/identity.json)。Cargo日志保留缓存复用事实，产物SHA不是新性能测量；没有独立native/loader或长期内存/句柄/线程采样。Git LF按完整实现提交追溯，与source-v3工作区原字节分列。[32份原字节封装](cl007/round10/raw-artifacts.json)可用utf8还原SHA，规范LF展示不替代原件；[运行器源码](cl007/round10/runners.json)含修正前/后归档器及失败重放，[Git发布清单](cl007/round10/checksums.json)另核。563份起点历史报告、旧Rust/Python测试、全部原型与正式数据文件保持。

## 退出与下一步

|状态|实际结论|
|---|---|
|done|具体旧授权复用反例、schema2完整身份绑定、材料与可执行分列、新pending应用材料、完整回归和原始证据|
|partial|文件/身份一致性工具可用；不能证明真人签署、真实恢复或实际长期输入可靠性|
|blocked|真实隔离VM/快照还原/负责人/执行授权未落实；生产不可回收Host/COM耗尽后的部署与真实文字恢复仍缺|
|not-run|8h/24h、独立native压力/loader、TSF注册/激活/安装/卸载、真实应用验收、新性能/长期资源测量|

用户需要准备实际环境并验证恢复，在冻结完整运行身份后另行出具schema2执行授权；后续仍须分别满足长期与TSF批准门禁。Unknown不得自动重放、旧新上下文隔离与原预算保护保持。已复现的本轮准入缺陷已修，已审增量无未修复可复现P0/P1；未执行的真实资源、输入可靠性和生产部署风险仍是阻断，不能据此宣称产品安全完成。PR #4继续Draft，不合main，不放行CL-010。
