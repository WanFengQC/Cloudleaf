# CL-007 原生 Windows 实现约定

本提交先固定实现边界，后续代码和真实应用证据单独提交；设计并不等于已实现或验收。

## 模块与线程

`apps/windows-ime` 输出 `cloudleaf_windows_ime.dll` 与管理工具；`adapters/windows` 提供平台会话、按键、COM 服务、edit session、候选 UI 和注册。只有平台 crate 允许 unsafe，每一处以中文说明 Windows API 的前提。核心继续禁止 unsafe、系统依赖和实时路径 I/O。

COM 服务、sink、edit session 和 UI element 均 apartment 亲和，`Agile = false`；记录创建线程，拒绝跨线程访问。内部可变借用不能跨外部 COM 调用，FFI 导出捕获 panic。生命周期改变先增加 epoch，再撤销 sink 和候选 UI，使旧回调不能上屏。按键 admission 调用共享只读接口；Ctrl/Alt/Win 未注册组合透传，执行按键在 edit session 中串行处理。有界等待队列发生溢出时明确恢复，不静默消费。

## 组合与提交

TSF composition 保存 `ITfComposition`，使用 document manager 当前 context。预编辑和最终提交通过 `RequestEditSession` 在写锁内更新 range、选择和组合终止；处理方法 HRESULT 与 session HRESULT 两层结果。写入之前失败记 NotApplied，已经调用写入而无法确认最终结果记 Unknown，并隔离该会话；只有确定 Applied 才发核心成功确认。取消、焦点离开及 deactivate 先失效 epoch，并清理 composition/UI。

候选 UI 实现 `ITfCandidateListUIElementBehavior`，列表内容来自核心帧。原生 selection/finalize 使用 revision + candidate ID；BeginUIElement 的应用接管结果必须被尊重。自己的无激活候选窗口按插入位置定位，WM_MOUSEACTIVATE 不抢焦点，点击通过相同选择接口。UTF-16 长度与光标使用共享严格转换函数。

## 依赖与安装

精确依赖 `windows = 0.62.2`、`windows-core = 0.62.2`，上游 Microsoft，MIT OR Apache-2.0，包 MSRV 1.82。实际完整 Cargo.lock 依赖另列许可证和 source 精确审计清单，保留全部版权文本。Windows crate 不继承 workspace unsafe forbid，但开启 unsafe_op_in_unsafe_fn deny。

管理工具提供显式 register/unregister/status，注册采用当前用户 COM CLSID 路径，不修改默认输入法；TSF 服务/简体中文 profile/category 注册分步失败回滚。DLL 路径必须绝对且存在。固定 CLSID/profile GUID 由代码唯一声明；注册前报告 root，桌面实测由 root 统一控制。卸载只移除 Cloudleaf 自己的项。x64 DLL 只能加载到 x64 应用；x86/ARM64 分发、签名安装包和权限边界不能由 x64 本地通过推断完成。

真实应用切换增加显式 `activate-session <新快照路径>` 和 `restore-session <快照路径>`。切换之前以 create_new 写入 UTF-8 的 `cloudleaf-profile-v1`，保存原 profile type、LANGID、CLSID、profile GUID 与 HKL；只在快照写入并 flush/sync 成功后调用 FORSESSION 激活，绝不修改默认或系统全局设置。恢复严格校验格式，成功后仍保留快照供验收追溯。不读取或记录宿主输入内容。注册拒绝覆盖已有 Cloudleaf COM 项，卸载须报告系统失败，不把失败操作显示为成功。

注册以新建 HKCU CLSID 项作为所有权边界，并先枚举 TSF 服务拒绝同 GUID 冲突；失败只按已成功步骤逆序回滚，回滚错误必须呈现。服务 Activate 任意中间失败均撤销已建立的 sink 和状态。当前 context 订阅布局通知，TS_E_NOLAYOUT 时隐藏候选并在后续布局通知中重试，禁止将暂缺布局判为未知文本提交。每次外部 COM 调用后重查 epoch，旧事务不能确认新会话提交。只读和密码输入 scope 在按键 admission 阶段透传；push/pop 重新查询实际焦点，不凭通知参数切换其他文档。

