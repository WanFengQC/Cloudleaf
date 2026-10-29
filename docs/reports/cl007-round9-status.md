# CL-007 Round 9：材料绑定的只读长测准入

2026-10-30。分支`codex/cl007-windows`，起点`333dc90e0985a13f75629e29599f3df1d7d951ab`，先行设计`ed02a81`，红测提交`062f619`；本轮完整被测实现 **`eb353ae981ea891d4f84888fd7551e95e8998ca4`**。其后提交仅归档本报告和原件，最终报告HEAD与新push CI由共享Round 9汇总另核，不转绑旧CI。

## 实际缺口、设计与修改文件

R7已有只读`--preflight-output`，R8已有逐segment资源报告与中断/续跑证据，本轮不重复实现。实际缺口是旧`execution_preflight`接受isolated/approved声明、五个非空字符串和matching identity，却没有核实际回滚/授权文件、快照SHA或Windows版本/架构；旧环境表也未接入runner。[先行约定](../cl007-round9-admission.md)先提交后实施。

|文件|实际变化|
|---|---|
|`scripts/check_windows_soak_admission.py`|新增跨平台、无需native binary的只读材料检查；读取明确引用文件，核path/SHA/字段绑定，输出缺件，永不启动child或签发授权|
|`scripts/stress_windows_faults.py`|原长期预检复用checker，批准额外绑定`environment_request` path/SHA；在创建segment/child之前拒绝；原短档和完整identity检查保留|
|`tests/test_windows_soak_admission.py`|6项真实实现测试：实际pending、同一owner/operator合法材料形状、每个必需字段、缺失/改动/错绑文件、身份/档位、CLI覆盖拒绝|
|`tests/test_windows_stress.py`|新增旧仅字符串批准反例，正向fixture补临时文件，保留全部原断言|
|`docs/acceptance/cl007-round9-*.json`|pending环境、恢复演练、执行授权、runner批准、真实应用人工记录5个模板；未填写真实人员、签名或结果|
|`README.md`、设计、本报告和`cl007/round9/`|使用方式、材料合同、实际日志/身份与当前阻断|

生产Rust、COM/STA/Host生命周期、watchdog、Unknown语义、共享接口、预算及数据均未修改。没有注册/激活/安装/卸载输入法，没有独立native压力/loader、8h/24h或真实应用执行；用户要求的现有常规Rust测试照常运行。

## 材料合同及反例证据

环境需真实VM ID、Windows版本/构建、架构、快照ID/SHA和负责人/执行者。允许单一用户兼任owner/operator，不要求虚构外部观察者。恢复记录须实际文件且Hash一致、passed、执行时间及环境/快照/SHA/operator对应；执行授权同样核实际文件、批准状态/负责人/环境/快照/operator/时间和明确8h/24h范围。文件相对其引用表目录解析。

runner仍比较完整源码/数据/工具/二进制/配置/系统identity，另比较实际系统版本/架构、批准档位，以及批准声明与材料中的环境/快照/operator。新runner/source SHA会令旧批准失效，旧仅字符串批准不会自动升级。普通材料一致不证明人签或实际恢复真实性，所有检查继续`declarations_independently_verified=false`；检查报告本身`is_execution_authorization=false`，不授权TSF或授予长期验收。

[真实红测原文](cl007/round9/red/stderr.txt)为10测1fail、exit1：新增“仅字符串批准不得代替实际材料”在旧实现得到`AssertionError: True is not false`。执行时HEAD为设计提交`ed02a81`加红测工作区；[原记录](cl007/round9/red/result.json)保留完整HEAD、测试文件SHA和命令，随后同一测试提交`062f619`。临时正向fixture显式标为fixture，不冒充真实VM、演练或批准。

旧R8请求和新R9请求仍pending。新R9实际CLI检查exit2，报告10项缺件、`ready=false`、`child_started=false`、系统操作false、长期验收false，见[原始拒绝报告](cl007/round9/validation/pending-admission.json)。没有创建任何运行segment或长测运行ID。

