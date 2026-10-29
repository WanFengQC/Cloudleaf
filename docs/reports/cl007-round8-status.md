# CL-007 Round 8：外部验收交接与只读资源趋势

**后续实际 CI 反例已修复：** 初次交付 `05dd085` 的 Windows push CI 失败、Linux 成功；根因为 checkout CRLF 改变已发布 JSONL 的物理字节。新增回归 `ffe03ee` 在旧实现真实失败后，`8d955a58518ef4f52c300fd4461c6fa5e56f2758` 仅在 CRLF→LF 候选精确匹配原记录 SHA 时解释换行，分列物理/重建身份，任意改动仍拒绝。重新执行 31 项 Python 全回归、审计、fmt 和三个只读报告通过。原 CI 失败、红测原文和新身份另存[追加证据](cl007/round8-ci-fix/checksums.json)，初次 `cl007/round8/` 的 37 文件均未改。下文原来的最终被测实现/30 项及源码 SHA 属于初次交付快照；本次修复的 source-v3 为 `0dc201a4aa9e70263639b14b317c9c9f12782472a2860eb1937dbe950c623b57`，reporter 为 `2b48f22e4d37cbd0bb40298067b6a4058b6f78f80a46d80e51e8b64040d936c7`，完整命令、返回码和数据身份见[新检查](cl007/round8-ci-fix/validation/checks.json)、[身份](cl007/round8-ci-fix/validation/identity.json)。新最终提交仍需独立 Windows/Linux push CI，不以本地通过改写原失败结果。

2026-10-30。分支`codex/cl007-windows`，起点`fa4f1fab93394f115d3a987a7f0201b8662c08cf`，本轮最终被测实现 **`9479138b9b6db62a7ff28986f7fa66602ff15a88`**。后续方向提交只归档报告，不把测量改绑到报告head。真实隔离环境、负责人、快照/恢复证明及执行授权均由用户明确为未准备；8h/24h未启动，真实应用未验收。

## 实际交付与边界

本轮读AGENTS、路线/范围、R7实际报告、runner和原始记录。R7已提供长期档/准入/流式检查点，不重复把它们列成R8新成果。[先行设计](../cl007-round8-external-acceptance.md)提交`7044ce6`后新增外部执行交接及只读报告器。

|文件|本轮实际目的|
|---|---|
|`scripts/report_windows_soak.py`|只读原manifest/逐段原件与显式stdout原文封装，校SHA、配置、进程身份/checkpoint和原始按键分母；描述逐段资源趋势及已观测预算值|
|`tests/test_windows_soak_report.py`|5项必要回归：真实短迹、实际中断/续跑独立段、OLS单位、损坏/非单调样本、原始final与缺失时长语义|
|`docs/acceptance/cl007-round8-environment.json`|真实未准备状态、环境/负责人/快照/授权/阈值空白请求；pending、所有批准字段null，不是runner批准文件|
|`docs/acceptance/cl007-round8-application-matrix.tsv`|9类真实应用待执行签收项，实际输入/版本/结果/证据/审阅者全空，不伪造验收|
|设计、README、本报告及`cl007/round8/`|目标VM检出/构建/预检和恢复交接、原始检查/失败/数据/工具/产物身份|

生产Rust、COM/Host/STA回收、DLL卸载、总预算、共享核心与`stress_windows_faults.py`均无改动。没有注册、激活、安装或卸载TSF，没有向真实宿主注键、杀宿主或探测/创建VM；没有新增原生压力档、重跑旧性能矩阵或编造长样本。

## 报告器的新逻辑与真实反例

逐段核对既有schema2 manifest、summary固定五原件SHA、采样时基/数值、原始checkpoint与进程身份、资源峰值和原始trace。已发布stdout展示可能是LF，只有匹配原记录SHA/字节数的明确`.raw.json`才可重建原件，物理展示SHA与原件SHA分列。summary缺失/中断资料仍按部分记录呈现；报告CLI exit0仅表示解释现有证据成功，不表示原测试进程或平台验收通过。

每个segment分别给出采样数、首末时刻、覆盖跨度和间隔统计；句柄、线程、working set、private commit的首末/min/max/delta和OLS单位每小时斜率；已观测Host/batch/预算、Rc/module Lease/Lock和自有IUnknown返回值；原始按键分位/分母与恢复字段。资源时基为controller `perf_counter`相对段启动秒，包含进程启动/故障准备，**不是纯稳态或单算法RSS**。每小时只是单位归一化，不预测一小时新增量；四秒记录的斜率不能用来宣布泄漏或合格，`resource_gate_passed=null`。

本轮新报告器自审发现两项可重放错误，均留真实已提交版本反例，不冒充旧生产缺陷：

- `b691e01`在初版报告上改summary缓存的native时长，真实断言得到`10.0 not less than 5`。`4b9cd0e`改为读取原始final事件并核缓存相等，避免变化的summary伪装负载时长。[原文](cl007/round8/red/output.txt)。
- `16a958b`对R7实际中断段要求未观测时长为null，真实得到`0.0 is not None`。`9479138`保留无final/无字段的null，并显式阻止其成为长期候选。[原文](cl007/round8/red/unknown-duration/output.txt)。

