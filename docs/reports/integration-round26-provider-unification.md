# Round 26 离线提供者统一

**共用适配器与 Bridge 迁移已实现，Windows Release 无窗口回放通过。** Windows 隔离宿主、独立翻译面板与 Bridge 使用同一 Worker IPC v1。本文记录共用适配器与 Bridge，面板实际入口回放见面板专项。

| 范围 | 变更 |
| --- | --- |
| `cloudleaf-translation` | WorkerProvider 复用现有 TranslationService 后台队列，识别同目录 Worker 与固定 catalog。WorkerObserver 读取阶段、进程和请求身份。服务回收 hook 由最后结束的后台线程执行。 |
| `cloudleaf-translation-ipc` | 握手超时有独立错误。确认退出后才清除 PID；创建管道线程失败时也检查子进程是否回收。 |
| `cloudleaf-bridge` | CLI 保持原参数、JSONL 操作、已有输出字段和内存提交三态。移除 LazyMarian。附加 Worker 身份与现有检查器生成的完整性提示。 |
| `tools/performance/round26_bridge_worker_replay.py` | 重放真正的 Windows Release CLI，加载固定权重，按已核验进程句柄注入故障。保留完整输入、结果及双路日志。 |

普通输入由现有 InputEngine 处理。显式后台任务传递方向、会话、epoch、输入修订和原文。相同进程保留当前方向的热模型。适配器核对 Worker 请求、任务、方向、模型目录及结果版本。诊断快照用上下文以及原文和译文 SHA-256 绑定当前结果。Bridge 校验输入身份后接受显式确认。

适配器在后台区分 catalog 缺失、SHA 不符及进程验证失败。固定 catalog 元数据读取上限为 64 KiB。模型清单、权重核验和张量推理均由工作进程执行。现有离线生成状态为 Generated，固定模型资产用于本地研究。在线能力沿既有显式授权路径。

## Bridge Release 实测

Windows NT 10.0.26300.0 x64，Rust 1.90。源码提交 `c765e634342b19f40fbde2ac4e2043187373a9ef`。Release 构建及真实回放分别用时 137.078 和 16.969 秒，返回码均为 0。下表是单次功能回放的模型计时，同机存在构建和测试，并无性能分位数含义。

| 场景 | 观测 | 模型载入 / 推理 |
| --- | --- | --- |
| 拼音选词中译英 | 固定中文候选及非空英文译文，PID 33028，generation 1，request 1 | 1628.58 / 141.41 ms |
| 再次同向请求 | 同 PID、同 generation、request 2，原始译文一样 | 0 / 161.65 ms |
| 切换英译中 | 固定英文句及非空中文译文，同 PID、同 generation、request 3 | 1001.79 / 199.55 ms |
| 推理中取消后重新请求 | request 4 取消，译文为空。显式 request 5 生成译文，保留完整性复核提示 | 0 / 553.65 ms |
| 强制结束并显式恢复 | generation 1 / request 6 失败。显式请求在 PID 43384、generation 2 / request 1 得到结果 | 814.87 / 348.69 ms |

Applied 后文档只写入一次。NotApplied 后只有显式重试才能获得新提交编号。译文提交未产生选择学习。切换原文后旧结果无法确认。Unknown 不修改文档、拒绝重试，需要宿主开启新会话。一个恢复样例在问候语后增译邀请进入的动作。完整性规则未提示该添加，原始输出保留在日志。

## 复现与身份

