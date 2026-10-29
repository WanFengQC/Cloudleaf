# CL-007 Round 4：owner退出、恢复材料与有界压力

2026-10-24。代码检查点 **`5c0ba9d651038eca5b3fcb53dba3f6835cf13a0c`**；源码指纹 `cloudleaf-source-v3 / 88fcfefd694709e4304108b9001d00fc86e306f67755ab95381d84cfbccf3027`。本分支仅推进Windows适配；没有注册系统输入法、激活profile、向真实应用注键或进入CL-010。A1–A4仍是部分实现，A5未放行。

设计先行：`dddb693`（纯值receipt）、`cc05623`/`9fcdc91`（真实线程证明及等待失败）、`9a3712b`（异常退出前flight快照）、`a8a0753`（180–600秒显式压力档）、`b4555cc`（满队列优先于scope回退）。[owner receipt约定](../../cl007-owner-receipts-v4.md)与[线程证明](../../cl007-owner-proof.md)限定实现和退出条件。源码实现先于本报告冻结；报告提交不改这些代码，最终发布head及其精确push CI由共享Round 4报告单独绑定，不沿用旧CI。

## 具体变更与实证

| 文件 | 实际变化 |
|---|---|
| `adapters/windows/owner_cleanup.rs` | 复制当前真实线程句柄，Arc/OwnedHandle保活；仅零等待明确signaled才证明退出。等待失败既不证明退出也不授权owner操作；不按可复用ID重新查找。 |
| `adapters/windows/recovery.rs` | 全局强Arc receipt保有纯值userdata；弱Host hook仅在owner TLS。Drop只标退休。真实退出及watchdog完成后，注销无窗口的唯一类并释放显示claim；不跨线程销毁HWND或释放Rc/COM。显式discard入口先回收已退出reader的凭证。 |
| `adapters/windows/session.rs` | 启动即绑定completion/monitor；所有权后、外调前保存flight。StartComposition/SetText前标Writing，确认Applied后才删除；正常移交成功才移除同epoch快照。满队列先拒绝，随后才考虑失效scope的同步回退。 |
| `adapters/windows/watchdog.rs`、`mod.rs` | 接入线程证明；owner已真实退出后只发送原子停工信号，未析构的watchdog/Host仍保活计费。 |
| `adapters/windows/render_tests.rs` | 真实COM STA退出/新STA读取、类失败与恢复、原始信封不丢不重、自有OS线程写后ExitThread、明确scope失效且队列已满回归；保留全部旧断言。 |
| `scripts/stress_windows_owner.py` | 显式离线工程压力与自有测试进程句柄/线程采样，不进入生产输入路径。 |

正常STA证据覆盖实际COM初始化/撤销、线程join、Unknown写后失败及新STA显式查看。旧owner已退出，唯一类注销成功后Host固定额度可以归还；Unknown批继续独立保活，明确放弃后四维预算归零且`DllCanUnloadNow=S_OK`。旧第三轮“未完成owner必须保活”断言保留，并明确注入类清理失败；另增无故障真实回收用例，未把旧预期直接改为成功。

异常退出故障在**本测试自有隔离子进程**中的两个自有Win32线程执行：异步FIFO和同步fallback各一次，在实际测试宿主`SetText`写入后`ExitThread(77)`，不运行Rust Drop/COM撤销。实际OS线程句柄证明退出；新STA可读取原instance/epoch/sequence、VK/LPARAM/Key和计划文本；写入中事件为Unknown，其后尚未执行的异步事件为NotApplied。两例各仅一次写入，之后无自动重放、无读取死锁。

此异常故障**没有**证明任意强COM对象可以回收：明确放弃后材料目录为空，但两个未析构Host及两个epoch reservation仍保活，DLL保持S_FALSE。新测试最初错误期待batch计费归零，已分开“目录为空”与“同一reservation仍被Host持有”的断言；未跨线程释放或重置生产计数制造零值。[原生Round 4输出](round4-owner/native-round4.txt)

## 本地检查与失败

Rust1.90.0/Edition2024，Windows11 build26300，x64/i5-11400/12逻辑处理器。全部命令、返回码、原始输出、数据/二进制身份见[证据索引](round4-owner/evidence.json)、[完整检查](round4-owner/summary.json)及[专项](round4-owner/diagnostics.json)。

| 检查 | 本检查点结果 |
|---|---|
| locked fetch、offline workspace debug/release、fmt、Clippy `-D warnings` | 通过 |
| 默认并行workspace全target | **153 passed**，0 failed；含原生93，2项专项默认ignored |
| doc tests、资源/依赖政策、Python unittest | 通过，Python **16 passed** |
| 固定cargo-audit0.22.2与精确许可证原件 | 15外部包、31许可证；RustSec1351条，0漏洞/0警告 |
| 新release DLL真实loader | 1预热+20轮，通过；句柄 **113→113**；假manager不代替真实应用 |
| 恢复故障、R3/R4原生用例、36批预算压力、模拟COM基准、原型与83种子回归 | 全部通过；完整失败分母和原始结果保留 |

RustSec数据库为`84d943a6deeb8cc6c6f632531aaf4f1c62275239`，锁SHA-256 `3c021bd45f9ff3dd89776deccc2f80d8f9b18401abdc1eaeb531ce92c072c042`。[审计](round4-owner/dependency-audit.json)