初版`4b9cd0e`已生成的检查和报告保留于[prior-validation](cl007/round8/prior-validation/checks.json)，不会把其0值当最终观测；最终输出和原件另存。数学小序列只验证单位，不生成8h/24h或人工批准。不同段从不累计时长，所有长期生产/真实应用接受字段恒为false；即便未来完整长段满足记录条件，也只生成可提交真人审查的候选，不自动放行。

## 本轮实际命令与原始数据身份

[9条适用命令](cl007/round8/checks.json)符合预期：Python版本、**30项Python全回归**、资源/许可检查、`cargo fmt --all --check`、diff、三个只读报告均exit0；重复输出路径明确拒绝exit2。30=R7既有25+新增5，未删旧断言。原身份回归会创建/终止自己拥有的Python子进程，不是新增原生压力、TSF或真实宿主测试。

新报告逻辑已经变化，因此纠正缺失值后重新跑针对性全Python与三个只读报告；旧原文保留。Rust/runner/锁均未改，本地没有再次跑157 Rust、Clippy/build、release COM/DLL、RustSec或旧基准来增加成绩；这些历史回归保持R7精确身份。最终新head的Windows/Linux push CI由共享方向另核，全量CI不能推断长测或真实应用完成。

以下是**原R7实测数据的R8解释结果**，不是新测量：

|R7原段|保留样本/按键|原始native持续秒|本轮解释|
|---|---|---|---|
|正常segment-0001|39 / 480|4.0329512|原件身份一致，短档，长期候选false|
|中断segment-0001|16 / 132|null|部分记录，无final，不补0，不计算完成恢复率|
|续跑segment-0002|38 / 480|4.0284962|独立新段，不能与前段拼接|
|注入失败segment-0001|10 / 18|null|child101旧故障保留，解释成功不代表测试成功|

正常段资源采样覆盖6.0196527秒（包含启动/故障准备），句柄50→145、working set 11,026,432→15,192,064B；该controller采样跨度不替代native 4.0329512秒。全部首末/min/max/斜率和间隔保留于[正常报告](cl007/round8/complete.json)，其余见[续跑](cl007/round8/resumed.json)和[原故障](cl007/round8/injected-failure.json)。全局COM引用仍unknown；Rc/Lease和成对IUnknown返回值不推断外部总数。

|身份|SHA-256|
|---|---|
|本轮源码cloudleaf-source-v3|`9e721b3153a70a42ce220e1bf11fd2b21d0490d89cfd0a79e94251b0887631e0`|
|新只读报告器|`56204d5d3314bb41eb40cb2bb0b9bf2e4a2e57c2edfd5e21df322b2a906634ec`|
|未改长期runner|`8acc7e0745d3e9ecba818d2c12dd2a99a22442a5cf1fd42da98166f6a937f1c1`|
|原型数据|`d4a87e958b23097253a4601b9ae373f02964e3e37729bb7f560e20835d5bb5f0`|
|既有release测试exe，未重建|`c7fe4b34c197871bb8d02acb37a005f93ac658800712a015a544312a7e54a0b6`|
|既有DLL，未重建|`204fbb8ee725562a449ba6bcac730ab113944350e92c7a2d6ddd97fd6e73b552`|

输入原件始终绑定R7最终`fa4f1fab93394f115d3a987a7f0201b8662c08cf`及其原生实现`e50c1d8`；新reporter/checker与原测量身份分别记录。[identity](cl007/round8/identity.json)、[原件/展示件](cl007/round8/provenance.json)、[无损UTF-8封装](cl007/round8/raw-artifacts.json)及[runner源码](cl007/round8/runners.json)可复核。source-v3按工作区原字节，Git LF与已有CRLF不冒充相同。起点465份报告和旧测试/64/41/83资源不变。

## 外部签收与仍未实现

待填包明确用户已确认VM ID、版本/架构、快照/存储SHA、恢复验证、负责人、执行授权与TSF批准均未准备。目标VM需从明确Git提交构建并自行生成8h/24h预检身份，不照搬本机exe/批准记录；8h/24h分别连续28800/86400秒，需真人固定阈值、保管原件、核身份和批准后才能独立启动。本轮没有生成任何批准或签名。

部署计划分别处理故障会话停止接管、纯值材料保留、Unknown隔离、自有工程进程退出及真人快照恢复。**生产部署控制器/用户文字确认恢复未实现，永久Host/COM耗尽仍阻断。**真实TSF注册/撤销和9类应用签收还需长期/可靠性门禁、签名位数/备份/可逆恢复及单独人工授权，不能由报告器或CI替代。

|状态|结论|
|---|---|
|已完成|外部执行/恢复清单、pending环境与9类应用签收表、逐段只读趋势和原件解释、5项必要回归及真实新工具反例修复|
|部分完成|已有工程诊断记录可审查；短段/线性斜率不能证明长期资源稳定，生产部署方案仍仅设计|
|外部阻塞|真实隔离环境、负责人、快照/恢复验证、执行批准、资源阈值与真实验收者均缺|
|未执行/未实现|8h/24h、真实应用/TSF、生产部署控制器；未重跑旧原生基准，不将旧数据解释写成新测量|

CL-007退出条件仍未满足，PR #4保持Draft，不合main、不进入CL-010。最终交付head与新CI在共享R8汇总记录；本报告不提前声明其通过。
