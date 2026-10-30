# Round 12：Windows 翻译面板与安全设置

2026-10-31。实际基线 `3946d22295591aa080ff7bea007f9f410b245fc0`，分支 `codex/translation-round11`、PR #5 Draft。工作区干净，远程 main 仍 `a564fd47b0f054ff360592af3ccb8d744021160f`。本设计先提交，随后实现；原四个方向、默认输入引擎、TSF 和长期测试不改。

## 实际能力与缺口

复用 `TranslationService`、Provider/Request/Result、取消、`TranslationPanel`、研究词典和在线 HTTP 提供者。当前只存在会话 SecretStore、复制 intent 和独立 CLI；没有原生窗口、设置文件、Credential Manager、系统剪贴板。离线模型没有 backend 或权重。原始 gloss 是 ResearchOnly，不是正式语义或整句翻译。

## 方案与文件范围

新增 `apps/cloudleaf-translation-panel`，binary `cloudleaf-translation-panel.exe`。`settings.rs` 管版本化非敏感配置、迁移和恢复；`controller.rs` 管 UI 状态及现有服务接入；`setup.rs` 在单一后台设置线程读写配置、载入词典、构造服务和读写凭据；`platform.rs`/`windows_store.rs` 封装系统凭据及剪贴板；`windows_ui.rs` 只做 HWND/标准控件、内存文本和非阻塞轮询。Linux/macOS binary 明确返回当前平台不支持 GUI，平台存储返回 Unsupported，不写明文。

采用原生 Win32 EDIT/COMBOBOX/BUTTON/STATIC：无需浏览器/GPU runtime、系统控件提供键盘导航与 UIA。需自行维护布局、DPI 与句柄安全；不能把体积/内存优势写成已测数值。依赖仅 target(windows) 精确 `windows-sys =0.61.2`，MIT OR Apache-2.0、声明 MSRV1.71，已经在 R11 锁和许可原件清单中；新增相应 feature，复用 serde/serde_json/zeroize，不升级既有依赖。FFI unsafe 仅在新 app 的平台模块，不能降低共享核心的 forbid。workspace/Cargo.lock/审计政策及 CI 按实际变化更新。

## 线程与生命周期

窗口线程只处理控件文本、内存状态、service submit/poll/cancel 和显式剪贴板写入，不做网络/磁盘/大型 JSON，不等待仍运行的worker；仅在workers_finished后回收已完成handle。设置线程有界命令/结果队列，后台准备 provider/service 后移交给 controller；不建立第二套翻译执行器。翻译仍只由现有固定 worker 执行。

配置替换时先取消当前任务并使上下文 epoch 失效，停止旧 service；证明原 worker 退出后才创建新 service，防止多次设置产生无限遗留 DNS/model worker。为此给原 service 增加非阻塞 `begin_shutdown`/`workers_finished`；显式 shutdown 继续 join，旧测试全部保留。等待旧 worker 期间显示重新配置/Busy，不偷偷扩预算。设置准备至多一个在途，新配置在 UI 中仍可编辑，应用时给出忙状态。

原文、方向、模式、提供者、设置变化、失焦和取消均使旧 task/context 失效。所有回调使用发起时捕获的 context、任务及配置版本；失败也必须经过 TranslationPanel.update。先成功后迟到错误不能覆盖；同文本不同配置不得共享旧授权。任务终态和耗时来自真实 service snapshot / 实际 Instant，不模拟进度。

关闭先发取消、停止服务和设置线程，窗口通过定时器轮询最多2秒，绝不在消息线程等待 join。合作路径记录实际退出；若后台尚未退出，关闭独立进程并由操作系统回收，不能声称所有 worker 已正常 join。窗口/字体/剪贴板句柄在所属线程处理，不涉及 COM/TSF。

Win32 SendMessage/SetWindowText 可同步重入：userdata 指向由 run 函数持有的 RefCell，回调 try_borrow_mut，内部程序更新通知不再次借用；在 HWND销毁回调内不释放仍被回调借用的状态。顶层 WM_ACTIVATE 而非子 EDIT 的失焦通知用于会话失效。多行 WM_SETTEXT 不保证 EN_CHANGE，因此程序清空必须显式更新 controller。

