# CL-007 第二轮：异步事件保全与独立监测

2026-10-22；分支 `codex/cl007-windows`。实现检查点 `ca8e200389c350f55d42bd3250b140c3687e7993`，源码指纹 `cloudleaf-source-v3 / 2aaf0b8f08ea77ad3e42fbdb00617cb7e9b8ebc6600fc3b1e75e77aac9d63492`。这是平台可靠性部分实现，A1–A4 尚未满足全部退出条件，A5、CL-010 未开始。没有执行系统注册、输入 profile 激活或真实应用验收。

本报告继承并保留[上一轮报告](status.md)及其原始产物；旧同步限制、旧依赖数据库和旧性能只描述各自检查点。新机器证据、原始测试输出、基准和校验在 [async-ownership/](async-ownership/evidence.json)。

## 实现变化

- 原生单个 IME 键取得 ownership 后保存 instance、来源 epoch、单调 sequence、原始 WPARAM/LPARAM、布局和修饰键，进入32条 FIFO。预判只读；连续相同的 count=1 原生消息分别编号，不按 bit30 去重。新键遇满队列不被吞掉，已有材料保全。
- 激活时建立实际/投影引擎。实际引擎结果与投影计划比较全部语义，独立 revision/commit-ID 不强行相等；真实 ACK 只使用实际 ID。候选 Finalize/取消、同步 fallback、失焦与 Unknown 后恢复均有差分语义回归。候选算法仍在共享核心，本分支没有修改核心搜索或默认词典。
- 原生命令先申请同步 barrier 排空旧 FIFO，再把原消息交给应用。拒锁时封闭旧回调并保全；不转换 Enter/Space/快捷键为伪造文本，不调用 SendInput 重发未知结果。
- 已接管事件按 Queued/Started/Writing/Applied/NotApplied/Unknown 管理。非空写入和空文本删除共用 Writing/Unknown 边界；SetText、选区、EndComposition 及其他宿主外调后重新核对 epoch/context/监测健康。旧事务不能确认新核心，也不能拿走新组合。
- 实际 ITfTextEditSink 的 OnEndEdit 调用 InWriteSession(client) 区分自身/外部/查询失败。通知使 scope 证明失效，外部编辑保守隔离；只读通知不写入、不读取宿主正文猜测归属，异步旧清理不擦范围。
- 两个独立线程只交换原子和无文本通知，分别检测事务与监测健康。500ms 是诊断事务期限；Queued/Started 过期封闭入口，Writing 过期保留 Unknown。真实 STA 自有恢复窗口接收通知，因此没有后续输入也能隔离。STA 卡在宿主 COM 时仅能先检测，须等 STA 返回才完成平台恢复。
- 通知投递失败立即停止异步接管，生命周期内最多尝试四次；持续失败后保留故障状态，下一 STA 入口隔离。线程部分启动失败、停止和晚到回调均有测试；owner 用真实 OS 线程 HANDLE 零等待确认退出后再 join、释放模块租约，不依赖 worker 尾部析构或仅看 is_finished。
- 用户显式查看后恢复窗口才显示材料；不同来源 epoch 分批，关闭旧材料不能取消新上下文。停用后材料独立于 Host 保留；单窗口32批/1MiB达到上限时拒绝新材料，不能覆盖旧内容。候选窗口外部销毁、结束 element 失败与反复创建释放也经真实 Win32 控件检查。

设计先行提交为 `d70d9ee`、`33b59de`、`37a2f4e`、`34856a5`、`ac6833e`、`449c385`、`b3f087e`、`b026fd0`。详细事件/恢复规则见[可靠性约定](../../cl007-async-ownership-v1.md)。

## 实际验证

在上述实现检查点，本机 Rust 1.90.0 / Edition 2024、Windows 11 build 26300、i5-11400、12逻辑处理器完成：

