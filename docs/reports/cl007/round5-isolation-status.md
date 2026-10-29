# CL-007 Round 5：v5故障隔离、纯监测资源回收与异常占用下的可用性

2026-10-28。起点`573911ae07e0f4bce02f86aad16058e0e4c564e5`；最终被测源码冻结于**`519ea587fe5a0fe56cb2a5ebc239832a118f692a`**，`cloudleaf-source-v3` SHA-256 **`8151e6aa5ff2ac10fc46b373d44d8b92f44e2c4864444965c4e5058fa139795a`**。本报告及入口为后续纯文档提交，不把测量改绑到报告head；最新push CI由共享Round 5汇总绑定最终发布SHA。没有注册或激活系统输入法，没有真实应用注键，不进入CL-010。

## 设计与具体修改

[v5约定](../../cl007-fault-isolation-v5.md)先于实现提交`263875e`；`c03e3c6`先提交真实32异常owner/外部COM回归并在旧生产代码得到失败，`088bf9c`才修复。持续占用profile先提交`6f85efb`设计，再提交`ad45bfe`测试及自有进程采样。两项旧测试的观测前提分别由`32cf37b`、`cc3363c`先行记录，再修改测试；生产逻辑未为这些失败放宽。

| 文件 | 本轮实际变化 |
|---|---|
| `adapters/windows/watchdog.rs` | 在首个worker创建前全局强Arc登记纯Send任务；Host不再唯一持有JoinHandle。构造结束、退休请求、独占任务及真实OS线程HANDLE全部signaled后才join并退slot/Lease；部分创建失败走同一退休流程，重复回收不重复退额。 |
| `adapters/windows/recovery.rs` | 将新增任务/原子/句柄及分配开销纳入既有固定计费，不提高全局额度。 |
| `adapters/windows/render_tests.rs` | 32个自有Win32 owner写后ExitThread、旧Unknown可达、新会话FIFO与外部真实COM引用隔离；新增有界持续占用档。保留旧断言，等待真实清理失败观测及显式维持容量测试scope前提，逐步断言实际入队长度。 |
| `scripts/stress_windows_faults.py` | 只启动自己的工程测试进程，保存原命令/环境、二进制SHA、事件和Win32句柄/线程/进程内存时序；不注册、不注入其他应用。 |
| `docs/cl007-fault-isolation-v5.md`、本报告、`round5-isolation/`、README | 资源分类、退出证明、准入/长期矩阵、完整失败/回归/压力与身份原件。 |

`JoinHandle::is_finished`不能充当OS线程结束证明；`WAIT_TIMEOUT`、`WAIT_FAILED`、构造中、锁不可得/中毒均保持任务和原计费。TLS/DllMain不执行reap/join。未析构Host、Rc、COM及epoch不能通过纯值reaper跨线程释放，继续S_FALSE和原额度；Unknown不因退出或超时变成NotApplied，也不自动重放。

## 可重放检查、真实失败与原始证据

Windows11 build26300、x64/i5-11400/12逻辑处理器，Rust1.90.0/Edition2024。精确命令和返回码见[全检查](round5-isolation/summary.json)、[release专项](round5-isolation/diagnostics.json)、[证据身份](round5-isolation/evidence.json)。默认并行Rust **157 passed**，其中原生97、默认ignored专项2；Python **16 passed**；locked fetch、offline debug/release、fmt、Clippy `-D warnings`、doc tests及资源审计均通过。

依赖审计15外部包及31许可证原件通过，cargo-audit0.22.2、锁SHA`3c021bd45f9ff3dd89776deccc2f80d8f9b18401abdc1eaeb531ce92c072c042`；RustSec1381条、数据库`40f5be6fb12ed93fc591bf4ab23b7d793b3f2239`，0漏洞/0警告。[审计原件](round5-isolation/dependency-audit.json)。这是该数据库快照的结果，不替代unsafe与应用兼容审核。

所有失败均保留：[失败索引](round5-isolation/failures.json)与[逐字原输出索引](round5-isolation/raw-logs.json)。`.raw.json`的`raw_text`及`original_sha256`保留原始UTF-8字节，可编码回原件核验；同名`.txt`仅供阅读，尾随空白清理另记显示SHA，不能拿显示副本冒充原件。结构化报告以LF保存以匹配Git对象；[产物原件索引](round5-isolation/raw-artifacts.json)分别列原始与发布SHA，发生CRLF转换的报告同时保存`.original.raw.json`逐字原件。

- `c03e3c6`真实red：32异常owner后固定计费仍低于128MiB，却因旧Host独占已停worker JoinHandle导致32slot耗尽，第33健康会话E_FAIL；exit101。修复未增加slot或总预算。
- `ad45bfe`完整检查失败：R4类清理失败断言在一次零等待reap后看到attempts=0。测试现在在原3秒界限内等待实际四个receipt失败尝试，再保留原错误、owner退出、解除注入及额度归零断言。
- `9ba0ea2`完整检查失败：旧容量测试overflow返回true，未记录中途队列长度。未满阶段scope失效会同步排空是源码解释，**不是已捕获的根因**。新测试显式维持有效scope并新增每步长度断言；真正满32仍在生产scope分支前拒绝。旧独立满队列+过期scope回归继续通过。
- `519ea587`冻结后完整并行检查与全部9项release专项通过；早期`088bf9c`及`ad45bfe`成功/失败原件各自保留，不改绑SHA。