失败均保留于[失败清单](round4-owner/failures/index.json)，没有删除旧测试或跳过失败断言：

- 未提交准备阶段：新ExitThread故障最初占用既有mode10，撞上旧EndComposition重入用例，触发COM边界不可展开panic；改为独立常量11，旧用例保留。首次新测试误把显式放弃材料视为Host共享reservation已经销毁，现改为分别证明目录清空和预算保活。
- 未提交准备阶段：新reader真实退出后，其显示token仍在全局receipt，原owner直接discard时旧R3的`batches=0`断言失败；修复显式owner入口先reap，保留旧断言。
- **冻结`78f08be96edcc2d5e03169b75c1850308f293cd6`完整检查真实失败**：满队列时100ms scope已过期，旧次序先同步barrier排空，再接管第33键。此为ownership次序缺陷；新设计和`5c0ba9d`把满队列检查放在scope分支之前，新增确定性过期scope回归，不延长生产scope、不绕过旧断言，重新完整并行检查通过。
- 工程命令准备中的包名拼写与PowerShell引号错误另列，未作为产品测试结果。准备阶段未提交源码不伪称已有精确commit；保留原始堆栈和对应修复检查点。

## 压力、产物和边界

当前平台仍使用固定**64条手写原型词典**。真实x64对象Host1288B、OwnedEvent264B、Candidate104B；新receipt/UI固定成本9032B，原epoch文本/固定预留仍为1,310,720B/3,592,832B，token另加88B；模块策略仍为64Host/64批/64MiB文本/128MiB固定。36批压力最高hosts37、fixed133,797,256B，达到固定维度后拒绝新ownership，原材料不变；明确逐批释放后归零。它是可达材料及副本的保守计费，不是OS物理堆的精确上限。[压力原始结果](round4-owner/production-pressure.txt)

默认机制压力20轮×4个真实COM owner，半数轮注入类清理失败，解除故障后每轮回零。显式180秒压力的实际轮数、过程检查点和句柄/线程初始、采样峰值、末值见[连续压力](round4-owner/extended-owner-stress/summary.json)。200ms快照只描述采样所得峰值；进程初始值包含冷启动阶段，不能用初末差自动判定泄漏或泄漏为零。每轮四维计费与DLL门禁逐轮断言，不依赖采样。三分钟仍是有界机制验证，**长期真实应用压力未验收**。

release DLL593,408B，SHA-256 **`d253221575cc1e14a88e8f4339e16e0645431e4b4f925d1a6eff8f86510df1ac`**。[产物身份](round4-owner/dll-identity.json)与[loader](round4-owner/real-dll-loader.txt)。模拟COM的keyedit/候选提交P95分别59,800ns/35,600ns；64条原型核心事件P95为5,400ns，均有原始输出和环境指纹。这些不是系统TSF端到端、正式词库或语言准确率成绩。

## 剩余阻塞与未实现

1. 不执行析构的异常退出仍可能永久保有Host/COM/JoinHandle及预算；此轮证明关键纯值输入材料可达、无盲重试，**没有**关闭任意异常退出的资源回收门禁。累计后新Host/事件会被预算拒绝，仍有可用性影响。任意指令处退出、永久卡住、外部强sink/callback或锁无法取得的情况保持保守保活，不能声称资源已经释放。
2. 正常owner退出与类清理可恢复路径已有机制证据；持续User32失败、宿主永久阻塞、未撤销COM引用仍明确阻塞。真实应用中的焦点竞态、密码/readonly、消息合并及全生命周期矩阵仍需验收。
3. A5未开始：没有系统注册/profile激活、记事本/浏览器/办公软件/多行编辑器测试；不放行正式注册或发布。跨架构、安装/签名/升级卸载、长时间真实应用输入仍未实现或未验收。
4. 本分支没有接入CL-008实验搜索或CL-009词典，没有正式语言模型、纠错/简拼/双拼/学习、翻译或macOS/Linux系统适配。不能以平台机制测试替代这些能力。

Round1/2/3报告与64/41/83资源不改写；[历史完整性](round4-owner/history-integrity.json)记录起始`bb6aea1`的Git对象逐项对照。新证据文本采用UTF-8/LF，保留输出尾空格和空行，[校验表](round4-owner/checksums.json)覆盖提交的原始证据。PR继续Draft，main不变，不进入CL-010。

## 重放

先执行README全检查，再按`summary.json`/`diagnostics.json`的精确参数运行release专项；`CLOUDLEAF_REVIEW_DLL`必须指向本次实际构建DLL。只给单个loader设置`CLOUDLEAF_LIFECYCLE_REVIEW_CHILD=1`，不在全套并行时设置。

延长档先用Cargo JSON消息取得本检查点的真实测试二进制路径，随后执行：

```text
python scripts/stress_windows_owner.py --test-binary <Cargo返回的release测试exe绝对路径> --seconds 180 --output-dir <新的证据目录>
```

脚本拒绝覆盖已有目录；持续180至600秒才是显式延长档。具体测试命令、选择的二进制SHA、只限本子进程的环境变量和原始采样包含在连续压力报告，普通CI不默认执行长档。