| 检查 | 实际结果 |
|---|---|
| locked/offline workspace debug/release 构建、fmt、clippy `-D warnings` | 通过 |
| workspace 全 target 测试（默认并行） | 126 passed，0 failed；其中原生66；2项显式诊断默认 ignored |
| 文档测试、资源与依赖政策审计 | 通过；64/41/83原有资源保留 |
| Python unittest | 16 passed，0 failed |
| 固定 cargo-audit 0.22.2 与原包许可证逐字节核对 | 15外部包、31许可原件通过；对应锁0漏洞、0警告 |
| 新 release DLL 显式 loader 测试 | 1 passed；1预热+20轮；149→149进程句柄 |
| release 模拟 COM 诊断基准 | 1 passed；两条路径各100预热、1000样本 |
| release 原型基准与83条历史算法诊断 | 完成；不作为正式语言质量或真实 TSF 性能 |

RustSec 数据库本次实际更新为 `640bf5a9f06e66d8721e8772766531fe77c8630d`、1349条；锁 SHA-256 `3c021bd45f9ff3dd89776deccc2f80d8f9b18401abdc1eaeb531ce92c072c042`。审计结论只适用于该数据库快照和锁，见[完整审计](async-ownership/dependency-audit.json)。

首次运行发现5个新测试依赖操作人员 Caps Lock 状态，将小写断言实际得到大写 N；修复测试隔离后复跑，未改生产大小写行为或降低断言。测试保存、设置、还原本测试线程的输入状态表，不向系统发送按键。Microsoft 明确该接口不影响其他线程或全局指示灯：[SetKeyboardState](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setkeyboardstate)。

关键可重放轨迹直接进入生产 NativeDrain/run/render 与真实 COM vtable：

| 故障/事件 | 被断言的边界 |
|---|---|
| n、i、Space、第二个原生 Space；重复/迟到 callback | FIFO提交一次；第二个 Space 仍归应用；晚到回调无新增写入 |
| 100次只读预判、32事件满队列、拒锁 Enter barrier | 不改变核心；第33个新键未接管；已拥有事件保持 NotApplied，无伪造换行 |
| SetText写入后失败、重复确认、Unknown 后新上下文 | 不自动重试、不补假 ACK；独立核心计数不同仍保持语义一致 |
| 删除组合后失败、EndComposition失败 | 空 SetText 同样保留 Unknown；后来应用新文本不被清理删除 |
| 删除 SetText 内重入失焦；EndComposition 内重入并安装新组合 | 旧事务不继续删除/重复结束，不拿走新 context 的 composition |
| 宿主 SetText阻塞650ms、STA不泵消息 | 独立线程先检测 Unknown，阻塞期间隔离尚未完成；STA返回后不能确认 Applied |
| 无后续输入的500ms排队超时 | 真实无文本窗口通知触发隔离；被封闭的迟到回调不写入 |
| 主监测线程不再 heartbeat | 对端检测故障并通知 STA；不再接管新键 |
| 通知连续投递失败 | 最多4次；未假称通知已到；下一 STA 入口仍保全原有 ownership |
| 部分线程创建失败、OS线程退出证明 | 故障不污染后续 monitor；completion 仅由 owner 在 OS退出证据后发布 |
| Factory/LockServer、100轮服务激活停用、Advise中间失败、Unadvise失败 | fake manager 的 sink/COM 引用对称；晚持有 sink 继续阻止 DLL卸载 |

代码位置：[原生故障测试](../../../adapters/windows/render_tests.rs)、[独立监测测试](../../../adapters/windows/watchdog.rs)、[资源生命周期检查](../../../adapters/windows/lifecycle_review_tests.rs)、[恢复控件测试](../../../adapters/windows/recovery.rs)。完整 workspace 输出见[原始日志](async-ownership/workspace-tests.txt)。

## 新产物与性能记录

release DLL 520,192字节，SHA-256 `5e7bfdcffe353af19dc887ade0ee39bebc524b5a119f4080bedce8f0ac6c999f`。loader 通过实际 LoadLibrary、DllGetClassObject、COM服务/fake manager、DllCanUnloadNow、FreeLibrary，并确认映射消失；默认 ignored 防止未经明确指定产物就运行。这仅覆盖空焦点、无待恢复材料的假 manager 生命周期，149→149不能证明所有资源分支有界，不能称系统输入法激活或真实应用通过。[原始 loader 输出](async-ownership/real-dll-loader.txt)

