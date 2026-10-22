# CL-007：真实 owner 线程退出证明

2026-10-23，Round 4。先提交本设计，再实现平台私有 `owner_cleanup.rs`；不改变核心/平台共享协议、系统注册或输入行为。配合 [owner receipt 设计](cl007-owner-receipts-v4.md)。

`OwnerThreadProof::capture_current() -> Result<Arc<OwnerThreadProof>>` 在 owner 正常执行期间，用 `DuplicateHandle(GetCurrentProcess(), GetCurrentThread(), ...)` 把当前线程伪句柄复制为真实独占句柄，并记录诊断用线程 ID。`OwnedHandle` 负责最后释放，Arc 只共享同一份句柄，不重新按 ID 查找线程。此值不含 COM、Rc、Weak、HWND 或回调。

平台私有只读方法为 `owner_id() -> u32`、`is_owner() -> bool`、`exited() -> bool`。`exited` 仅作 `WaitForSingleObject(real_handle, 0)`，唯有 `WAIT_OBJECT_0` 才认为退出；超时或失败均不得宣称退出。`is_owner` 同时要求当前线程 ID 相同且零等待明确返回 `WAIT_TIMEOUT`，避免 ID 复用后把新线程当成旧 owner；等待失败不证明“仍活跃”，也不能授权 owner 操作。调用不等待线程结束，不参与 DLL 租约计数；持有它的 receipt 在资源清理得到证明前继续收费、保活。

证明仅涵盖原 OS 线程退出，不证明其 COM 引用、窗口类、控件内存、watchdog 或未解决输入已经清理。调用方还必须分别确认无存活窗口、回调/worker完成与类注销成功；Unknown store 只有用户明确放弃才释放。不得因为线程句柄 signaled 就跨线程 drop Rc/COM 或对其他线程调用 DestroyWindow。TLS/DllMain 中不创建此证明、不调用 User32、不等待；owner正常创建资源时提前取得句柄。

验证独立覆盖当前线程活跃、跨线程身份、真实 owner join 后退出、Rust JoinHandle 被释放后仍能观察原线程、反复创建/退出及 Arc 最后释放。测试只创建自己的线程，无系统输入法激活或输入注入。完整集成另由 receipt 的生产 COM/Win32 故障与压力矩阵证明。

官方依据：[GetCurrentThread 伪句柄与跨线程限制](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentthread)、[DuplicateHandle 转真实句柄](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-duplicatehandle)、[零等待和线程对象状态](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject)。
