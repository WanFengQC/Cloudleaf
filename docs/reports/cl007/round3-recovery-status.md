# CL-007 第三轮：模块恢复预算与窗口所有权

2026-10-23；实现提交 `77cbbd62969629ae2c7f5eb349c3cdacd4fd2d4f`，源码指纹 `cloudleaf-source-v3 / 7c068d7da55788c391bac6a950e56928a631c303d16da8e174eb22b8ab2460a4`。本轮仅修改 Windows 平台层；原型词典、候选算法及 main 不变，没有注册、激活 profile 或向真实应用注入按键。A1–A4 仍为部分实现，A5 未放行。

历史[第二轮报告](async-ownership-status.md)、原始数据和失败记录全部保留。本轮设计先行提交为 `d3794ac`、`707d252`、`5d1bcc0`、`603de28`；接口与规则见[模块预算](../../cl007-recovery-budget.md)及[恢复生命周期](../../cl007-recovery-lifetime-v3.md)。本轮所有命令、原始输出和校验集中于 [round3-recovery/](round3-recovery/evidence.json)。

## 已实现与实际调用路径

1. 生产 `Host`、活动/退休恢复资源与独立材料 store 共用四维原子预算。首个 epoch 原生 ownership 在投影改变之前预留完整32事件容量；Applied 全部完成才释放，移交 store 和显示副本共享原 reservation。Unknown、窗口外毁、失焦或停用不自动放弃。
2. 同步 fallback、候选 Finalize 和取消同样在实际写锁内确认 scope 后取得预算，预算不足时不改变核心、不吃原生键或执行新写入。原生同步 fallback 保留原始 instance/epoch/sequence、w/l、Key、修饰键和布局；UI动作明确标注平台来源。写后失败保留 Unknown 及“可能已写入”的计划目标，不把它作为未应用文本重试。
3. 恢复正文与 HWND 解耦。主窗口外部销毁不清 store；独立无原文入口允许显式重建并再次点击查看。后续可用 owner 存在全局未解决材料时也显示无原文入口，用户可以查看其他实例、复制或明确放弃。来源 epoch 不改为新上下文，已放弃 epoch 的迟到交付不能复活。
4. 同一 store 同时只允许一个正文视图；正文与全部 reservation 在同一 Mutex 内取得一致快照。源材料已放弃而 EDIT 仍显示副本时，显示者继续持有原 token。清屏须调用成功且确认 EDIT 长度为0，失败保持 token/viewer；主窗先成功创建再认领 viewer，创建失败不会锁死其他恢复入口。
5. 任意线程调用 `DllCanUnloadNow` 只作后台线程退出检查和有界无文本 owner 通知；不跨线程取得 Rc/COM、销毁 HWND 或等待 STA。owner 暂停消息泵时返回 S_FALSE，恢复泵后由 owner 清理。DestroyWindow/注销失败保留完整 View、userdata 地址、原预算和模块租约，防止只 pin DLL 却释放窗口参数或提前归还额度。

## 容量推导与压力结果

当前原型边界为64字节预编辑、默认50候选、最多32已接管事件。启动前从内置已加载 TSV 取 id/text/annotation/source 最大字节，按两倍字符串容量、逐分配32字节元数据、候选向量128槽推导单计划成本。固定成本覆盖 ledger32 + snapshot32 + 16临时/双引擎/UI副本；恢复文本包含每事件1024字节元数据、4096字节头/原因、字符串增长与 UTF-8/UTF-16/EDIT 展示副本。它是当前原型的保守策略，未来资源/配置变化必须重新审查，不能外推生产词库。

| 真实 x64 对象/计费 | 本次值 |
|---|---:|
| Host / OwnedEvent / Candidate | 1288 / 264 / 104字节 |
| 恢复 View/Store/broker/容量及固定余量 | 5616字节 |
| Reservation 自身及保守元数据 | 88字节，模块自动加计 |
| 每 epoch 文本预留 | 1,310,720字节 |
| 每 epoch 固定/计划预留 | 3,592,832字节，另加88字节 token |
| 模块上限 | 64 Host、64批、64MiB文本、128MiB固定/动态开销 |

初始4MiB/16MiB策略实际只能容纳三个最坏 epoch，默认并行旧 watchdog 测试首键被门禁拒绝；[原失败](round3-recovery/initial-4mib-default-parallel-failure.txt)保留，串行同断言通过。按完整队列/多实例资源需要调整策略后再次默认并行全部通过，没有重置生产计数或放松旧测试。中间两项新测试准备错误（独立计数域仍假定文本维度必满、故障注入晚于可能的同步 fallback）也保留[原始失败](round3-recovery/intermediate-test-preparation-failures.txt)，修正准备时机与精确计费断言后复跑。

release 生产 COM/Win32 压力实际保留 **36批 Unknown 材料**；采样峰值 **hosts=37**（含新入口测试活动 Host）、batches=36、text=47,185,920、fixed=133,670,864字节，固定维度先拒绝下一批。拒绝后同步/异步原生键、Finalize 和取消均无新写入，旧材料逐字节、身份和顺序不变；逐个明确关闭后四个计数归零且 DLL 返回 S_OK。[原始压力输出](round3-recovery/production-pressure.txt)

这不是 OS 物理堆、工作集、窗口句柄内存的精确上限；EDIT 可能保留内部缓冲容量。本轮压力记录的是实际对象布局与可达材料/副本的保守计费，不声称所有系统分配器开销已被证明。

## 自动化与原生故障证据

