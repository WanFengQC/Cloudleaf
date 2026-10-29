# CL-007 Round 7：生命周期分类、长测准入与证据完整性

2026-10-29。起点`caefb7f2ac7c8584249c05359fcb174f87640886`。本轮只改变离线诊断工具及其测试，不改生产COM/Host/共享接口、预算或Unknown语义，不注册、激活、卸载TSF，不向真实应用注键或终止真实宿主。未取得隔离环境及执行授权，8h/24h不启动。

## 实际审阅和故障处置

审阅对象：`service.rs`的Factory/Service/LockServer/Lease和sink配对，`session.rs`的Host/STA/epoch/ledger/invalidate，`watchdog.rs`的全局JoinHandle目录/双worker/真实退出证明，`owner_cleanup.rs`的复制线程HANDLE，`recovery.rs`的纯值材料、HWND/class/userdata/monitor完成条件，`budget.rs`的最后owner退账，以及候选窗口回调。清理/卸载不把线程ID、is_finished或消息超时当COM释放许可。

|故障|归属与回收|恢复、用户状态与继续输入|隔离与终止条件|现有可重放证据入口|
|---|---|---|---|---|
|正常退出|owner STA撤销sink、失效epoch、销毁窗口；worker真实退出后join/退slot；最后Lease释放|材料显式处理，健康新会话可继续|无活对象/锁且清理证明齐全才S_OK|`lifecycle_review_tests`、`worker_shutdown_releases_module_leases_without_blocking_sta`|
|owner真实退出，纯值材料可用|真实复制线程HANDLE证明退出；只回收满足条件的纯资源，不跨线程Drop Host/COM|新显式恢复入口读取原epoch材料，Unknown仍未知；有预算时新会话独立输入|未析构Host/COM继续保活计费|`physical_sta_exit_keeps_data_reachable_from_new_explicit_entry_but_pins_unfinished_owner`、R5异常owner专项|
|外部COM强引用|外部持有接口，模块/Host仍存活，拒绝伪归零|停止故障会话接管；材料可用性独立检查；健康会话仅在额度允许时继续|不能替外部Release；待其正常释放，或明确关闭隔离工程进程|`lifecycle_review_tests`外部引用/LockServer、R5占用专项|
|owner永久阻塞|HWND/STA/COM仍归owner；不强制跨线程清理|watchdog检测后拒绝旧会话接管；写入前与Writing分别隔离，后者Unknown|原线程无法证明退出，额度不退；生产宿主不能被工具杀死|watchdog queued/started/writing超时和无消息专项|
|watchdog失效|observer监测primary心跳，通知有界；未知状态继续保活|MonitorFailed拒绝新接管，不能声称提交未发生|两者均失效仍须外部观测/部署策略，不声称完整可用|`observer_detects_failed_primary_monitor_and_disables_admission`、通知失败专项|
|恢复预算耗尽|全局固定/文本/Host/batch额度原值不变|新ownership/Host拒绝，保留已有材料；不能吞入无法保存的新键|不扩无限预算，不丢旧材料解锁；需要用户正常退出/受控隔离部署|`material_budget_rejects_new_context_without_overwriting_unreleased_records`、`actual_class_unregister_failure_pins_userdata_and_host_budget_until_admission_stops`|
|无法证明安全|WAIT_FAILED、占锁/构造中、窗口类注销失败、迟到callback等仍持原Lease和额度|只读显示已知状态；拒绝危险接管/重试，不把Unknown变NotApplied|证明恢复前保持隔离和S_FALSE；期限届满是验收失败，不是释放许可|`actual_wait_access_failure_proves_neither_owner_nor_exit`、`locked_or_constructing_task_remains_pinned_until_exclusive_completed_cleanup`及恢复窗口专项|

旧instance/epoch/sequence、重入和重复回调保持已有拒绝规则。用户可观察状态包括恢复窗口内容、明确未知状态、拒绝接管与HRESULT；并未实现完整生产托盘通知/部署控制器。上述工程COM宿主不是TSF真实应用验收。

## 具体工具缺口和最小修复

