# CL-007 v5：故障隔离、监测线程回收与会话可用性

2026-10-27。固定起点`573911ae07e0f4bce02f86aad16058e0e4c564e5`，Rust1.90.0；先提交本设计和故障回归，再改生产实现。共享核心/协议、候选算法、64Host/64批/64MiB文本/128MiB固定总额、32个monitor slot均不变。禁止系统注册、profile激活和真实应用注键；CL-010不启动。

## 实际缺口与资源分类

R4已把flight/store/owner receipt提早保存，但`Watchdog.retired`仍把纯Send的JoinHandle、slot、worker Lease及completion仅放在STA Host。宿主在SetText中直接ExitThread不执行Drop时，纯Signals可让两个worker停工，Retired却无法进入全局reaper；连续32个异常owner可耗尽monitor slot，即使Host/epoch总预算仍有余量，新的健康会话也不能启动。

|资源|可回收证明|缺证据时|
|---|---|---|
|监测worker JoinHandle、其slot/Lease|已提前登记、构造结束、退休请求已置位；每个真实线程HANDLE零等待返回WAIT_OBJECT_0；随后join完成|活动、构造中、WAIT_TIMEOUT/WAIT_FAILED、锁不可得/中毒均保活，不退slot|
|owner receipt/唯一窗口类/显示claim|沿用R4真实owner退出或显式owner入口、worker completion、无存活窗口类注销等证明|继续保活、记账，不跨线程DestroyWindow|
|未解决输入store|正文/原始信封可达；用户显式放弃才清除材料|Unknown绝不自动重放；不以超时或线程退出推断NotApplied|
|Host、STA Rc、外部COM接口、未析构epoch凭证|只能由合法owner正常释放最后引用、运行析构|纯值reaper不得跨线程释放；继续原额度/Lease和S_FALSE，资源耗尽仍明确拒绝|

正常线程退出不代表COM引用消失；worker回收不代表所有Host已经回收。模型只恢复可证明安全的纯Send部分，不把任意指令处退出、永久卡住或强引用永存包装成可无限恢复。

## 提前登记与回收状态机

1. `Watchdog::start`取得既有slot，创建带Mutex状态的纯Send任务，**在创建第一个worker前**用全局强Arc登记。任务内部保有Retired（JoinHandle向量、Lease、slot及finished凭证）；Watchdog只保有纯值Signals、任务Arc和completion，不成为JoinHandle唯一可达路径。
2. 构造中标志阻止reaper观察部分向量。0/1号线程创建失败时先设置stop/退休、唤醒已经创建的自有worker，再结束构造；仍由同一全局任务保有实际JoinHandle和Lease，不遗弃已创建线程。登记失败发生在线程创建前，回滚slot/Lease；登记后锁中毒保守保活。
3. `stop`只设置停止和无文本通知状态，并唤醒已登记自有worker。普通Stop不等同Drop；沿用旧测试中“worker已退出但尚未明确退休”不能先标完成的契约。正常Watchdog Drop置退休请求；R4已证明owner真实退出的入口经Signals置stop及退休请求，均不调用COM。
4. reaper对全局任务取有界Arc快照，构造未完成/退休未请求者跳过。每项使用try_lock避免等待状态锁；所有JoinHandle的真实HANDLE仅零等待。唯有全部WAIT_OBJECT_0后才消费join（worker只含已知纯Signals，不含COM/任意用户析构），置finished并释放worker Lease/slot；任务状态取走及移除只进行一次。重复stop/reap或多个reaper不能重复join/退还。
5. `JoinHandle::is_finished`只能作诊断，不能代替OS线程退出证明；WAIT_FAILED绝不授权释放。不得在TLS/DllMain中执行reap/join。全局任务本身Mutex不可得/中毒时继续持有原任务和Lease，不报告清理完成。

对象新增固定开销须纳入平台Host保守固定成本。不会为了通过32+1测试提高总预算，测试记录实际使用和剩余额度；超过不可回收Host/epoch预算时仍拒绝新ownership，不吞键、不覆盖材料。

## 先行回归与实施范围

