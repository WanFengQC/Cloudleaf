# CL-007 第三轮：共享恢复预算与 STA 所有者

2026-10-23；基线 `2761001`。本文先固定接口，再实现；不注册输入法、不激活 profile、不向真实应用注键，也不修改核心算法或原型词典。

## 范围与接口

模块预算由 `adapters/windows/budget.rs` 管理：`reserve_host(fixed_bytes)`、`reserve_batch(text_capacity, fixed_capacity)` 返回独占 `Reservation`；只读 `can_reserve_*` 不改状态。`Reservation` 可由 `Arc` 共享，最后一个持有者释放完整额度。`usage/limits` 分开报告 hosts、batches、text_bytes、fixed_bytes；预留失败不得留下部分扣费，计数路径不等待互斥锁。token 自带模块租约。预算控制活动 Host、已停用实例、退休 UI 与未解决材料，不以 watchdog 的线程槽代替。

Host 激活时取得对象固定成本额度；第一个原生异步事件取得 ownership **之前**，为本 epoch 的完整32事件队列预留批次额度。尺寸从实际 Rust 对象布局、已加载原型候选字段最大容量、预编辑64字节、50候选、队列32项及 UTF-8/UTF-16 展示副本上界推导；报告实测结构大小和压力峰值。该界限只适用于当前已加载原型，不是生产词典规模目标。后续实际计划超过已登记上界时必须停止新接管，不能截断或继续假定上界有效。

正常 Applied 移除事件，队列完全排空且没有移交材料时释放批次 token。隔离时恢复 store 与 Host 共享同一 token，不重复占用批次，也不提前释放；Unknown 永不因窗口销毁、焦点变化、停用或额度紧张而清除。额度耗尽时新事件仍交给应用，旧材料、顺序与来源保持不变。用户显式放弃才可解除未解决材料的额度。

同步 fallback、候选 Finalize 与取消也属于实际写入入口：在写锁内确认 scope 后、改变核心/确认接管之前取得相同 epoch 预算。独立的同步事务材料标注操作类型（不能伪装成来自系统的原生信封）、来源 epoch 和 Started/Writing/Unknown；实际编辑完全完成才释放。预算满时原生 fallback 不吃键，UI操作明确失败且不写入；监测不健康时原生新键透传。迟到的已放弃批次由 store 的来源 epoch 水位拒绝复活。恢复正文同一 store 同时只允许一个视图显示，防止反复打开产生不计费的无限副本。

恢复材料的来源 `instance/epoch`、每条 sequence/outcome 和文本保存在不含 COM/HWND 的独立 store；全模块登记未解决 store，已停用 Host 不再是唯一所有者。窗口只是视图。重复交付同一 epoch 更新同一批材料，不能生成重复历史；不同 epoch 分批。恢复主 HWND 外部销毁只清视图句柄，不清 store。独立的无原文恢复入口允许用户明确重建主窗口，仍须再次点击查看才显示内容。材料不会自动插入新文档，Unknown 不会自动重放。

每个 UI owner 另有无文本 broker HWND；watchdog 与卸载清理请求投递到 broker。`DllCanUnloadNow` 在任意线程仅回收真实 OS 已退出的后台线程，并向其他 STA 的 broker 发有界无文本清理请求；不能取得或移动 Rc/COM、跨线程 DestroyWindow，也不等待 STA。只有 owner STA 可销毁自己的 HWND、注销类和释放其模块租约。失败通知、未泵消息、宿主卡住、未解决材料及未完成回调继续保持 S_FALSE。

物理 STA 退出时不能凭 TLS 析构宣称所有资源已安全释放：仍有材料的 store 继续持有模块租约；尚未确认注销的 owner 资源保守保活。broker 被外部破坏时观察失败并在下一可用 owner 入口修复，不能承诺没有任何 owner 入口的永久阻塞会自行恢复。全模块 store 可被后续可用 STA 的显式恢复入口读取；没有消息泵时只检测/保留，不宣称恢复完成。

## 文件范围与依赖

- `budget.rs`：共享计费和独立计数/并发压力测试。
- `recovery.rs`：独立 store、恢复入口/主视图、broker、跨线程请求及真实 Win32 故障测试。
- `session.rs`：激活固定成本、ownership 前 epoch 预留、Applied/隔离/停用的 token 转移与释放。
- `service.rs`：卸载仅请求 owner 清理，保持未完成状态。
- `mod.rs`：登记模块；文档与第三轮报告记录准确覆盖和剩余门禁。

共享预算接口冻结后两部分并行实现，先运行定向生产路径测试，再执行 README 完整检查、固定工具依赖审计、新 release DLL 装卸和模拟 COM 基准；root 统一核对最终提交双平台 CI。

## 故障矩阵与仍有边界

| 路径 | 必须核对 |
|---|---|
| 多 Host、多 epoch、停用保留材料、总额满 | ownership 前拒绝新键；旧材料数量/顺序/文本不变；Unknown 不释放 |
| 全部 Applied、显式放弃 | 对应额度归还；不会影响其他 instance/epoch |
| 外部销毁主窗口、反复重建、晚到回调 | store 不丢失、无重复批次、不自动显示正文、不污染新上下文 |
| 异线程 DllCanUnloadNow、owner 暂停泵消息 | 调用有界返回 S_FALSE；错误线程不销毁；恢复泵后 owner 清理 |
| 通知失败、broker 外毁、monitor 部分启动失败 | 保持可审计失败；无假成功、无无限重试、无原文跨线程信号 |
| 线程退出、未解决材料、仍持 sink/callback | 未满足全部退出条件前保持模块保活 |

真实应用、密码/readonly真实宿主、系统 profile、长时输入压力、跨架构与安装发布仍属于 A1–A5 未完成范围；本轮专项通过不放行 A5。

官方依据：[DestroyWindow 的线程归属](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-destroywindow)、[DllCanUnloadNow](https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-dllcanunloadnow)、[PostMessage 的失败与异步语义](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-postmessagew)。
