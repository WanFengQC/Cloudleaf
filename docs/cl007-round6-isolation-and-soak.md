# CL-007 Round 6：进程隔离、长期诊断和真实TSF准入

2026-10-28。起点`7f581dfca43515105fca918ffd79e55f315f7729`。先设计/失败测试，后实施工程host及离线控制器；不修改共享接口，不注册/激活输入法，不把可运行8h/24h档当成已跑完。

## 实际生命周期与故障边界

`service.rs`的Factory/Service、LockServer及Lease共同阻止卸载；`DllCanUnloadNow`只请求零等待watchdog/owner清理，OBJECTS或LOCKS非零即S_FALSE。Host只在owner STA访问Rc/COM，Drop调用stop_platform，将未确认输入交给恢复材料；watchdog任务已在创建worker前登记全局纯Send Arc。recovery全局receipt保存纯值/线程证明，不把TLS弱Host变成跨线程强引用。正常owner完成窗口清理或真实线程终止、monitor退出、无回调且类注销成功后才归还纯资源。无法证明安全的COM、Host、epoch、未知锁继续保活，不以放弃材料代替销毁对象。

进程内恢复先停止该会话接管、隔离旧instance/epoch/sequence，Writing或写后退出仍为Unknown，禁止自动重试。新会话有独立身份和预算；不可回收额度累积到上限仍拒绝接管。进程级最终回收只能由**拥有该隔离测试进程的控制器**结束整个进程，操作系统回收该进程的资源；不能杀死Word/浏览器/记事本来恢复输入，也不能在生产TSF中把COM挪到另一个进程后假定文档锁仍有效。生产部署级进程隔离/代理协议及文档事务恢复尚未实现。

R5控制器实际只接受180–600秒，事件/样本全存在内存且退出才写summary；控制器中断会丢掉资源采样，直接延长24小时还会累积工程内存。R6仅解决这项可重现的诊断缺口。

## 长期控制器契约

保持原`--seconds 180..600`有界入口；新增`--profile 8h/24h`固定28800/86400秒，及明确不具验收资格的`diagnostic`短档。长期默认每秒一个六键FIFO周期，避免无限速测试造成数十亿事件；原有默认三周期和有界档行为保留。仅诊断档允许显式注入中断/失败，不能偷偷缩短8h/24h门槛。

每次运行固定源码commit/fingerprint、工具/实际Cargo测试exe/原型数据SHA、profile/秒数/周期与采样/检查点参数。manifest首次创建后不改；每个segment独立目录保存stdout、逐键原始事件JSONL、资源JSONL、检查点JSONL、最新原子checkpoint和最终summary。检查点先flush/fsync原始文件再同目录replace；检查点按实际周期生成，不能把checkpoint覆盖当历史证据丢弃。summary只保留有界统计与文件SHA，不把长期所有记录留在内存。

Ctrl+C、受控中断、宿主非零退出、超时和取样错误均保存已取得证据并标失败/不完整。只终止自己创建且仍持真实句柄的测试子进程。续跑必须验证manifest全部身份一致，并用OS进程句柄和创建时间证明先前控制器/child已结束或PID已复用；无法证明时拒绝，不按裸PID杀进程。续跑创建新segment、重新制造独立测试上下文，不把Unknown重放进新进程。中断前后时长不能拼成连续8h/24h，最终完整门槛必须由一个segment独立满足。

## 原始审计与测量口径

工程Rust host仍调用生产native/COM写入路径，在同一进程保留32异常owner原预算，逐周期检查六次写入、FIFO、空ledger及预算不变。新增测试专用事件带真实instance/epoch/sequence、VK/LPARAM、每键接管耗时与到本周期写入确认耗时；只记录自编“nihao 空格”，绝不采集用户文本。保存周期检查点及末尾32个Unknown核对/明确放弃，恢复成功分母与未执行尾部独立列出。

采样记录句柄/线程/current working set/OS peak working set/private commit。Host的Rc strong_count、模块OBJECTS/LOCKS和对自有诊断IUnknown成对AddRef/Release的返回数分别命名；都不能冒充全局真实COM引用数，外部COM总数保持null。成对调用仅owner STA、仅诊断对象，净引用不变；不跨线程Release。延迟分位数由落盘逐键样本计算，注明工程host/节流配置和完整分母，不能外推真实TSF输入。采样失败不算零资源；不把采样峰或commit当精确RSS。

## 真正TSF验收准入与注册草案

当前准入保持blocked。放行前必须有：独立审核的生命周期/unsafe边界与故障退出策略；同精确源码/二进制/数据身份的连续8h及24h机制证据；全部旧回归和故障失败分母；无未知材料自动重放；预算拒绝/COM永久引用的恢复与部署策略；独立可回滚Windows VM/测试用户、系统/应用版本及快照标识；签名/位数/安装撤销与恢复路径；只读/密码scope与真实应用矩阵和人工监控责任人。

仅全部门禁通过后才提交具体DLL/CLSID/语言profile、测试账户和机器、临时注册范围、快照/撤销步骤、运行时长及负责人的注册方案供人批准。**本轮只交受阻草案，不提供已批准或已执行状态，不调用注册/激活API，不操作生产注册表。** 真实应用包括记事本、浏览器、办公软件、多行编辑器，覆盖组合/选词/取消/退格/焦点/重入/迟到回调与Unknown核对；自建COM测试不能替代。

范围：`scripts/stress_windows_faults.py`、`tests/test_windows_stress.py`、`adapters/windows/render_tests.rs`和只读测试计数入口、README及新Round6报告。生产COM/资源回收语义和默认输入引擎不改。全套Rust/Python、原专项/依赖审计和短档真实中断/续跑/故障后证据重建之后，绑定最终推送SHA另验Windows/Linux CI；8h/24h未跑完必须明确未验收。