- `cargo build -p cloudleaf-bridge -p cloudleaf-translation-worker --release --locked --offline` 返回 0。构建回执和双路日志位于 `.local/round21-runs/round26-bridge-release/`。
- `cargo test -p cloudleaf-translation -p cloudleaf-translation-ipc -p cloudleaf-bridge -p cloudleaf-translation-worker --all-targets --locked --offline` 返回 0，75 通过、1 个要求权重环境变量的 Worker 测试保持 ignored。真实权重在上表的实际 Bridge CLI 内执行。配套 `cargo clippy` 使用相同包与目标、锁定离线，附加 `-- -D warnings`，返回 0。目录为 `.local/round21-runs/round26-adapter-exact-tests/` 和 `.local/round21-runs/round26-adapter-exact-clippy/`。
- `python -X utf8 tools/performance/round26_bridge_worker_replay.py --bridge .local/target/release/cloudleaf-bridge-headless.exe --catalog CATALOG --catalog-sha CATALOG_SHA --lexicon crates/cloudleaf-bridge/tests/fixtures/lexicon.tsv --out .local/round26-bridge-worker-real` 返回 0。实参和源码逐文件 SHA 见 `.local/round21-runs/round26-bridge-worker-real/result.json`。五组实际 CLI 的 stdin、stdout、stderr 和退出码均位于 `.local/round26-bridge-worker-real/`。报告 SHA-256 为 `3c33a67ae4ef12b8cee821c88022a21f62f074b467e0402cd96176587c1a711b`。
- Bridge EXE 1,606,144 字节，SHA-256 `4f9f0872cd5548d155f44f781f81429e6f3f1c274897d066657e9f5b0a65fa0c`。同次 Worker EXE 3,315,200 字节，SHA-256 `1c3297561d100d8353c63bd35b7a8bf26ff7f7261ac290b3ddaf6b3a0fec7e94`。
- catalog SHA-256 `f88a5f57513a0aaae3bbd659950adf042c8c8dde07736084cb951f61f22c7f0f`。中译英模型为 `opus-mt-zh-en@cf109095479db38d6df799875e34039d4938aaa6`，权重 SHA-256 `a7c75c919608ea1af3bb6d4aca052af43f35e6967a4f133a865bfe2bde13f21e`。英译中模型为 `opus-mt-en-zh@408d9bc410a388e1d9aef112a2daba955b945255`，权重 SHA-256 `e5f4423b70cb0f5a40d38d1a2733a77cec2aa23cc0bae647985fe17f8f5fe616`。资源身份来源为 `.local/round21-model-identity/summary.json`。
- fixture SHA-256 `50b020cbe8813bf783a1ac66731e93771439ddff9a6b9a369dde58332c704041`。重放工具 SHA-256 `3d061bcc978b485509042e724e4b54977344e1e5dc408d0f7c3077cd00c19f1e`。

桌面、真实 TSF 和外部语言质量验收未执行。本回放使用内存提交和本地测试原文。依赖闭包、面板 63 例复现与最终提交 CI 由各专项或轮次汇总记录。

## 独立面板与隔离宿主 Release 复验

独立面板使用实际 `SetupWorker`、`PanelController`、schema 3 设置和固定术语表，从标准设置加载两个固定 Marian 模型。63 条研发回放在比较原始 UTF-8 译文、完整性发现、风险复核状态和模型身份时均为 63/63 一致。覆盖 32 条中译英和 31 条英译中、61 次使用已驻留的同方向模型、一次换方向重载和 46 条风险复核。唯一送译进程在双向任务间保持 PID，新增取消场景保留原文、拒绝复制过期译文。区分缺失目录和损坏目录，两个实际启动的子进程均独立证实退出。无窗口测试还验证复制准备行为和离线路径不读取在线凭据。

面板回放用 `python -X utf8 apps/cloudleaf-translation-panel/examples/replay_round26.py .local/target/release/examples/worker_replay.exe .local/target/release/cloudleaf-translation-worker.exe .local/round26-panel-replay`，返回 0。Probe SHA-256 `b48bf9d8e08f3712b89952cfb63f4ae713b2c04c69e901e8896dad03a0370635`，同次 Worker `01f38242f402d1d34e9fa68c73e010c5e837b0dcf1b45932769060bf1e764840`。双方固定资源身份与 Bridge 相同。面板 Python 首次将独立对照行误纳入三份原始译文索引；失败保留于 `.local/round26-panel-preflight-red`。修正后逐份匹配测试输入身份。产物和原始日志在 `.local/round26-panel-replay`，安全摘要与输入输出 SHA 可公开。

Windows 隔离宿主使用真实 `LocalHost` 和产品翻译 `Controller`：以 Release 双向启动实际 Worker，核对准确的 `model_id@revision`、任务结果版本、可显示预览和未确认时的空文档。取消后确认预览失效，正常中文按键仍进入组合，关闭后确认 Worker 回收。首轮结果断言误以为模型身份是 64 字符摘要；按实际协议修正测试，Release 重跑返回 0。测试二进制 SHA-256 `576dd307b7c2a88bff22835dee7d4120824174d1b656cac3fec7cb4fa1619453`。命令和失败/通过原文分别位于 `.local/round21-runs/round26-isolated-host-real` 和 `.local/round21-runs/round26-isolated-host-real-fixed`。该入口仍直接使用同一个 WorkerClient 协议，确认事务与输入感知保持在宿主。HUD、系统剪贴板和真实窗口均未操作。

独立重放 22 条固定规则对照后，原始输出、结构化发现和风险复核三项保持 22/22 一致。12 条预置错误提示 10 条，10 条等价表达提示 1 条。本测对原有输出应用同一检查规则，不调用模型。受测提交 `bb58b28`，安全摘要在 `.local/round26-panel-controls-fixed/public-safe.json`，原始日志和首次链接失败分别保留。
