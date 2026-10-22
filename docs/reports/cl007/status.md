# CL-007 原生适配阶段记录

日期：2026-10-22。独立分支 `codex/cl007-windows`，原基线 `a564fd4`，设计先于实现，首个原生实现检查点 `0776163`。已合并共同契约分支，不修改 main。此报告记录部分实现，CL-007 及正式产品均未完成，不能进入 CL-010。

## 已实现及证据

- `apps/windows-ime` 输出 x64 DLL 与显式管理工具；`adapters/windows` 独立实现 COM 工厂、TSF sink、组合编辑、候选 UI、只读按键预判、提交确认、epoch 失效和恢复边界。候选算法仍调用原有核心。
- 原生按键和候选 Finalize 都申请同步写锁，授锁前不修改核心；拒锁保留原键或候选供重试。每次合法重复按键创建独立事务；同一 COM 回调最多执行一次，申请返回后即封闭迟到回调入口。
- 提交只有完成 SetText、选区和 EndComposition 后才确认 Applied。写入前失败为 NotApplied，进入写入调用后不能确认结果为 Unknown，隔离并禁止自动重试。Unknown 失焦、退出及取消不删除可能已提交文本。
- 失焦先将全部旧资源脱离 Host，再调用外部 COM；关键文本/UI 外调后重查 epoch，旧事务不能确认新核心或存回旧 UI。同步清理拒锁后异步只结束组合，避免迟到范围删除。
- 候选 UI 可由应用接管，NULL 首次查询页数遵守系统契约；自绘窗口不激活宿主外焦点，临时 Rc 保护同步重入销毁。TS_E_NOLAYOUT 隐藏候选并等待布局通知重试。
- 注册实现拒绝覆盖 HKCR/TSF 同 GUID 服务，分步骤回滚；会话切换先 create_new、UTF-8、sync_all 保存 profile 快照。仅实现并审查命令，未执行注册、激活或恢复。
- 精确锁定15个外部包，保留31份原始许可证。独立审计与实际脚本均核对 Cargo registry 原件；Git index 导出后31个 SHA-256 仍一致，`.gitattributes` 禁止原件换行转换。`dependency-audit-final.json` 记录工具、锁、数据库及许可身份。
- RustSec 使用 cargo-audit 0.22.2，数据库1348条、提交 `4b587f00996ebe5c05a73eaef105ef241e876414`；对应锁文件0漏洞、0警告。此为数据库快照结论。CI 已添加固定工具和精确许可核对门禁。

## 未完成与禁止放行项

1. 完整异步原生按键 ownership、原始事件保全/顺序重放尚未实现；同步事务不是这一退出条件的替代品。没有使用 SendInput 盲重派。
2. 剩余异步仅布局和旧组合结束：布局队列上限32，operation ID/epoch/context 身份，重复回调无重复派发，500毫秒过期回调隔离；独立无消息 watchdog 未实现。清理拒锁或超时可能保留旧预编辑，自动宿主恢复与对用户完整反馈未完成。
3. 已知失败的完整交互重试、所有 TSF compartment/密码输入 scope 的真实宿主表现、外部文本编辑和选区更新监测尚未完成真实验收。现 scope 查询失败采取不消费键的保守行为。
4. 尚未注册、激活 Cloudleaf；root 只读核实 HKCU/HKLM COM CLSID 与 CTF TIP 四个固定路径均不存在。本轮保持该状态，未触及默认输入法。
5. Notepad、浏览器等真实应用的组合、候选、提交、鼠标、Unicode、焦点、快捷键与故障恢复均未验收。COM 故障宿主测试不替代这些证据。
6. x86/ARM64、签名、安装包、权限边界、长时资源泄漏和跨版本 Windows 兼容性未完成。Windows/Linux CI 状态须以该分支实际运行记录为准，不能沿用 main 成功记录。
7. lParam 重复次数为1的各独立消息分别执行；重复次数超过1的合并消息整包透传，合并自动重复处理未实现。bit30不作为去重 ID；不能部分处理却接管其余重复字符。

## 检查与性能解释

最终执行格式、clippy、全部 Rust 及文档测试、Python 测试、资源审计、精确依赖审计与 release 构建。原有测试保持，新增故障测试直接调用生产 COM 实现的 vtable，覆盖拒锁、readonly、scope 拒绝、迟到/重复回调、Unknown 清理、布局满队列及过期等边界。

本地在实现提交 `3a6696eb6efa85297c47c46732f03475c9715e75` 完成：Rust 88 passed、0 failed，另1项显式诊断基准默认 ignored；其中 Windows 原生28项测试。Python 16 passed。格式、全 workspace clippy（`-D warnings`）、文档测试、资源审计、精确许可证/RustSec 审计与 release 构建通过。诊断基准再以 release+ignored 显式执行，1 passed。复现命令：

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
python -m unittest discover -s tests -p "test_*.py"
python scripts/check_workspace.py
python scripts/audit_windows_dependencies.py --output .local/windows-dependency-audit.json
cargo build -p cloudleaf-windows-ime --release --locked --offline
cargo test -p cloudleaf-windows-ime --lib --release --locked --offline simulated_com_diagnostic_benchmark -- --ignored --nocapture --test-threads=1
python scripts/benchmark.py --iterations 1000 --output .local/core-performance.json
python scripts/evaluate_algorithms.py --output-dir .local/algorithm-final
```

`core-performance.json` 和算法报告只测64条手写原型词典及83条诊断种子，源码指纹 v3 包含 adapters。它们不是正式准确率、TSF UI 延迟或真实 Windows 输入法性能。原生 DLL 体积与哈希另记录，不据体积推断运行性能。

本轮源码指纹为 `cloudleaf-source-v3` / `5c37626623ad0c448e2f9ddafd0b8b7ccc38732bfa5cc48810e5857c00e0a2dd`。原型核心事件 p50/p95/p99 为1.9/4.9/10.2微秒，峰值进程工作集6,000,640字节。模拟 COM 诊断（100预热、1000样本）记录于 `native-diagnostic.json`，keyedit p50/p95/p99 19.3/38.0/61.9微秒，候选提交4.9/8.8/20.1微秒；这些都是模拟宿主机制测量，背景负载未控制，非 Windows 应用端到端性能。

`artifacts.json` 记录实际 Cargo 产物：x64 DLL 393,728字节，SHA-256 `cdccb7a420eca4e0bafc331f49d10767c4d3014691cf32eb3e6162ff135946fc`；管理工具196,096字节，SHA-256 `f8491a6b32603c3080177c1a3ef42a10e26589c7b8839ff478e20d1e2b50c502`。测试宿主二进制身份及完整模拟诊断命令另见诊断 JSON。