## 全套本地回归及身份

[11项实际命令与返回码](cl007/round9/validation/checks.json)：diff/status/fmt/Clippy(all-targets)/Rust all-targets/doc/release/workspace audit/Python/固定Windows依赖审计均0，最后pending材料检查按预期2。Rust **157 passed、2 ignored**（原长期手动测试仍ignored），Python **38 passed**（旧31+本轮7）；不删除旧测试或放宽断言。文档测试无失败，release工作区构建成功。Clippy/build/test为`--locked --offline`，Rust1.90.0/Edition2024。

RustSec使用仓库既有固定cargo-audit0.22.2工具与实际15个Windows依赖许可原件，结果见[完整审计](cl007/round9/validation/dependency-audit.json)，工具SHA`f4d0d54145787f959f2ebcac8bd288ef90a86f8e512cad26b49aea321e7e1b53`，原数据库身份与警告分母保留。没有安装新项目依赖或放宽资源/许可政策。

|身份|SHA-256|
|---|---|
|实现source-v3工作区原字节|`182bc819e2a461b00cb3e075681a3ecd2e2fc668276765cf4dffee7c9f15b587`|
|新只读checker|`6abcc4e5851314803eba67a21596daee18d2a476e5d431e5979f2e6671eca6e2`|
|修改后runner|`8ac58e1bb51f133b2022e887a32c0c994cccc13bd434736861fda4164278e1cc`|
|未改资源reporter|`2b48f22e4d37cbd0bb40298067b6a4058b6f78f80a46d80e51e8b64040d936c7`|
|未改原型数据|`d4a87e958b23097253a4601b9ae373f02964e3e37729bb7f560e20835d5bb5f0`|
|release CLI构建产物|`ea27f277b39e36454c250b2bb2bddf16b02c1012b543693009e38d0166c4c4d9`|
|release DLL构建产物|`204fbb8ee725562a449ba6bcac730ab113944350e92c7a2d6ddd97fd6e73b552`|

构建日志保留Cargo实际复用情况，Hash不宣称所有产物都是新编译；未运行CLI benchmark、loader或新的性能实验。其余release文件大小/身份见[identity](cl007/round9/identity.json)。Git LF按完整实现提交追溯，与工作区source-v3原字节分列。[28份原文封装](cl007/round9/raw-artifacts.json)可还原进程原始UTF-8字节，展示规范LF不替代原件；[运行器源码](cl007/round9/runners.json)和[发布清单](cl007/round9/checksums.json)可复核。

529份起点历史报告、旧测试、R8环境/矩阵及64/41/83原型资源保持。既有reporter回归随38项Python执行，仍以实际R7中断/续跑分别呈现，不拼时长，长期接受false；未重跑独立原始trace报告或新native段。

## 单人职责与退出状态

|状态|实际结论|
|---|---|
|done|材料只读checker、runner不可绕过接入、真实旧实现反例、全套回归、pending模板及人工记录模板|
|partial|工具只能核材料一致性和SHA；真实恢复、人员及授权真实性由用户决定，长期资源/真实输入仍无验收|
|blocked|隔离环境/快照实际还原/负责人及明确授权尚未落实；不可回收Host/COM耗尽后的生产部署和真实文字恢复未实现|
|not-run|独立native压力/loader、8h/24h、TSF和真实应用验收；无新的长期性能/资源测量|

用户本人可准备真实环境、执行恢复演练并决定授权，同一人可兼负责人/执行者；Codex只检查实际材料、保全证据，不代签。R8九类应用矩阵保持，新人工记录模板状态not_run，后续必须另获TSF批准并记录真人实际输入/结果，不把自审冒充独立验收。

本轮已复现的准入缺陷已修，已审增量没有未修复可复现P0/P1；生产不可回收资源/文字恢复与真实长期行为仍是发布阻断，未执行不等于无风险。PR #4继续Draft，不合main，不放行CL-010。最终报告提交的新Windows/Linux push CI由共享R9汇总实际核对。
