# Windows 完整平台适配设计

- 任务：CL-006 设计输入；CL-007 实现，CL-010 完整链路验收。
- 状态：2026-10-22 设计基线。未实现 TSF COM 服务、候选窗口、注册/安装和真实应用集成。
- 核心契约：[`core-platform-contract.md`](core-platform-contract.md)。

Windows 方向交付完整的系统输入法适配能力：稳定事件接管、组合文本、候选交互、可靠提交、焦点与资源生命周期、可安装升级及真实应用兼容性。接口风险验证只是一项前置实验，不代替上述交付或降低算法与词库要求。

## 1. 当前源码与边界

现有六个 crate 中，types、pinyin、dictionary、candidate、input 为共享核心，CLI 为测试宿主。输入引擎已经有原始预编辑、光标、候选帧、修订号、提交请求及成功/失败确认；候选最多50条、输入最多64个ASCII字符只是当前原型边界。没有 HWND、COM 绑定、原生按键映射、TSF range、系统候选 UI 或已授权生产词库。

平台层只能维护 Windows 原生状态和消费核心结果。拼音解析、候选搜索、排序、学习与词典索引在 Rust 公共核心迭代；TSF 不复制算法，也不为适配现有简易排序而锁死未来候选结构。正式词典、算法评测和 Windows 接入按稳定契约并行推进。

## 2. 实现模块与依赖

| 未来模块 | 输入/职责 | 输出/边界 | 验收证据 |
|---|---|---|---|
| `apps/windows-ime` | COM DLL 入口、服务实例、用户语言配置入口 | 调用 adapter；不得运行数据管线/网络模型 | 加载/卸载/位数、引用计数和异常边界 |
| `adapters/windows/session` | thread manager/client ID、document/context 生命周期、epoch | 核心会话拥有者和失效 token | 激活失败回滚、失焦/出栈/销毁/停用清理 |
| `adapters/windows/keys` | 原生键、布局、修饰键、测试/执行回调 | 只读 admission、唯一执行序号、核心事件 | 快捷键/自动连发/重复回调/模式键矩阵 |
| `adapters/windows/composition` | 核心 frame、原生 range 与 edit session | 组合更新、显示属性、选择区及提交三态 | 读写锁、同步/异步、失败/重入/迟到注入 |
| `adapters/windows/candidate_ui` | 当前 context/revision 的完整候选与布局 | Windows UIElement 与必要自绘窗口 | 数字/翻页/高亮、系统自绘、DPI/焦点/无障碍 |
| `adapters/windows/registration` | 固定服务/语言 profile 标识、安装版本 | 可重放注册、升级、卸载与回滚 | 普通用户/管理员、x64/其他位数支持声明 |
| `tests/compatibility/windows` | 应用矩阵、事件/日志与屏幕时间标记 | 真实行为、端到端延迟、资源与崩溃报告 | CL-010 全链路证据 |

模块路径是实施规划，尚未创建空生产模块来冒充进度。Rust Windows 绑定库先审查许可、版本、所需 COM 接口和维护情况；本轮不引入依赖。先以 Rust 实现薄适配，只有明确接口/可靠性问题且有证据时再评审最薄 C++ 桥接；桥接不包含业务算法。Windows SDK 和绑定局限不得传入公共核心。

## 3. COM 与生命周期

