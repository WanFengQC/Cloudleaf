# CL-007 Round 8：外部执行交接与离线趋势

2026-10-30。基线`fa4f1fab93394f115d3a987a7f0201b8662c08cf`。本轮不改生产Rust、COM/Host回收、共享接口、预算或长期runner；不注册/激活/安装/卸载TSF，不向真实宿主注键，不启动未获批8h/24h。

## 实际缺口和有界交付

R7已提供固定长期档、身份/批准预检、原始事件/资源/checkpoint流和严格新段续跑，不能重复将它们列为R8新功能。实际缺口是：外部执行者拿到这些文件后，尚无只读报告器对原始资源序列计算逐段趋势、核对原始文件与最终记录，也缺少可逐项签收的部署/恢复交接清单。

新增`report_windows_soak.py`读取一个既有run目录。入口核manifest、每segment的配置/进程身份、摘要中固定文件SHA、最后checkpoint及JSONL记录；重用原有trace审核。摘要缺失/中断材料继续展示部分结果，损坏记录明确列issue。只处理原目录固定文件，不启动进程、终止进程或操作系统注册。已发布stdout展示件若规范LF，只有显式`.raw.json`原文封装匹配被记录的SHA/字节数才可重建原件，展示SHA与原件SHA分列。

每个segment单独输出：原始样本数、起止采样时刻/覆盖、间隔中位数与最大值、句柄/线程/working set/private commit首末/min/max/delta和OLS每小时斜率、已观测预算/Host Rc/module Lease与自有IUnknown返回值、原始按键延迟分位/事件分母、恢复字段及文件身份。采样时基是runner相对本进程的`perf_counter`秒，包含启动/故障准备阶段；不是仅稳态，不是单算法RSS。斜率只是现有样本的描述，不能独立证明泄漏或判资源合格。

连续时长只显示每段自身的native `steady_elapsed_ns`；不同段不相加，不将controller wall time当输入负载时长。8h/24h候选证据必须同时具备同一段固定配置、足够native持续时间、原始trace完整、摘要已通过及绑定批准记录；即使全部满足，报告器也只标“可提交外部审查”，不自动授予长期生产或真实TSF验收。短档、缺材料、未知授权和资源阈值未冻结时继续不放行。

只对新增计算/核身份写必要测试：真实R7短段重放、同身份续跑分段、不完整段、显式损坏拒绝和已知小序列OLS单位。夹具只验证数学，不扩成长期样本，不伪造人工批准。既有生产代码未改，不重跑旧性能矩阵或长期/模拟验收来增加成绩；运行适用Python全回归、资源审计和格式检查，新HEAD仍需Windows/Linux push CI。

## 外部执行包与签收责任

交接以精确Git检出和目标VM构建为单位，而不是把本机exe加一个脚本就声称可移植。保留源码/数据/工具/目标机二进制的各自SHA。外部执行者在目标VM自行构建后生成8h/24h分别的预检身份，负责人核真实隔离条件后签发匹配的批准记录；本轮只提供空白pending表，不代签。

1. **环境负责人**交付隔离Windows VM标识、版本/位数、专用测试用户、快照、可用磁盘、禁休眠/电源策略、宿主应用范围、恢复演练结果及授权引用；原件位置登记到pending表。任何项缺失不启动。
2. **执行者**记录精确checkout、Rust/Python版本、DLL/test exe/runner/数据SHA，locked/offline构建；先运行只读`--preflight-output`，核完整身份，取得分别绑定8h和24h的批准文件。源码/配置/机器/二进制改变即重新预检和批准，不改旧文件。
3. **观测者**检查原始JSONL和周期checkpoint持续写入、磁盘余量及独立时钟；出现OS/runner异常即保全当前目录。只能按已批准范围停止自有工程进程；真实Word/浏览器及TSF宿主不由脚本杀死。
4. **中断后**从原checkpoint证明旧runner和child已退出后才能`--resume`创建新segment。新段不补原段连续时长；Unknown材料保持未知，不模拟按键重放。原始目录先复制到证据保管位置并核SHA，再人工决定恢复快照/重试，禁止用清目录“归零”。
5. **审阅者**运行本轮离线报告，检查每段分母、时长、资源趋势和异常证据，按照预先登记的资源阈值人工判定。没有独立阈值/长期实物就没有资源放行。保留失败全分母，不从整体趋势删除不利区间。

获批后的诊断命令沿用R7 runner：

```text
python scripts/stress_windows_faults.py --test-binary <目标VM实际release测试exe> --profile 8h --sample-seconds 1 --checkpoint-seconds 60 --output-dir <全新8h目录> --preflight-output <全新8h预检JSON>
python scripts/stress_windows_faults.py --test-binary <目标VM实际release测试exe> --profile 24h --sample-seconds 1 --checkpoint-seconds 60 --output-dir <全新24h目录> --preflight-output <全新24h预检JSON>
```

本轮不提供一键启动/注册脚本。未来获得真实批准后，使用R7规定的`--environment-approval`先重新预检，再在同身份参数下执行；CLI记录一致不证明环境和签名真实性。

## 部署故障与真实应用准入

永久COM/未析构Host/owner阻塞仍保活计费；额度耗尽停止新ownership，保留原纯值恢复材料。Unknown不改为NotApplied、不自动重放，旧context不搬到新应用。生产部署控制器、用户文字确认/导出及不可回收宿主恢复仍未实现；“用户正常关闭应用或退出会话”只是待验收操作方案，不是已实现自动恢复。

自有工程进程可在批准范围内由runner终止并由OS回收整个测试进程，这不证明可跨线程Release或强卸生产DLL。VM快照恢复由真实环境负责人按批准方案执行并记录前后状态；本轮不自动调用虚拟机或注册命令。

连续8h/24h、可靠性和部署恢复门禁完成后，才可另行申请隔离环境TSF注册/实际应用验收。注册前必须固定签名/位数/CLSID/profile及备份/撤销方案并人工批准。应用矩阵至少覆盖记事本、浏览器文本框/地址栏、办公软件、多行编辑器、只读/密码控件、管理员切换、多屏DPI；每项原始输入、选词/取消/退格/失焦/异常提交结果均由真人记录，不用工程host、静态检查或CI代替。

当前环境负责人、VM/快照、授权及真人审阅记录均缺，8h/24h与真实应用保持未执行。模板见`docs/acceptance/cl007-round8-environment.json`和`cl007-round8-application-matrix.tsv`。

## Windows checkout 的字节解释修订

`05dd085` 的 push CI `40913130729` 在 Windows 出现真实反例：Git checkout 将 `checkpoints.jsonl/events.jsonl/samples.jsonl` 的 LF 转成 CRLF，Linux 同提交通过，Windows 30 项 Python 中两项因物理 SHA 不符失败。只读报告器此前仅处理 stdout 的显式原文封装，缺少已发布 JSONL 的 checkout 解释。

固定五文件中的上述三个 JSONL 若物理 SHA 不匹配已记录 SHA，只尝试一次 CRLF→LF；只有转换结果精确匹配原记录 SHA 才解释为 checkout 换行。物理 SHA/字节数始终保留，重建 SHA/字节数及转换类型另列。未给预期 SHA、其他文件、字节改动或转换后仍不符均不借此接纳；不改变数据内容、历史证据或 runner。新增真实 checkout 形状回归并保留旧实现失败原文，原非单调/损坏回归保留。