首次实现曾只允许同步按键写锁；本轮按[异步 ownership 约定](cl007-async-ownership-v1.md)扩展为执行/投影两引擎与32条原生信封 FIFO。只有核心明确 Handle 的单个 IME 键可以取得 ownership；后续 Space/Enter/快捷键等原生命令先执行同步 barrier，再保持原消息交给应用。拒锁或队列满保留旧 ownership，新事件不被吞掉；不把应用命令转换为文本，也不模拟系统键盘重放。Unknown 隔离后只结束 composition，绝不删除可能已经写入的文本；空文本删除同样进入 Writing/Unknown，逐次外调重新检查上下文与监测健康。

原生适配源码位于 `adapters`，因此基准和评估源码指纹升级为 `cloudleaf-source-v3`，在原 v2 规则上追加 `adapters/**/*.rs`。历史 v2 报告保持原身份，不将原型基准解释为 TSF/真实应用性能。

候选 Finalize 仍只允许同步写锁，先完成旧 ownership barrier 并验证真实 revision/id；拒锁明确返回错误，保留候选供重试。独立 Abort 也使用同步锁；同一编辑锁内重入 Abort 按 FIFO 排队。异步原生回调与布局刷新分别绑定操作 ID、epoch、context 并只运行一次。500毫秒超时由两个只持有原子的独立线程检测，Queued/Started 与 Writing 超时分别保留未应用/Unknown；真实 STA 自有窗口接收无文本通知，因此无后续按键也能隔离。宿主 COM 卡住时只能先检测，不能宣称 STA 已恢复；通知失败最多四次，随后保留故障状态直到可用 STA 入口。

恢复窗口默认隐藏输入材料；用户显式查看、选择复制或关闭。材料标明来源 epoch 并与 Host 独立存活，停用不会自动丢弃；达到保全预算后停止新接管。监测 owner 保留模块租约，实际 OS 线程句柄确认退出后才释放。仍有恢复材料或未退出线程时 DLL 不可卸载是保全结果，不能报告 A4 完成。自身/外部编辑由 InWriteSession(client) 区分，通知只读 cookie 不写入，不读取应用正文猜测归属。

失焦清理先同步申请写锁；只有同步确认仍属预编辑的旧组合才删除范围。拒锁则异步降级为只结束组合、不删文本，避免宿主后续编辑扩展旧 range 后被迟到清理删除。两种清理均一次性，异步清理超过500毫秒不再执行；代价是拒锁或超时可能保留旧预编辑，列入未完成的宿主恢复要求。任何 Unknown 清理始终禁止删除范围。

原生消息的低16位重复次数必须为1才进入当前事务路径；0为非法消息，超过1的合并重复消息整体透传，明确列为兼容性未完成，不能处理一次却接管整包。连续 count=1 的合法重复消息分别创建事务，各自执行，不按键码、时间或 bit30 去重。过期候选 revision/未知候选 ID 返回参数错误且不隔离健康会话，不给陈旧 Finalize 假成功。

## 验证与状态

执行格式、clippy、全部原有测试、平台新增纯函数/COM 测试、资源审计和 release 基准。真实 Windows 应用验收至少覆盖独立 Notepad 和浏览器输入区：组合/提交/翻页/鼠标/焦点/快捷键/取消/重激活/Unicode；保存命令、程序版本、实际文本与操作结果，不能用模拟宿主代替。故障注入必须覆盖失败回执、迟到 epoch 和 Unknown 不重试。

当前状态：已有部分原生实现及可重放的 COM/Win32 可靠性证据，CL-007 未完成；真实应用、系统注册/激活和安装验收未执行。A1–A4 仍须补长时间压力、真实宿主故障及未覆盖资源故障，未关闭前不放行 A5。外部销毁恢复窗口后的可达性、不同线程调用 DllCanUnloadNow 时 STA 窗口退出、所有宿主停止行为没有本轮完成证据。正式词典、完整算法和跨架构分发不属于本分支完成证据。

上游依据：

- https://docs.rs/crate/windows/0.62.2/source/Cargo.toml.orig
- https://docs.rs/crate/windows-core/0.62.2/source/Cargo.toml.orig
- https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-implement.md
- https://learn.microsoft.com/en-us/windows/win32/tsf/text-service-registration
- https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-requesteditsession
