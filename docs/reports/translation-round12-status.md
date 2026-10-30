# Round 12：Windows 原生翻译面板与安全设置

2026-10-31。本轮实际交付可构建、可启动和操作的独立 Windows 面板，接入现有 TranslationService，而非另一套翻译调度器。真实桌面已观察中文研究查询、复制和方向切换；完整 GUI 验收仍为 partial：桌面外部输入中断后停止自动化，最终 Release 的全部按钮未逐项人工验收。正式语义、语言质量、模型推理与系统输入法仍未交付。

## 版本、关系与文件

- 基线：`3946d22295591aa080ff7bea007f9f410b245fc0`，实际 fetch 后本地/远程一致，工作区原先干净。
- 设计先行：`2c05aca`；原生面板/凭据/设置实现：`9e5192b`；真实控件方向复位反例：`156a602`；本轮完整被测实现 **`d50e41a36a002e9e36140a2893484c05bae89fba`**。
- 继续原工作区 `C:\Users\EDY\.codex\worktrees\translation-round11\Cloudleaf` 和分支 `codex/translation-round11`，仅普通 push 更新 [Draft PR #5](https://github.com/WanFengQC/Cloudleaf/pull/5)。本报告/证据另作文档提交；最终 HEAD 与其自己的 push CI 放在 `.local/round12-delivery-receipt.json` 及 PR 正文，不能用旧 CI 代表最终文档提交。
- main 仍以 `a564fd47b0f054ff360592af3ccb8d744021160f` 为检查点；原四个方向不得改动。没有合并、rebase、强推、Ready、TSF 操作或8h/24h长测。

| 实际文件 | 修改目的 |
|---|---|
| `apps/cloudleaf-translation-panel/src/main.rs`、`windows_ui.rs` | 真正 Win32 binary、标准控件、DPI/键盘导航、多行 Unicode、显示/取消/复制/设置操作和消息循环 |
| `controller.rs`、`setup.rs` | 复用现有 service/panel 的生命周期和授权；有界后台设置线程负责配置/词典/凭据 IO |
| `platform.rs` | Windows Credential Manager、CF_UNICODETEXT；非 Windows Unsupported，无明文回退 |
| `settings.rs` | 严格 schema2、schema1迁移、原子替换/有效备份/损坏恢复，重启保持离线 |
| app 两组集成测试、原生控件单元测试 | 真实本地HTTP、实际Windows凭据和控件，状态/设置/剪贴板边界 |
| `crates/cloudleaf-translation/src/scheduler.rs` | 增加 begin_shutdown/workers_finished，停止接单、取消与真实退出判定 |
| `secret.rs`、`tests/nonblocking_shutdown.rs` | 无效owned secret也清零；不合作/异常退出的资源回归 |
| Cargo、精确政策、CI、README/许可说明 | 新增 app 成员、现有Windows绑定features、Windows EXE产物上传及使用方式 |

完整实现路径见[证据索引](translation-round12-evidence/index.json)。默认输入候选引擎、共享类型、原正式数据及 Round1–11 历史报告/测试不改。四个原工作区只用于隔离，不代表不同开发者；平台实现阶段单一写入者，窗口代码先在工作区外准备，交还写入权后串行合入。Codex 技术复核不冒充真人审核。

## GUI 技术与可执行文件

Win32 标准 EDIT/COMBOBOX/BUTTON/STATIC，原生窗口消息循环，无浏览器运行时、GPU框架或新增GUI库。实现了标准Tab导航、原文多行Enter/粘贴、16KiB UTF8限制、严格UTF16转换、DPI缩放/调整、模式能力限制和明确错误。结果显示提供者、数据来源、OutputKind及ResearchOnly/Generated；Generated不代表质量验收。输入/词典/HTTP没有从窗口消息线程访问磁盘或网络。

```text
cargo build -p cloudleaf-translation-panel --release --locked --offline
target\release\cloudleaf-translation-panel.exe
cloudleaf-translation-panel.exe --dictionary PATH_TO_CATALOG_JSON --settings-dir PATH_TO_SETTINGS_DIRECTORY
```

本地因旧测试窗口仍运行而使用 `CARGO_TARGET_DIR=.local/round12-build`，从 Cargo JSON 实际返回产物路径：

```text
C:\Users\EDY\.codex\worktrees\translation-round11\Cloudleaf\.local\round12-build\release\cloudleaf-translation-panel.exe
```

该实现 Release 为 **3,863,040 B**，SHA-256 **`19e3b9c2ba2803f3b2844b17164f6a6b80d105de824ae8a7989ee73ac86b6502`**。这是本机MSVC构建身份，不假定与不同环境CI字节相同。启动代码与数据路径见 app README；没有随 EXE 自动下载词典或模型。

## 交互、线程与关闭

窗口使用同一个 TranslationService提交/轮询/取消，TranslationPanel校验task/context；词典准备、设置文件、服务构造及凭据管理均由一个有界设置worker承担。网络继续在现有固定translation worker执行。没有第二套翻译任务执行器。

原文、方向、模式、提供者/配置、取消或失焦使旧任务失效；成功和失败都经过发起上下文校验。配置revision及provider授权绑定，完全相同文本换配置不复用旧许可。每次发送许可在构造请求时消费。快速请求允许明确Busy，不扩大队列。状态是实际snapshot的Queued/Running/Completed/Cancelled/TimedOut/Failed/ProviderUnavailable，不用动画计时假装任务进度。耗时来自实际Instant，包含UI观察间隔，不是模型/网络纯延迟。

配置替换先停止旧service，workers_finished后才回收已完成handle并准备新service，避免DNS或不合作provider遗留预算后无限创建新池。等待旧worker期间显示准备/忙；设置编辑暂时禁用，原文仍可保留。关闭发取消、停止设置接单、定时器最多等2秒；若后台未退出则独立进程结束由OS回收，不宣称所有worker已join。没有跨线程释放COM或Windows控件。

## Windows 凭据和设置

真实 CredWriteW/ReadW/DeleteW，Generic Credential、当前用户本机持久范围。target使用Cloudleaf命名空间/UTF16，reference按端点SHA隔离；blob为Secret的UTF8字节，限2560B。读取核类型、长度、UTF8和Secret约束，guard擦除本地blob后CredFree。自有Secret及读取密码的UTF16临时缓冲清理，错误与Debug不显示密钥/引用/配置正文。密码EDIT只是掩码；不宣称擦净Windows内部所有副本。保存/删除在后台，UI立即清空密码输入。

实际隔离namespace测试覆盖保存、更新、删除、不存在、2560/2561B边界和坏UTF8；另通过controller→设置worker→真实凭据API完成保存/删除回归。均使用唯一fixture名称并清理，没有访问用户已有API凭据。没有真实外网调用或付费API消耗。

配置默认 `%LOCALAPPDATA%/Cloudleaf/TranslationPanel/settings.json`，支持测试目录覆盖。严格拒绝未知字段/API key字段、无效版本、超限/不安全端点；非敏感配置才序列化。schema1只迁移已知字段并关闭联网/本机HTTP；schema2启动也关闭联网，原保存密钥不构成授权。写临时文件、sync、同目录替换、保留有效备份；坏主文件尝试有效bak，否则用离线默认且保留坏原件。遗留临时文件不作为已完成配置采用。没有物理断电验收。重置不自动删除密钥，删除需显式操作。

## 剪贴板

仅点击复制才写CF_UNICODETEXT：UTF16/NUL/CRLF，先分配/锁定/填充，再打开真实窗口拥有的剪贴板；成功转交HGLOBAL，否则由guard释放。占用立即反馈，不循环等待；失败不消费结果。没有自动读取系统剪贴板、监听其他应用或自动上屏。

实际GUI点击复制研究gloss已执行，原文保持；编码覆盖中英/换行/特殊字符，实际占用错误测试只持锁，不读取/清空旧剪贴板。**中文特殊字符的真实系统剪贴板粘贴回读未完成**：后续粘贴遇到外部桌面输入而中止，不拿纯编码测试冒充完整回读。

## 实际 GUI 与资源证据

在Windows11 build26300真实交互桌面启动Debug程序，绑定由工具返回的实际窗口，观察布局与UIA控件。已执行：中文输入“你好”→实际123,544条研究词典→`hello; hi`/ResearchOnly；来源是原始gloss，不是正式等价翻译；点击复制后原文仍为“你好”。[当时的真实截图](translation-round12-evidence/research-query-debug.png)。单例观察为23ms（含UI轮询），不能推广为性能分位数或正式准确率。

方向切换首次失败，实际控件被每次render的CB_SETCURSEL复位。保存旧debug EXE身份、GUI树及源码，并用真正Win32控件自动化反例验证，exit101；修复仅在载入设置时同步控件。修复后真实鼠标选择“英文→中文”保持成功。此前日志中“Direction switch invalidates…”是预期动作名称，其实际树未变化，不能当通过，已追加更正保留原文。

自动化一度缺窗口geometry，重新激活/观察后恢复；随后检测外部桌面输入，停止继续操作，没有盲点/盲粘贴。复制回读、多行键入、非敏感设置按钮、GUI密钥按钮、网络同意界面及最终Release全部按钮仍未逐项验收。已向用户提出桌面控制选择，未收到回复时优先完成工程/CI。Computer Use[技能](C:/Users/EDY/.codex/plugins/cache/openai-bundled/computer-use/26.1023.51421/skills/computer-use/SKILL.md)及[安全规则](C:/Users/EDY/.codex/plugins/cache/openai-bundled/computer-use/26.1023.51421/docs/guidance.md)要求“Do not change Windows security settings, Windows privacy settings, or any in-app security or privacy settings.”，故安全设置/密钥界面的自动点击不能作为已完成；真实隔离凭据API及controller路径另已通过，不替代人工按钮验收。

第一测试窗口正常发出Alt+F4后进程37744已不在进程表；未取得退出码，不写成exit0。后一个debug进程43432保留供操作，文件锁导致默认目录重建失败；改用独立构建目录，没有强杀用户正在操作的窗口。一次快照 working set136,990,720B、private118,460,416B、269handles/8threads，仅描述当时debug进程，不是峰值/趋势/泄漏或长期压力结果。

## 自动化、失败与身份

完整被测源码d50e41a；Windows本地12条命令分别保存原始stdout/stderr/返回码，全为0：[full记录](translation-round12-evidence/round12-full.json)。

```text
git diff --check
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
cargo build --workspace --release --locked --offline
python scripts/check_workspace.py
python -m unittest discover -s tests -p "test_*.py" -v
python scripts/audit_translation_dependencies.py --audit-binary ACTUAL_AUDITOR --output REPORT
python scripts/benchmark.py --iterations 100 --output REPORT
python scripts/evaluate_algorithms.py --output-dir REPORT_DIR
cargo build -p cloudleaf-translation-panel --release --locked --offline --message-format=json
```

本地**121 Rust / 22 Python passed，0 ignored**；新增19 Rust（原102保留）。Linux仅运行适用测试，不能把Windows特定测试不编译当成其通过。已覆盖真实本地TCP取消/超时/迟到成功与错误、配置切换/单次授权、离线零请求、快速请求有界拒绝、失焦/原文保留、非合作worker预算/异常线程退出、设置/凭据/原生控件及剪贴板边界。没有默认引擎替换或真实语言质量声明。

保留的实际失败：新关闭/平台API缺失101；EnableWindow feature/import错误、测试语法/闭包解引用错误、Clippy折叠if和参数数量失败；错误断言把已禁用provider期待成OnlineNotAuthorized（原语义实际是ProviderUnavailable，保留零连接断言并纠正预期）；30次快请求后错误假设20次yield队列必空（改为验证有界Busy并在3秒内恢复，预算和失败分母保留）；真实方向复位断言101；正在运行EXE的os error5。另一次PowerShell辅助脚本生成引号解析失败及文档patch匹配失败只在会话转录，不是产品测试结果，改用文件脚本/准确patch后继续。没有删除原测试、禁用lint或放宽网络/资源门槛。

Cargo.lock SHA-256：`1d33e38816d16e3d6653f3ed066b0c5a9ac1f518ce5a116af95ba0adf7e5f57d`。8 workspace crate，外部仍136包/242份许可，只有既有windows-sys0.61.2（MSRV1.71、MIT/Apache-2.0）新增features，无已有依赖版本变化；核心Rust/MSRV1.90。系统字体不复制分发。RustSec/许可原件审计真实通过，数据库和工具SHA保留full报告；这不是全部供应链或法律保证。

数据仍使用R11固定catalog SHA `9edc7d980dcedfdbc400a7246b9806ae880fda7b745ef840c3f3ab5248134b98`，没有新增数据、词频或正式映射。源码、构建工具、Cargo产物和数据身份分别登记。旧64条输入原型基准只检查旧路径，不代表翻译UI或生产语言性能。正式Top1/Top5/MRR仍null。

## CI、模型和剩余门禁

实现d50e41a已普通push，[push CI41096582002](https://github.com/WanFengQC/Cloudleaf/actions/runs/41096582002)执行两平台检查。workflow新增Windows release EXE上传，Linux继续跨平台核心检查。最终文档提交还须核它自己的event=push/head_sha、两job全部step、三个产物head/digest，最终结果放交付凭据与PR正文；此处不提前写CI成功。

离线模型只读审查见[固定来源、许可和接入方案](../translation-offline-model-round12-review.md)。OPUS双方向/M2M100仅候选，权利原件/运行库/性能待核；NLLB非商业及用途边界不适合当前产品随包。没有下载巨型权重、引入推理运行时或伪造推理结果，面板真实显示未安装。

- **done**：真实Win32程序、现有service接入、版本设置/安全恢复、Windows凭据、显式Unicode剪贴板写入、有界任务/配置失效/错误和关闭实现、本地回归及真实窗口部分操作。
- **partial**：GUI完整用户操作矩阵、最终Release视觉/交互、全生命周期资源趋势；模型选择只有来源研究。
- **blocked / not-run**：真实离线模型权重/运行时准入、生产语义人工审读、独立gold/质量指标、真实在线服务兼容、TSF、长期压力和系统应用验收。安全设置按钮由用户人工检查；没有将技术复核当作真人批准。

继续保持五PR Draft、main不合并。下一步在无桌面竞争时完成真实窗口矩阵，再分别推进授权模型和正式语言数据；本面板不等于完整输入法。**CL-010仍不放行。**