116个起点历史报告Git对象与64条词典/41条事件/83条种子数据保留，见[完整性](round5-isolation/history-integrity.json)。所有新证据的提交字节SHA见[校验表](round5-isolation/checksums.json)，表不包含自身。

## 异常预算持续占用、COM隔离与资源结果

同一自有隔离进程先制造32个写后`ExitThread(77)`，不运行Drop。原instance/epoch/sequence及Unknown逐条仍可由新会话显式查看，各旧文档一次写入、无重放。已证明退出的monitor worker被回收，原32Host/32epoch继续收费：text41,943,040B、fixed118,838,080B；策略仍为64Host/64批/64MiB文本/128MiB固定，slot32。

随后同一进程健康会话持续**180.000092秒**执行**1,017,364个六事件FIFO周期/6,104,184次生产SetText调用**。每周期核对“你好”、空ownership ledger/epoch、写次数及固定预算完全相等。旧32Unknown在末尾各只出现一次，写次数不增；显式放弃后材料目录为空，未析构Host与epoch仍各32，DLL保持S_FALSE。该结果说明已安全回收monitor slot后的有界可用性，不证明任意异常Host可回收，也不是系统输入法吞吐量成绩。[原始profile](round5-isolation/pinned-owner-stress/summary.json)

profile采集622个带时间戳样本；采样循环休眠200ms，工具枚举本身另有成本，不能视为精确5Hz。10–60/60–120/120–181秒区间句柄均144；后两段线程均4、working set15,687,680B、private commit5,681,152B。全程采样峰句柄147/线程7/working set16,343,040B/private commit5,812,224B。报告另有OS记录的PeakWorkingSetSize；采样最大值不是精确峰，private commit不是RSS；初始化与结束差值不证明泄漏或无泄漏。测试二进制SHA **`0c10c7a15b2aabed93d6d4b180624e96d39e87e7b64ca8dfe61733ce74bf388e`**。

外部COM专项保留实际NativeDrain及clone，旧Host失效后新context完成6写，三个迟到/重复旧调用均不写旧文档、不污染新上下文；最后合法owner释放COM后四维额度归零。该用例不证明外部永久持有引用也能卸载。[原生专项](round5-isolation/native-round5.txt)

release真实DLL599,552B，SHA **`63028284a49dbbbe612edbbbaa93e24e4abf306a558c3cc42f0c4ca62c398af5`**；真实LoadLibrary/类工厂配假manager的1预热+20循环句柄113→113，通过卸载门禁，不代表真实TSF宿主。[产物](round5-isolation/dll-identity.json)、[loader](round5-isolation/real-dll-loader.txt)。36批既有压力最高hosts37、fixed133,813,610B，达到固定维度后拒绝新ownership并保留原材料，显式释放后归零；计费仍是保守成本模型，不是OS物理堆上限。

模拟COM keyedit/候选提交P95为57,900ns/36,400ns；64条手写原型事件P95为5,000ns，原型宿主OS峰6,033,408B。原始分位数、工具链、数据/源码/宿主二进制SHA保留于[原型报告](round5-isolation/core-performance.json)及[模拟COM原件](round5-isolation/simulated-com-benchmark.txt)。两者均不属于正式词库、真实Windows应用或语言质量评测。

## 剩余阻塞、未实现与重放

1. **部分完成**：能证明退出的纯监测worker/slot回收、指定32异常owner后的材料可达与健康输入、真实COM引用隔离已有机制证据。**仍阻塞**：任意指令异常退出、永久卡住/锁中毒、外部永存引用及未析构Host的部署级隔离/重启策略。累积不可回收额度后仍明确拒绝，不能承诺无限恢复。
2. **未验收**：8小时/24小时机制压力、真实应用至少8小时自然输入和睡眠/焦点/兼容矩阵；180秒并未关闭长期可靠性门禁。真实应用准入仍拒绝，不注册/激活/发布。
3. **尚未实现或未验收**：真实记事本/浏览器/办公软件/多行编辑器输入，跨架构安装/签名/升级卸载，完整readonly/password/DPI/宿主生命周期；macOS/Linux系统适配、正式语言模型/纠错/简拼/双拼/学习及翻译不在本分支实现。未接入CL-008或CL-009，不改共享契约/默认候选算法。
4. 保持PR #4 Draft；不合并main，不进入CL-010。最新最终head的Windows/Linux **push** CI需在共享Round 5汇总单独核对，不沿用起点或本地成功。

重放先执行README全套及`summary.json`/`diagnostics.json`命令；release loader显式提供本次真实DLL。持续档用Cargo `--no-run --message-format=json`返回的release测试exe执行：`python scripts/stress_windows_faults.py --test-binary <真实测试exe> --seconds 180 --output-dir <新目录>`。脚本拒绝覆盖目录，只对自己创建的子进程设置环境，全部参数见profile原件；普通CI仍跑默认三健康周期。