| 诊断 | 样本 | P95 / P99 |
|---|---|---|
| 64条原型核心事件 | 79,000 | 5.0 / 11.1µs |
| 同词典查询 | 11,000 | 0.6 / 1.2µs |
| 模拟 COM 同步 keyedit | 1,000 | 40.6 / 49.9µs |
| 模拟 COM 候选提交 | 1,000 | 30.1 / 44.3µs |

原型冷载入和 nihao 序列0.306ms、进程峰值工作集6,029,312字节。模拟 COM 使用64条原型/VT_EMPTY scope/TS_E_NOLAYOUT，不测真实窗口候选布局和真实应用。没有控制全部背景负载；本轮没有声称性能优于旧版本、正式词库达标或真实 TSF 流畅。完整身份与数据见[核心基准](async-ownership/core-performance.json)、[模拟 COM 输出](async-ownership/simulated-com-benchmark.txt)及[校验清单](async-ownership/checksums.json)。

## 未完成与阻塞

1. **A1–A4部分实现，未放行A5。** Notepad、浏览器、办公软件、多行编辑器及系统输入过程均未真实验收；没有注册或激活 profile。密码/readonly真实宿主、context销毁/焦点竞态和长时间压力仍需独立证据。
2. 恢复窗口的32批/1MiB仅是**单窗口**上限。跨已停用服务实例保留的有材料窗口进入 STA retired 集合，尚无总额门禁；反复新建 Host 并保留旧材料可能累积。watchdog slots退出后会释放，不能作为该集合上限。这是A4放行阻塞。
3. 外部销毁恢复主窗口后的材料可达性存在风险；不同线程调用 DllCanUnloadNow 时，原 STA的无材料退休窗口清理没有本轮覆盖证据，不能称全部资源退出完成。
4. 有未放弃材料、未退出线程或外部持有 sink/callback 时继续保持 S_FALSE。宿主永久不返回或两个 monitor同时停止时，没有绝对实时恢复保证；持续消息投递失败只在后续可用 STA入口完成隔离。
5. 合并重复消息 count>1仍整包透传；所有应用原生命令的重放不在能力内，不用伪造按键掩盖。x86/ARM64、签名、安装/升级/卸载故障矩阵、跨版本系统兼容和真实资源压力未完成。

## 复现命令

按 README 设置进程级 Cargo PATH、UTF-8后执行：

```text
cargo build --workspace --locked --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
python scripts/check_workspace.py
python -m unittest discover -s tests -p "test_*.py"
python scripts/audit_windows_dependencies.py --output .local/windows-dependency-audit.json
cargo build --workspace --release --locked --offline
cargo test -p cloudleaf-windows-ime --lib lifecycle_review_real_dll_loader_no_registration --release --locked --offline -- --ignored --nocapture --test-threads=1
cargo test -p cloudleaf-windows-ime --lib simulated_com_diagnostic_benchmark --release --locked --offline -- --ignored --nocapture --test-threads=1
python scripts/benchmark.py --iterations 1000 --output .local/core-performance.json
python scripts/evaluate_algorithms.py --output-dir .local/algorithm-v1
```

loader 要在当前进程环境设置 `CLOUDLEAF_REVIEW_DLL` 为刚构建DLL的绝对路径；测试内部自建独立子进程验证全局计数。若需打印内部句柄数，可仅在显式过滤这一项的独立测试进程设置 `CLOUDLEAF_LIFECYCLE_REVIEW_CHILD=1`，不得用于并行跑全部资源测试。固定工具在其他目录时可向审计脚本传 `--audit-binary`、`--db`；本机使用已安装0.22.2，未重新安装。

远程 CI必须针对证据文档之后的最终head单独核对；代码检查点 `ca8e200` 的PR run为 [39801343366](https://github.com/WanFengQC/Cloudleaf/actions/runs/39801343366)，本报告写入时仍在执行，不能写作最终head已通过。
