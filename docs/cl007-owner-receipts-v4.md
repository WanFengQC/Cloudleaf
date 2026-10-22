# CL-007 第四轮：纯数据 owner receipt 与可验证回收

2026-10-23；基线 `bb6aea1`。先设计，再实现。不注册系统输入法、不激活 profile、不向真实应用注键，不改核心算法/词典/main。

## 已发现的问题

第三轮 `View` 把 `Weak<Host>`、Rc/Cell、HWND、显示凭证和租约绑在一起；TLS退出通过 forget Rc 防止错误线程释放。这样虽然保住材料及 raw userdata，却永久占有 Host额度和DLL租约，无法在“旧STA确实退出、全部材料已明确放弃”后恢复。只有Endpoint pin DLL并不能替代userdata对象存活证明。第四轮移除这个永久混合对象。

## 私有平台接口和归属

1. `View` 改为纯 Send/Sync 的 `Arc` 状态：唯一 generation、owner OS线程证明、原子 HWND数值/注册状态、不可变唯一类名与模块身份、独立 Store、Mutex保护的显示材料凭证、预算与模块租约。它不含 Rc、Weak<Host> 或任何COM接口，不增加 unsafe Send。
2. `Weak<Host>` 只放在创建STA的 TLS hook表，以generation索引。停止平台时owner显式摘除hook；窗口过程也只在owner调用该hook。TLS析构不调 User32、不等待线程、不释放强COM对象。仍被外部sink/callback强持有的Host/COM对象继续自行保活，纯receipt不得替它们伪造引用释放。
3. `owner_cleanup.rs` 提供 `OwnerThreadProof::capture_current()`、`owner_id()`、`is_owner()`、`exited()`，内部用真实重复线程句柄与零等待，不按可复用线程ID重新查找。根单独实现/测试此模块；主实现消费冻结接口。
4. 模块全局注册表在类/窗口创建前取得 `Arc<View>` 强引用，直到两个类已证明注销且没有存活窗口才释放。WndProc的raw userdata始终借用这个稳定地址，临时Arc保护重入；不把裸指针或Rc移到其他线程。
5. `RecoveryWindow`包装的析构只标记退休，不做User32或阻塞；全局receipt仍保有状态、原预算和租约。owner显式入口可以销毁自己的窗口并注销自己的类。`DllCanUnloadNow`只进行零等待证明、有限量回收及无文本通知，不无限等待。

## 退出状态机

- 活动owner：只有owner可以清屏、DestroyWindow、摘hook及注销类；其他线程只发有界无文本清理请求。持有Unknown时仍提供用户恢复入口。
- 已停用、owner仍活：watchdog真实线程退出完成后，owner可清掉无材料UI；有材料仍独立于Host存活，不自动放弃。销毁/注销失败记为未完成receipt，保留原状态与额度，后续显式owner入口可重试；每次检查限制工作量，无忙等。
- owner真实线程句柄已signaled：还要确认watchdog/后台通知退出，并证明没有这些唯一类的存活窗口，才可由可用线程注销无窗口的类、清除纯显示claim与凭证、归还owner固定额度。任何未获证据、类仍有窗口或注销失败继续S_FALSE；绝不跨线程DestroyWindow或drop Rc/COM。
- Unknown Store仍在全局材料目录中，owner资源回收不会丢弃它；新可用STA只显示无原文入口，用户明确查看/复制或放弃。只有材料、owner资源、后台任务及外部COM引用分别完成后才可S_OK。

Windows线程退出、User32窗口销毁、类注销和COM apartment的官方语义由[owner证明](cl007-owner-proof.md)限定；设计不能靠“进程里看不到窗口”推断全部COM已释放。宿主永久卡住或强COM引用仍存在时保持可审计阻塞，不用假零计数冒充恢复。

## 文件范围与并行依赖

- 根：`owner_cleanup.rs`及对应proof设计、真实OS句柄单元证据。
- A实现：`recovery.rs`纯状态/registry/hook及回收、`session.rs`显式停止接入、`service.rs`卸载检查、`mod.rs`登记；新增生产COM/Win32故障回归及第四轮报告。共享core和budget公开接口不变。
- 提交index按根协调；文件写入互不交叠。

## 测试和退出边界

保留第三轮全部有效断言及失败记录。历史“物理STA退出仍必须S_FALSE/hosts1”的回归保留为“类清理仍注入失败”的保守门禁，再新增无故障完整退出证明；不能只改expected来获得通过。

必须覆盖真实COM apartment init/uninit/线程join、未解决Unknown跨STA可达、明确放弃后的四维预算与DLL归零、反复owner退出与类失败后恢复、异线程卸载/不泵消息/通知失败、部分创建失败、迟到回调及持有外部强sink/callback。正文与token同锁快照、清屏失败保token等第三轮回归不弱化。压力记录多owner/多轮、实际资源尺寸/峰值和成功/拒绝/恢复轨迹，不能只跑空焦点循环。

本轮机制验证仍不替代真实应用、系统profile、密码/readonly宿主、长时间输入、跨架构及安装验收，A5不自动放行。完整README本地检查、固定审计工具、真实新release DLL装卸/原始基准与最终精确提交CI分别记录。
