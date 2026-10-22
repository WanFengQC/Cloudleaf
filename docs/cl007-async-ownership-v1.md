# CL-007 A1–A4 异步接管与恢复约定

状态：实现前设计；2026-10-22。继承核心平台契约 v1/v2，替换上一阶段“仅同步按键”的适配限制，不改变核心候选算法。A1–A4 获得真实退出证据之前继续禁止注册、激活。

## A1：原生事件及顺序

`NativeEnvelope` 包含 service instance、context epoch、单调 sequence、原 WPARAM/LPARAM、键盘布局标识、修饰键快照、归一后的 Key、重复次数、取得 ownership 的标记。测试预判只读现有帧、投影帧、容量、健康及输入 scope 证明，不调用任一引擎 handle，不克隆后运行再回滚。相同键码/bit30/时间不表示相同事件；独立 count=1 消息分别编号和保留。

Host 在激活路径预加载词典并建立执行引擎与投影引擎；不在输入路径读取磁盘、访问网络或复制词典。只有 OnKeyDown 决定取得 ownership 后才给投影引擎发事件。投影引擎的模拟成功只用于回答后续事件的 IME 语义，不能确认宿主写入、显示候选、更新学习或释放实际 ownership。实际执行引擎逐序处理队首并与投影计划比较，未处理或不一致立即隔离。

| 事件分类 | 接管与执行 | 无法执行时 |
|---|---|---|
| 投影核心明确 Handle 的单个 IME 键 | 可接管，保存原信封并排入 FIFO；实际写锁中重新验证 scope、身份、先后次序和核心结果 | 保全未应用材料，不能因核心后来 handled=false 而丢弃 |
| Space/Enter/Left/数字等在投影中 PassThrough 的原生命令 | 不能转换为文本/换行/编辑动作；先同步申请 barrier 完成所有更早已接管事件，随后原事件仍交给应用 | 拒锁时封闭未开始回调、隔离原有 ledger 后透传原生事件；已开始写入不得伪装取消，按 Unknown 处理 |
| Ctrl/Alt/Win、未支持按键、合并重复消息 | 保留完整原始重复次数，整体走原生命令 barrier；不处理一个重复却声称消费整包 | 不 SendInput、不 PostMessage 假造键盘事件，不承诺重放应用命令 |
| 候选 Finalize/取消 | 先同步 barrier，revision+id 再验证；候选拒锁明确失败、供用户重试，不排入可被原生键越过的异步提交 | stale 参数错不能隔离健康会话 |

FIFO 上限32个已接管事件；入队前检查全部容量，满时不吞新事件，不覆盖旧事件，进入恢复。默认每个事务500毫秒；deadline不是 Applied/NotApplied 的替代证据。queue head、事务、回调和确认均绑定 instance/epoch/sequence；每个回调只运行一次。普通新键不能绕过旧队列，TSF 同步锁优先于已排异步锁，所以 barrier 必须显式先消费旧 FIFO。

输入 scope 证明只属于该 context/epoch，外部文本/选区/属性编辑、context 切换、焦点切换和证明过期即失效。焦点时可异步读锁刷新证明；scope 未知或证明失效时不直接异步接管，等待实际锁内验证。每次实际写锁仍重查 readonly/password scope。scope 变化发生在已取得 ownership 后时隔离保全，不在密码目标写入 IME 文本。失败与恢复 UI 不自动显示或记录原文。

## A2：事务状态和用户恢复

每条已接管事件经过 `OwnedQueued -> Started -> Writing -> Applied`，或从 Queued/Started 转 `NotApplied/Isolated`，从 Writing 转 `Unknown`。Applied 只在实际 SetText、选区、EndComposition 完成且身份与健康仍有效后记录；前面步骤成功不代表提交成功。只在写入前可证明未发生编辑时报告 NotApplied；进入 SetText 调用即视为可能写入，超时、重入失焦和异常都不得自动重发。

ledger 只保留当前未完成的有界所有权材料，不写日志、不落盘、不跨应用自动搬运。隔离、满队列、barrier拒锁、失焦或超时后提供可实际访问的恢复窗口，列明每个 sequence/outcome 与原生动作的区别。用户点击“查看/复制恢复材料”才显示或复制原始键序列及可证明未应用的提交/预编辑，结果未知单独标注“可能已写入，禁止自动重试”；复制是用户显式操作，不代表回放成功。关闭窗口是明确放弃临时材料。恢复材料不直接自动插入新焦点文档。

