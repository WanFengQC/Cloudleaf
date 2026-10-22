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

首次实现收紧按键事务：按键只申请同步写锁，授锁后才修改核心，拒锁时不消费、不修改核心；不把提交尚未确认时的原生键转换为可能失去原义的核心 Deferred 事件。候选操作和旧组合结束可异步。仅支持异步写锁的宿主列为兼容性未完成，不能据此称完整 TSF。Unknown 隔离后退出只结束 composition，绝不删除其可能已经写入的最终文本；确定仍为预编辑的组合取消才清空范围。

原生适配源码位于 `adapters`，因此基准和评估源码指纹升级为 `cloudleaf-source-v3`，在原 v2 规则上追加 `adapters/**/*.rs`。历史 v2 报告保持原身份，不将原型基准解释为 TSF/真实应用性能。

候选 Finalize 同样只允许同步写锁，授锁前不改变核心；拒锁明确返回错误，保留候选供重试，禁止排入可能被后来原生键越过的异步选择。独立 Abort 也使用同步锁；同一编辑锁内重入 Abort 按 FIFO 排队。剩余异步队列只接纳布局刷新，固定32项，操作单调 ID、epoch、context 三重身份；重复回调不得重复执行。超出500毫秒的回调拒绝执行并隔离，后续按键执行阶段检查超时并隔离；预判只读取超时状态、不改变核心或队列。无消息时的独立定时器与完整异步按键 ownership/replay 仍未实现，均阻塞注册与真实验收，不能以同步键路径替代退出条件。

## 验证与状态

执行格式、clippy、全部原有测试、平台新增纯函数/COM 测试、资源审计和 release 基准。真实 Windows 应用验收至少覆盖独立 Notepad 和浏览器输入区：组合/提交/翻页/鼠标/焦点/快捷键/取消/重激活/Unicode；保存命令、程序版本、实际文本与操作结果，不能用模拟宿主代替。故障注入必须覆盖失败回执、迟到 epoch 和 Unknown 不重试。

当前状态：实现未开始；真实应用、安装与恢复未执行；正式词典、完整算法和跨架构分发不属于本分支完成证据。只有退出条件逐项获得真实证据才标记 CL-007 完成，否则准确记录部分完成和阻塞。

上游依据：

- https://docs.rs/crate/windows/0.62.2/source/Cargo.toml.orig
- https://docs.rs/crate/windows-core/0.62.2/source/Cargo.toml.orig
- https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-implement.md
- https://learn.microsoft.com/en-us/windows/win32/tsf/text-service-registration
- https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-requesteditsession