文本服务由 TSF 创建，`Activate` 接收其所属 `ITfThreadMgr` 和 `TfClientId`；保存必要引用及 sink cookie，注册失败按逆序撤销。`Deactivate` 取消会话、关闭 UI、撤销 sinks，并释放激活取得的 thread manager 引用。不能把应用自行创建 thread manager 的流程照搬到输入法服务。[Microsoft Activate](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itftextinputprocessor-activate)、[Deactivate](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itftextinputprocessor-deactivate)、[Thread Manager](https://learn.microsoft.com/en-us/windows/win32/tsf/thread-manager)。

为每个拥有原生上下文的服务实例明确线程/apartment 归属；不把裸 COM 指针作为普通 Send/Sync 值交给 worker，不从翻译线程直接更新 document。需要跨 apartment 时使用 COM 封送。in-process DLL 遵循宿主 COM 环境和注册 threading model，不能在 DLL 任意入口改变宿主 apartment。上述是本项目的保守实现约束，不假定所有 TSF 对象均可自由跨线程调用。[Microsoft COM apartment 规则](https://learn.microsoft.com/en-us/windows/win32/com/single-threaded-apartments)。

thread/document/context 的 sink 成对注册和撤销，RAII 持有 cookie 与引用。native 调用可能重入；不要跨原生调用持有核心可变访问或等待另一个线程。维护一次操作 ID 与执行中标记，重入事件只能按 admission 决策保留或交回，不重复操作引擎。

`OnSetFocus` 的新/旧 document manager 都可能为空；以会话 epoch 切换处理，空新焦点也要清理。context 出栈或销毁与 document focus 变化分别处理，不能仅观察前台 HWND 推断组合仍有效。[Microsoft OnSetFocus](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfthreadmgreventsink-onsetfocus)。

## 4. 输入事件与组合

按 `core-platform-contract.md` 实现预判/执行分离、序号去重、修饰键透传、键盘布局和 Caps/Shift 处理。对 Enter、Space、数字选词明确消费同一次原生事件，避免提交后再产生换行、空格或数字。应用快捷键、按键抬起和已处理按键的对应关系要测试，不以“所有键都吃掉”保证表面演示。

取得合法写权限后，在当前 context 中创建和维护 composition range，更新预编辑、显示属性及选择位置。`ITfContextComposition` 是创建/操作 composition 的系统接口入口；composition 对象、range 和结束通知属于平台生命周期，不写进核心词典或候选模块。[Microsoft composition 接口](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfcontextcomposition)。

外部编辑、选择变化、撤销重做、应用主动结束组合、只读/禁用输入上下文、密码字段和上下文销毁都需要明确策略。无法安全映射外部编辑时取消当前未确认组合并重建 epoch，不猜测提交位置。密码/敏感输入场景不进行候选上下文学习或翻译采集；具体输入 scope 映射与应用行为由实现时再核实系统接口并记录。

## 5. 编辑会话与提交可靠性

`RequestEditSession` 的同步/异步参数与双层 HRESULT 必须分别处理。按键处理可评估同步请求，不能要求所有系统通知都允许同步读写；官方说明 `OnEndEdit`、`OnLayoutChange` 和 `OnStatusChange` 内的同步读写请求会失败。异步请求接受不等于完成；上下文不在 document stack、只读、锁冲突等有不同结果。[Microsoft RequestEditSession](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-requesteditsession)。

实现事务阶段：创建信封 → 请求会话 → 进入 `DoEditSession` 后再验 session → 验 range/权限 → 更新最终文本 → 结束组合/选择收尾 → 分类 Applied/NotApplied/Unknown。每步记录操作阶段及固定错误码，不能写完整输入日志。HRESULT 失败可能发生在已有修改之后，因此不能统一映射 `CommitFailed` 并让用户重试。

核心进入 Committing 后暂停普通事件派发。适配器可接管有界 FIFO 并在成功确认后按序恢复；未知结果隔离，已知未写入结果才允许显式 RetryCommit。超时、失焦、关闭文档不自动重发文本。当前同步 CLI 不具备这些原生保证，必须通过真实编辑会话故障测试后才允许交付。

队列压力与恢复同样属于完整能力。CL-007 必须验证满队列前停止新接管、保留已接管未应用输入和用户恢复方式，不能把队列上限变成静默丢键。Unknown 的暂停不得通过阻塞原生线程/消息循环实现。

## 6. 候选 UI 与布局

核心帧提供顺序、稳定候选 ID、高亮及页信息；数字编号只是当前页投影。鼠标/无障碍选择也必须绑定 context、revision 和候选 ID，再由核心处理选词，不能让 UI 直接提交一个过期显示字符串。当前核心没有按候选 ID 选择的公开事件，因此需要在 CL-007 前补版本化事件/测试；不能以鼠标模拟多次 Down 绕过稳定性。

使用 `ITfUIElementMgr` 管理 UI 元素周期；`BeginUIElement` 的 `pbShow=false` 表示应用可能自行绘制，服务不能再显示自己的窗口。更新/结束必须配对，并使迟到回调失效。自绘与应用接管均呈现同一帧，不能只实现一个浮动窗口。[Microsoft BeginUIElement](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfuielementmgr-beginuielement)。

位置查询需要有效 read edit cookie；`GetTextExt` 返回屏幕坐标，`TS_E_NOLAYOUT` 以及不可见时的零矩形不能直接当作候选窗口正常位置。使用最近有效布局或暂时隐藏，并在布局更新后重试读取，不能抢焦点或同步自旋。[Microsoft GetTextExt](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontextview-gettextext)。

UI 覆盖多显示器/DPI、屏幕边缘、上下翻页、横/竖排候选、长词条与多义释义、键盘和读屏访问、缩放/高对比度主题。长释义放独立详情区域，不压缩提交文本或改变排序；注释迟到只在匹配 revision+candidate ID 时更新。与翻译任务分开，任何释义/模型失败不阻断候选和提交。

## 7. 验证矩阵与退出标准

| 类别 | 必测情形 | 自动化/实测证据 | 当前状态 |
|---|---|---|---|
| 加载与生命周期 | 激活、停用、反复切换、部分初始化失败、卸载/升级 | 引用计数/配对sink审计、进程退出、注册回滚 | 未实现 |
| 原生按键 | 字母/自动连发、Ctrl/Alt/Win、Caps/Shift、布局切换、测试/执行重复 | Windows宿主测试+真实应用，零意外吞键/重复提交 | 未实现；已有核心协议模型 |
| 组合与提交 | 编辑/光标/退格/取消，Enter/Space/数字、外部选择改变、撤销重做 | 精确文本、选择范围、组合生命周期断言 | 未实现 |
| 异步与故障 | lock/read-only、queued callback、上下文失效、写入后失败、重入、超时、满队列 | Applied/NotApplied/Unknown分类及恢复，不盲目重试 | 未实现；模型验证语义 |
| 候选与位置 | 翻页、鼠标/读屏、pbShow两值、长释义、多屏DPI/边缘/无布局 | 候选次序与frame一致、UI生命周期、焦点不被抢 | 未实现 |
| 真实应用 | 记事本、浏览器地址栏/单多行网页、办公软件、多行编辑器、权限切换 | OS/app版本、复现脚本、文本结果、问题分级 | 未执行 |
| 性能与稳定性 | 正式词库、长串、持续输入、并行翻译、休眠恢复、长时间资源观察 | 核心/TSF/布局/UI/提交分段及端到端分位数、内存/句柄曲线 | 未执行 |
| 隐私与独立性 | 离线、翻译进程退出、密码/敏感scope、日志和凭据 | 故障注入、脱敏日志审计、基础输入不受影响 | 未执行 |

测试机先记录本次已有 Windows 11 环境，但支持范围、CPU架构、最低Windows版本及应用版本必须在实现验证后明确；不把CI编译成功当作Windows真实兼容性。报告要记录输入事件、预编辑、最终文本、候选顺序、焦点/epoch、成功/失败阶段、可重现条件和隐私处理。

CL-007 退出要求为以上系统能力均有实现和相应平台测试；CL-010 必须完成产品范围内应用矩阵、正式数据与算法集成、端到端性能和故障恢复，无高严重度丢字、重复提交、卡死或残留候选。任何尚未支持的平台情形要明确登记，不以一个“可输入”的技术演示替代验收。

## 8. 开始实现前仍需解决的契约扩展

当前契约已可描述异步提交和上下文隔离，但核心API对完整产品仍有差距：任意候选ID选择、原生预判、Unicode混合预编辑与选择区、外部编辑通知、上下文文本的隐私可控输入、可配置限额及异步注释更新。每项先更新类型/行为文档，再加入共享测试和原生测试。算法成熟度、正式词库质量、远程CI和Windows平台实现分别验收，不能互相代替。