Rust 1.90.0、Edition2024，Windows 11 build26300、i5-11400、12逻辑处理器：

| 检查 | 结果 |
|---|---|
| locked fetch、offline debug/release 构建、fmt、Clippy `-D warnings` | 通过 |
| 默认并行 workspace 全 target | **144 passed**，0 failed；其中原生84，2项诊断默认 ignored |
| doc tests、资源/依赖政策、Python unittest | 通过；Python **16 passed** |
| 固定 cargo-audit0.22.2、包许可证逐字节审计 | 15外部包、31许可证原件通过，0漏洞/0警告 |
| 新 release DLL 真实 loader | 1 passed，1预热+20轮，句柄 **113→113** |
| release 模拟 COM、恢复故障、第三轮原生输入、压力及原型诊断 | 全部通过；原始样本与限制保留 |

RustSec 实际数据库 `640bf5a9f06e66d8721e8772766531fe77c8630d`、1349条；精确锁 SHA-256 `3c021bd45f9ff3dd89776deccc2f80d8f9b18401abdc1eaeb531ce92c072c042`，并未沿用历史成功替代本次审计。[审计](round3-recovery/dependency-audit.json)

新增故障直接调用生产 COM/Win32 路径，保留全部旧断言：

- 主恢复窗口20轮外毁、用户显式重开，正文默认空白，Unknown/sequence/epoch不丢失、不重复。
- 另一线程卸载检查在 owner 不泵消息时有界 S_FALSE；等待30ms窗口仍在，owner恢复泵后销毁并回到 S_OK。
- 通知失败最多四次；broker外毁后只有 owner 修复，原材料不变。两窗口类注册中间失败回滚已成功部分。
- 实际额外同类窗口使 `UnregisterClassW` 失败；userdata View 继续存活，固定额度持续占用。累积64 Host后新 Host被拒绝，DLL保持 S_FALSE，未伪造释放。
- 独立 owner 真实更新/清除 store 与取快照交错，旧正文只保有旧 token，新正文同时包含新 token；不存在两次锁造成的错配。
- 自有 EDIT subclass 拒绝空 `WM_SETTEXT`：即使 SetWindowTextW 表面成功而正文未清，仍保留 shown/token；解除故障后明确关闭才释放。
- 主窗创建失败不认领 viewer；scope过期同步原生 SetText 后 Unknown、停用、新 owner 显式查看仍保存真实原始信封及计划目标，不发生自动重试。

详见[恢复故障](round3-recovery/recovery-fault-matrix.txt)、[原生入口回归](round3-recovery/native-round3.txt)和[默认并行输出](round3-recovery/workspace-tests.txt)。

## 产物与性能边界

release DLL **563,712字节**，SHA-256 `750a8b268e5d0a56743220bfdba09cea1ce1008e0d5d3ea15cdc612d570ac543`。[身份](round3-recovery/dll-identity.json)、[loader](round3-recovery/real-dll-loader.txt)、[29项产物校验](round3-recovery/checksums.json)使用 UTF-8/LF 固定，避免 Git文本换行影响复核。

模拟 COM 两路径各100预热+1000样本：同步 keyedit P95/P99=48.3/66.2µs，候选提交=35.1/48.5µs。原型核心79,000事件 P95/P99=4.7/7.8µs；11,000词典查询=0.6/0.6µs，冷载入0.3102ms。背景负载未完全控制，无配对收益声明；模拟 COM、64条原型和83条历史诊断均不等于真实 TSF、正式词库或语言准确率验收。[样本](round3-recovery/simulated-com-benchmark.txt)、[原型](round3-recovery/core-performance.json)

## 未完成与 A1–A5 门禁

1. **C物理 STA 退出仍是放行阻塞。** 实际旧 owner CoInitializeEx/CoUninitialize并退出后，新 STA能显式查看/放弃其材料；但旧窗口类/owner资源没有安全注销回收证明。材料释放后仍有1个 Host固定额度及模块租约，必须 S_FALSE。当前策略有界保活，未实现可靠回收；累计耗尽时拒绝新 Host会影响可用性，不能称完整退出或产品就绪。
2. DestroyWindow/注销持续失败、owner永久阻塞或通知永远不可达也只有保全/拒绝证据，没有宣称宿主已经恢复。没有使用 unsafe Send 或跨线程 drop Rc/COM 来伪造零计数。
3. A1–A3 新增共享额度与恢复视图路径经人工故障注入，真实宿主焦点竞态、密码/readonly、长时间压力和系统事件矩阵仍未全部退出；合并重复消息兼容边界保留。
4. **A5未开始**：没有系统注册/profile激活、记事本/浏览器/办公软件/多行编辑器验收。x86/ARM64、安装/签名/升级卸载、正式算法和正式词库均未由本轮交付。

root 另按本报告之后最终 head 核对 Windows/Linux CI，并记录到共享第三轮报告。所有 PR保持 Draft，未合并，CL-010未开始。

## 复现

运行 README完整检查；固定审计工具可通过 `--audit-binary` 指向已装0.22.2，不能用不同版本替代。随后设置 `CLOUDLEAF_REVIEW_DLL` 为新 release DLL绝对路径，按用户附件运行两项 `--release --ignored --nocapture --test-threads=1` 专项。需要直接看到loader句柄输出时仅在该单项子进程设置 `CLOUDLEAF_LIFECYCLE_REVIEW_CHILD=1`；不得在全套并行中设置。具体命令和返回码见 [summary.json](round3-recovery/summary.json) 与 [diagnostics.json](round3-recovery/diagnostics.json)。