- `render_tests.rs`新增真实自有Win32线程故障：32次写后ExitThread，正文和每个原NativeEnvelope在独立恢复视图可达且顺序/Unknown不重试；旧代码预期在健康第33会话启动处失败。修复后在这些未析构Host/epoch继续收费及S_FALSE下，健康FIFO输入独立完成，不丢键/重复提交；显式放弃材料也不能假退未析构Host账目。
- 保留真实生产NativeDrain COM强引用及其clone；旧Host停用/epoch失效后建立新Host/context并输入，再执行迟到和重复旧callback。两个文档、候选帧及原材料不被污染。强引用存在时Host/Lease保持，owner合法释放最后引用后才归还；不是把测试接口手动置空计数。
- 保留旧worker部分启动失败、completion、所有R1–R4有效断言。由于JoinHandle从Host迁到任务内部Mutex，旧测试仅调整只读取证路径，不改变判断和预期。
- 生产范围为`watchdog.rs`及固定开销接入；`recovery.rs`只消费既有owner退出信号，不改共享API。新报告、失败、回归和资源原件另存Round5目录。

## 真实应用准入与长期矩阵

准入仍拒绝：没有处理不可回收对象耗尽/故障隔离部署策略、真实输入可靠性证据、独立审核和用户批准前不注册激活。到达门禁后另行在隔离Windows用户/可回滚安装下审核记事本、浏览器表单、办公软件及多行编辑器，覆盖readonly/password、焦点/重入、同步异步拒锁、合并重复键、候选选择/取消、输入scope变更、停用/退出、DPI与候选UI等。每次验收绑定系统/应用版本、二进制/词库SHA、事件轨迹与真实文档结果。

|压力层|负载与故障|退出要求/当前范围|
|---|---|---|
|快速回归|32异常owner、外部COM强引用、迟到callback和正常新FIFO|每个原事件恰好一次材料/执行，Unknown不重试；保留剩余Host计费与S_FALSE|
|有界机制延长|同一进程180–600秒反复正常owner、类失败/恢复与显式放弃|记录轮数、四维预算、HANDLE/线程有序采样和原始日志；不称生产长期|
|长期机制|8小时/24小时，频繁激活停用、不同异常占比、预算耗尽与恢复|固定稳态资源基线、分配器/句柄线程趋势、拒绝率与恢复时长；本轮未运行则明确未测|
|真实应用长期|至少8小时自然输入/编辑、多应用焦点及睡眠恢复|不丢字/不重上屏、Unknown逐例核对、端到端延迟/内存与兼容矩阵；未准入/未测|

README完整Rust/Python、fmt/Clippy/doc、资源/依赖许可/RustSec、release loader、故障/压力及原型性能各自冻结代码后执行。所有失败和原始报告保留，测量SHA不随纯文档提交改绑；最新push双平台CI由共享汇总另核。

官方边界：[Rust1.90 JoinHandle状态](https://github.com/rust-lang/rust/blob/1.90.0/library/std/src/thread/mod.rs)、[WaitForSingleObject](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject)、[ExitThread](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-exitthread)。

## 持续不可回收占用的显式profile（实施前补充）

`088bf9c`已通过10条工程检查及157项Rust/16项Python；其原始证据保持。再给新增v5故障回归加入测试专用`CLOUDLEAF_REVIEW_PINNED_SECONDS=180..600`：先制造32个写后异常退出，再在同一进程持续保留这些Host/epoch原额度，健康新Host连续至少180秒执行六个真实生产原生事件到“你好”的FIFO周期。每周期核对六次写入、健康ledger/epoch已清、旧32Host/epoch及原材料不变；不减记、转移或提高总预算。默认无环境变量仍只测3周期，普通CI不跑长档。fixture每周期清理自身诊断调用向量/自有文档，不制造无界工程日志。

新增离线`scripts/stress_windows_faults.py`只启动自己的已构建测试进程；Win32句柄/线程、GetProcessMemoryInfo的当前working set/OS记录的进程峰working set及当前private commit分别按时序记录，不能把采样最大值当OS精确峰或把commit当物理RSS。记录固定SHA、实际原始命令、环境变量、开始/结束与每个检查点，末值与冷启动不直接相减推泄漏。此profile是有界异常占用机制证据，8h/24h、真实应用和物理内存容量门禁继续未完成。
