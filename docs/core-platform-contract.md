# 核心与平台契约 v1

- 状态：CL-006 的设计契约；现有 Rust API 已核对，宿主信封和三态结果尚不是生产适配器。
- 日期：2026-10-22。
- 适用：Windows TSF 首先接入，macOS/Linux 以后共享相同核心语义。
- 实现依据：`cloudleaf-types/src/lib.rs`、`cloudleaf-input/src/lib.rs`。

本契约冻结消息含义、所有权、顺序和失败分类，不冻结最终拼音算法、候选数量、输入长度、GUI 外观或未来二进制 ABI。当前 Rust 数据结构是进程内 API，不是可直接跨进程传输的协议；以后引入 IPC 需另行定义版本、长度、编码、取消和兼容策略。宿主必须保留核心对 `handled`、`error` 和提交确认的全部含义。

## 1. 既有核心 API

| 数据/操作 | 当前实际内容 | 约束 |
|---|---|---|
| `InputEvent` | Activate、Deactivate、FocusLost、Key、SetMode、CommitSucceeded、CommitFailed、RetryCommit | 平台按事件含义归一；不携带 HWND/COM 指针 |
| `Key` | Char、Backspace、Space、Enter、Escape、方向及翻页 | 当前没有修饰键、物理扫描码、按键预判或原生事件 ID |
| `InputFrame` | state、mode、revision、preedit、cursor、完整候选、高亮、页开始和页大小 | 候选为核心已排序的有界数组，高亮是全局索引 |
| `Candidate` | id、text、annotation、source、frequency、match_kind | text 是提交内容，annotation 不得上屏或参与平台排序 |
| `CommitRequest` | id、text | 同一引擎内 ID 不复用；不同引擎可从同一 ID 开始，不能单靠 id 路由 |
| `EventResult` | handled、frame、可选 commit 和 error | 有错误时先分流错误；`handled=false` 不等于可以丢弃或随意重放原事件 |
| `InputEngine` | new、handle、frame、failed_commit | 宿主独占可变调用；无网络、文件、COM 或 UI |

每个事件由宿主原子地送入一个引擎，取得独立快照。平台不向帧数组写入新候选、不重新排序、不把注释拼入 text。不同输入策略和语言模型通过未来核心策略接口演进；保持输出契约可兼容时无需复制平台业务逻辑。

## 2. 宿主信封与身份

以下是宿主需要实现的概念 schema，**不是声称 crate 已提供的公开类型**：

```text
HostSession = { service_instance_id, context_epoch }
NativeEvent = { session, native_sequence, normalized_event, modifiers, admission }
FrameEnvelope = { session, revision, frame }
CommitEnvelope = { session, request_id, revision, text, transaction_state }
CommitOutcome = Applied | NotApplied | Unknown
```

1. `service_instance_id` 标识一次服务实例；重建引擎必须换值。`context_epoch` 在获得新的原生文档/组合上下文、失焦、上下文出栈或销毁时递增。用自身 token 映射原生 context，不以裸 COM 地址长期充当唯一 ID。
2. 帧仅在 `session` 等于当前宿主上下文且 `revision` 仍等于其最新显示修订时应用。已消费旧版本的 UI、布局或注释回调直接失效。revision 不是时间戳，也不是整个进程唯一 ID。
3. 提交确认先匹配完整 `session + request_id`，再调用核心成功/失败事件。即使不同引擎恰好都有 request_id=1，也不得跨会话确认。
4. 失焦或停用时先使信封失效，再取消核心和候选 UI，最后释放原生资源。已经失效的异步编辑会话回调在操作原文档前也必须检查 session；仅在回调后过滤核心 ack 不足以阻止错误上屏。
5. ID 耗尽即停用该实例并显式恢复，不饱和后重复使用。原型核心已有修订号/提交号耗尽保护。

## 3. 按键接管、修饰键和顺序

原生预判不得调用会改变状态的 `InputEngine::handle`。TSF 的 `OnTestKeyDown` 判断是否接管，`OnKeyDown` 才处理实际事件；二者分别返回是否将处理和是否已处理。[Microsoft 按键预判](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfkeyeventsink-ontestkeydown)、[实际处理](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfkeyeventsink-onkeydown)。

宿主拥有同一来源严格递增的 `native_sequence`，只用于关联同一次原生事件的测试/执行/重复回调。不能把 WPARAM+LPARAM 相同视为重复，因为用户自动连发是合法新事件。每次合法重复按键分配新序号。重复执行回调沿用原序号，不能再次更新预编辑或再次请求提交。

Ctrl/Alt/Win 等未注册的快捷键不转换成 `Char`，交回目标应用；Shift/Caps Lock 根据键盘布局及模式生成字符，不能把快捷键文本化。模式切换只由显式已注册命令生成 `SetMode`，默认快捷键仍待产品验证。核心 v1 没有只读 `would_handle` API，CL-007 必须实现并测试宿主 admission 逻辑；不得通过运行核心再回滚来预判。

核心等待提交确认时，宿主不能继续派发普通输入。宿主可在同一 context 保留已接管事件的有界 FIFO，正常确认后按原顺序重派一次。CL-007 首轮预算为 32 个已接管事件，限额是宿主初始预算，不是最终产品输入上限；须以压力实测调整。未取得所有权的事件不得悄悄放入 FIFO。测试模型的满队列操作返回拒绝并保留调用方原事件，不声称已经实现 Windows 层溢出恢复。