R6控制器的长期身份只记录OS字符串；没有机器/解释器身份，也没有隔离环境与执行授权的必填准入。R7增加只读预检：记录环境摘要，8h/24h必须读取与完整运行身份匹配的外部批准记录，包含隔离Windows环境声明、环境ID、负责人、快照、回滚计划和执行授权引用。此工具只核记录和摘要，不证明声明真实性；人工批准和环境核对仍是必要前置。`--preflight-output`只写新报告，不创建运行段、不启动子进程。当前没有批准材料，实际预检必须阻断。短诊断档不要求长期批准。

R6成功条件只检查子进程0、最后final的时间/预算和JSON解码错误，没有对比已落盘逐键/周期与final分母；合法JSON的缺键、重复或缺周期仍可被当作完整证据。先用旧`latency_summary`真实复现这些缺口，再在同一原始入口一次性核诊断协议：逐键序号、周期、会话身份与六键顺序，周期写计数/FIFO证明，唯一final与键/周期总数一致。没有最终记录的中断仍保留部分分母，不能报告完整通过。它只审核诊断记录，不能替代生产输入可靠性。

不修改Rust host/生产回收。新源/工具改变后重新冻结实现，重跑短档正常/中断/新段续跑/失败与全套检查。历史事件不重新生成或改绑身份，原R6文件保留。

## 部署级恢复与真实环境准入

进程内：故障会话停止ownership、失效旧上下文，纯值材料保持来源身份；Unknown不得自动重放。外部COM/未析构Host无法安全回收时保持计费和模块锁，预算耗尽显式拒绝；不声称新会话永久可用。

工程隔离进程：控制器只可终止自己创建且持真实句柄的测试子进程，保留日志/检查点后在新segment重新建立测试上下文；OS回收整个工程进程，不是逐个跨线程Release。段时长不拼接。

生产部署：拟采用平台层故障状态/恢复材料导出、用户确认文字状态后正常退出受影响应用或会话，再在独立身份恢复。**尚未实现该生产部署控制器/用户体验，也未证明文档事务跨进程可恢复。** TSF文档锁和COM不能被简单搬到代理进程；不可回收预算和真实文字状态未闭合前禁止发布。不得自动杀Word/浏览器、强卸DLL或代用户重放Unknown。

8h/24h分别连续28800/86400秒；固定源码/数据/工具/二进制/配置/系统身份，检查点/中断保全和新段规则不变。先验证短档与原始协议；取得隔离VM、测试账户、快照、负责人、授权记录、磁盘/电源策略及回滚演练后才可独立启动长档。当前仅交工具和待执行清单，长期实测未完成。

真实TSF准入还需连续长测、完整生命周期/故障审阅、部署恢复方案和可逆注册方案（实际DLL SHA/签名/位数/CLSID/profile、注册前备份、撤销验证、具体应用版本/矩阵/负责人），再经人工批准。此文件不是执行授权；本轮不运行注册、激活、安装、卸载或真实应用注键。

## 只读预检和待执行命令

```text
python scripts/stress_windows_faults.py --test-binary <实际release测试exe> --profile 8h --sample-seconds 1 --checkpoint-seconds 60 --output-dir <全新运行目录> --preflight-output <全新预检JSON>
python scripts/stress_windows_faults.py --test-binary <实际release测试exe> --profile 24h --sample-seconds 1 --checkpoint-seconds 60 --output-dir <另一全新运行目录> --preflight-output <另一全新预检JSON>
```

没有批准记录时，两次预检均exit2，`child_started=false`，不创建运行目录。报告的`identity`包括精确commit、source-v3、数据/工具/测试exe摘要、配置、OS、架构、CPU数量、机器名摘要及Python版本/exe摘要。即使预检记录齐全，`declarations_independently_verified=false`，仍须人工核实记录中的真实环境和执行授权。

未来获批记录为外部UTF-8 JSON：`status=approved`、`isolated_windows_environment=true`、`environment_id`、`operator`、`snapshot`、`rollback_plan`、`authorization_reference`和逐字段等于对应预检的`identity`。每个profile/配置必须分别批准；本轮不生成伪批准文件。人工签发后用`--environment-approval <文件>`重新预检；执行时去掉`--preflight-output`，仍保留同批准文件、实际exe和所有相同参数。运行manifest还绑定批准记录规范JSON的SHA，续跑若改变批准/环境也拒绝。预检和批准记录不授予真实TSF注册或真实宿主操作权限。