恢复窗口由平台自己的消息循环与窗口处理函数驱动，不在按键路径弹模态对话框等待用户。控件只负责展示与复制，不实现拼音算法。复制失败保留可选择的只读文本供用户操作；不循环等待剪贴板。

恢复材料的所有权独立于服务 Host：失焦或停用不能自动丢弃未确认材料，退役窗口用独立模块租约保留至用户明确关闭；因此有材料时 DLL 保持不可卸载是有意保全，不得报告为已完成全部退出。后台通知线程确认退出后，创建窗口的 STA 才可清理无材料窗口。每次新 offer 立即清空已经显示的旧材料，原文仍须重新点击查看；记录 offer 时 epoch，关闭旧材料窗口不能取消新上下文。多个 epoch 的待恢复材料分别标识、保留，不把旧输入自动移交新文档；达到有界保全预算时停止新接管，不能覆盖尚未放弃的材料。

## A3：独立 watchdog

watchdog 使用独立线程及原子状态，只传有界无文本信号，不持有 Rc/COM/核心引擎，不跨线程调用 apartment 接口。工作检测线程与健康检测线程互相监督 heartbeat；启动失败、线程退出/panic、监测自身失效均停止新的异步 admission。所有 deadline 和写入阶段通过原子状态关联 operation ID，不能让旧超时覆盖新操作。

Queued 超时通过 CAS 封闭回调入口；Writing 超时只标 Unknown 和暂停，不碰宿主 range，不盲重试。工作线程向 STA 自有窗口发无文本通知；STA 空闲时即使没有用户输入也进入恢复。STA 被卡在宿主 COM 调用时，独立线程仍能报告原子检测状态；只有 STA 回来后才能关闭 UI/结束组合，报告必须区别“检测完成”与“宿主恢复完成”。监测线程都不可用或 OS 调度暂停时不存在绝对实时保证，须准确报告检测健康与恢复延迟。

## A4：退出与资源

每个服务实例的线程、通知窗口、候选窗口、COM sink、edit callback、恢复窗口与后台任务分别登记生命周期。先原子停止 admission/使 epoch 失效，再让 STA detach 旧 COM 资源；后台线程只处理停止标志并释放自身 ModuleLease，不能等待 STA。STA 不阻塞 join 一个可能失效的线程；线程未退出时 DLLCanUnloadNow 必须保持 S_FALSE，不能假装资源已释放。

后台模块保活进一步收紧：worker 不释放最后 ModuleLease，owner 持有 JoinHandle 与 lease；停用后尚未退出的任务转移到有界 retired 集合。DllCanUnloadNow 先用真实 OS 线程 HANDLE 的零等待确认 signaled，之后才 join 并释放 lease/handle；不能仅凭 is_finished 或 worker 尾部析构推断 DLL 已安全退出。启动部分失败也必须进入该集合，集合容量在启动前预留，满时拒绝新监测。停止后的 monitor 不健康；重新激活创建新监测。通知投递失败是可观察故障，停止异步 admission，并有界重复尝试发送无文本恢复信号。

测试分别验证资源取得/释放对称、Factory、Activate中间失败回滚、Deactivate、晚到callback、持有的跨实例对象、线程退出、窗口销毁、COM sink撤销、后台ModuleLease与DLL可卸载结果。弱引用或一个计数归零不等于所有资源完成。跨线程只能操作原子，故障注入不得在错误线程直接调用非agile COM 对象来伪造支持。

## 实施与证据

新代码限制在 `adapters/windows/{ownership,watchdog,recovery,...}.rs` 及平台入口；共享核心不引入 Windows 依赖。保留上一轮所有故障测试，将新测试直接接入生产 OnTest/OnKey/RequestEditSession 与真实 COM fixture，不用独立模型代替。独立 thread/watchdog 检测、STA通知恢复和资源退出分别测量，原型64条与模拟宿主成绩继续不等于真实 Windows 性能。完成 README 全套检查、依赖审计、release 基准和远端双平台 CI 后更新状态；真实应用验收仍须单独放行。