生产宿主必须在队列满前进入明确恢复状态：停止新的输入接管，保留既有有界事件和提交请求用于恢复提示，取消可安全取消的会话，向用户提供未应用文本恢复方式；结果未知时不能自动改写文档或模拟键盘重发。具体 TSF 返回值、恢复 UI 与原文顺序须通过 CL-007/CL-010 故障注入验证，未经验证禁止发布。队列满、超时和异步调度本身都不代表目标文档未修改。

## 4. 组合帧与文本单位

当前 preedit 仅 ASCII 全拼，cursor 是 UTF-8 字节偏移；因此当前字符索引、ASCII 字节位置和 UTF-16 code unit 位置数值一致。宿主仍应显式执行 `preedit[..cursor].encode_utf16().count()` 并验证字符边界，不把 UTF-8 偏移直接当成通用 Windows 文本偏移。

提交 text、中文候选和 annotation 可包含 Unicode。它们的 UTF-16 长度必须独立计算；例如“云🙂”是 2 个 Unicode 标量、3 个 UTF-16 code units。后续支持 Unicode 预编辑或混合输入时，先升级光标/选择区契约并验证字素边界，再改变核心输入模型。

平台在编辑会话中维护自己的组合 range、显示属性及选择范围，更新目标组合文本；`InputFrame` 不含原生 range。候选窗口按当前帧/当前 context 的光标布局定位，窗口不能夺取应用输入焦点。当前 v1 尚无任意光标区间选择、外部编辑重基、鼠标候选选择、候选异步注释更新和 surrounding-text API；这些扩展须独立版本化并补测试，不能假装已由一个 cursor 字段覆盖。

## 5. 提交三态与确认

| 宿主结果 | 判定依据 | 对现有核心的操作 | 后续输入 |
|---|---|---|---|
| Applied | 实际编辑步骤在当前有效 context 完成，原生提交事务按既定验证点确认 | `CommitSucceeded { id }`，成功后清空组合 | FIFO 顺序继续 |
| NotApplied | 确定编辑未发生，例如在写入前明确拒绝；不得仅根据一个 HRESULT 猜测 | `CommitFailed { id }`；failed_commit 保留完整请求 | 暂停，只有用户显式 RetryCommit 或取消 |
| Unknown | 写入后发生错误、无法确认是否应用、超时或上下文/事务状态无法判定 | 不伪造成功或可重试失败；宿主隔离上下文，核心仍 pending | 暂停，不自动重试；明确恢复/取消后换 epoch |

当前 `InputEvent` 没有 Unknown，宿主隔离是 v1 的必要责任。Unknown 以后若得到同一有效事务的确定 Applied/NotApplied 才可转为相应 ack；上下文已被用户放弃时，迟到确定结果也不回流到新引擎。取消只清理输入法状态，不撤销已经可能写入目标应用的文本。

TSF 的 `RequestEditSession` 方法 HRESULT 与 `phrSession` 含义不同；方法调用成功或者 `phrSession=TF_S_ASYNC` 不能据此发送 `CommitSucceeded`。排队成功后仍等待实际 `DoEditSession`，检查有效上下文及写操作进度。读/写权限与同步/异步要求由宿主决定，不能阻塞 TSF 线程等待一个回调。[Microsoft RequestEditSession](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-requesteditsession)。

## 6. 生命周期、诊断和并发

服务激活成功才创建/激活可用引擎；Deactivate、FocusLost、模式切换与上下文销毁按现有核心规则取消未确认组合。焦点迁移时不把旧 FIFO 搬到新应用。用户明确取消、停用或关闭上下文可放弃其临时未应用输入；宿主记录恢复状态而不保存完整输入历史。

平台 COM 指针、edit cookie、HWND、布局对象只归属平台模块；核心只处理值类型。TSF 所属线程上串行管理 context 与编辑会话，后台翻译/模型只返回版本化值消息。宿主不可持有核心可变借用跨越原生调用，以防 COM 重入导致重复提交。日志默认只含模块、错误码、状态、epoch/revision/request_id 和耗时；不得记录 preedit、候选、提交全文和密钥。

## 7. 契约验证与实现差距

| 契约条款 | 当前证据 | 后续平台证据 |
|---|---|---|
| 会话身份与旧帧过滤 | `platform_contract.rs` 使用真实核心验证两个引擎 ID 碰撞仍被宿主过滤 | 原生 context 切换/销毁后的迟到 edit callback |
| 事件接管、去重与 FIFO | 模型验证预判不变状态、重复序号只消费一次、pending 排队后按序恢复、快捷键透传 | `OnTestKeyDown/OnKeyDown/OnKeyUp` 与合法自动连发、队列溢出恢复 |
| Applied / NotApplied / Unknown | 真实核心成功、失败保留/显式重试；模型 Unknown 不 ack、不重试 | 写入前/写入后/组合结束失败、异步会话迟到及 COM 重入 |
| 生命周期 | 模型焦点取消、队列清理与 epoch 失效；已有核心单测 | 输入法切换、窗口关闭、document stack、退出清理 |
| UTF-16 和候选只读 | UTF-16 单位与候选/注释分离断言；已有固定候选语料 | range/选择区/位置/多屏 DPI/应用自绘候选 |

`platform_contract.rs` 只是可重放的宿主协议测试模型，真实模块为 `cloudleaf-input`。它不实现 COM、真实焦点、Windows 消息、候选窗口或文档编辑，不能作为 CL-007/CL-010 的完成证据。契约设计可作为并行开发基线；正式 Windows 适配尚未开始。新增生产 adapter 必须复用这些测试意图并补平台级验证，不直接把测试模型当作发布代码。