## 界面与设置

多行原文与只读结果、方向（中英/英中/Auto到明确目标）、词/短语/整句、提供者、开始/重译/取消/清空/复制/错误详情。按 capability 禁用词典整句；离线模型显示未安装。显示真实 Idle/Queued/Running/Completed/Cancelled/TimedOut/Failed/ProviderUnavailable，以及准备/关闭状态、来源、ResearchOnly/Generated 和实测耗时。输入上限16KiB UTF8，Unicode错误明确反馈。粘贴只由用户对标准EDIT执行，不自动读取剪贴板。

设置区域包含 catalog 路径、endpoint、model、默认方向、连接/总超时、缓存、联网/本机HTTP开关、API Key保存/删除、应用/恢复默认。每次在线发送需用户勾选当前提供者/端点授权，并绑定 provider.authorization；设置或端点变化立即清除此同意。默认离线；启动/迁移/恢复均不自动启用联网，即使保存的配置或密钥存在。无默认服务，不改变 redirect/proxy/HTTPS限制。

默认设置路径 `%LOCALAPPDATA%/Cloudleaf/TranslationPanel/settings.json`，可用 `--settings-dir` 隔离测试；`--dictionary` 仅显式提供研究 catalog。schema2 严格拒绝未知字段、密钥字段、超限/损坏；schema1仅迁移已知非敏感字段，关闭所有网络授权。后台使用临时文件、sync、同目录替换和有效备份；损坏主文件尝试有效备份，否则安全默认并反馈，保留原文件直到用户明确应用。恢复不授予数据/网络权限。中断测试不是物理断电证明。

## 凭据与剪贴板

`WindowsCredentialStore` 实现现有 SecretStore.get，另提供显式 set/delete；namespace限定 Cloudleaf 面板，可传独立测试 namespace。Generic Credential 使用当前用户本机持久范围。target用UTF16 NUL结尾；blob沿用Secret的UTF8字节，不擅自当UTF16。平台入口限制2560字节；读取核对类型/长度/UTF8/Secret约束，固定分类错误，不输出引用或正文。CredReadW分配由guard擦除blob后CredFree；临时秘密/UTF16缓冲清理。密码EDIT只掩码，不宣称Windows内部缓存均已擦净，保存/取消立即清空编辑框。无效Secret入口也清理自有字符串。

剪贴板仅用户点击复制时写 CF_UNICODETEXT：UTF16/NUL/CRLF，GMEM_MOVEABLE，分配/填充后再OpenClipboard真实HWND，Empty/Set成功后转交HGLOBAL，失败释放并保留译文。占用立即反馈，不循环等待，不监听或自动读系统剪贴板。研究内容明确标记，复制不变成正式数据。本轮不向其他应用自动输入。

## 设计先行与验收

先固定本设计，补非阻塞关闭、配置/迁移/恢复、控制器取消/配置/失焦/旧结果、授权、离线零连接和平台资源回归，再实现。Windows真实凭据只用唯一fixture名称并finally删除，不访问用户API凭据。剪贴板测试使用隔离拥有窗口与原创文本，实际写入可能替换当前剪贴板，应在真实GUI用户点击路径验证并明确记录。

全套fmt/Clippy/all-targets/doc/release/Python/workspace与依赖审计逐命令记退出码；产出真实exe及SHA，Windows/Linux新head push CI。具备交互桌面时用Computer Use观察真实窗口、输入/查询/方向/取消/复制/错误/设置及关闭，保留截图/步骤；看不到GUI则明确未验收。不得使用自动化手段修改系统或app安全/隐私设置；真实凭据隔离回归通过工程测试执行，真实联网开关由既有授权的本地测试配置/状态机验证，不借GUI绕过该限制。

最终新增 `docs/reports/translation-round12-status.md` 和 `.local/round12-delivery-receipt.json`，区分实现、自动化、可观察GUI、外部质量与尚未实现。模型只读审查独立记录版本/许可/来源，禁止下载大权重或编造推理。所有五PR保持Draft、main不合并、CL-010关闭。
