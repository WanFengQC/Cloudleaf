# Round 26 兼容与配置准备

| 入口 | 现有生效配置 | 当前约束 |
| --- | --- | --- |
| Windows 隔离宿主 | 命令行 catalog 路径与 SHA、1/2/4 或自动线程档、normal/below-normal、可选 TSV 术语；当前输入方案和词库由输入配置处理 | 绝对 CPU 掩码只用于诊断，未持久化。真实窗口继续暂停 |
| 独立面板 | schema 3；保留 catalog 路径，既有路径配置可在后台解析内容 SHA；可选离线术语表；用户显式配置和授权在线 API | Worker 固定默认调度及单线程。原有 Windows 凭据存放与 JSON 兼容 |
| Bridge CLI | 既有固定 lexicon、catalog 路径及 SHA 参数；旧 JSONL 字段语义保留，`translation-evidence` 可追加 Worker 身份和进度 | 首次请求惰性启动本地 Worker。当前输出无人工确认即不提交 |
| 统一设置中心 | 独立配置入口和锁文件已具备 | 尚未实现。资源版本、线程策略、离线优先和输入方案等尚无统一产品界面 |

Release 入口各自持有并监督一个 Worker，工作进程身份和目录锁独立。隔离输入 DLL 的锁定普通依赖闭包不含 `cloudleaf-offline`、Candle 或 reqwest。面板和 Bridge 无 `cloudleaf-offline` 或 Candle，`cloudleaf-translation` 的 reqwest 仍随公共翻译 crate 编译。模型计算保留到 Worker，离线路径依照本地 IPC。四条 `cargo tree -p <crate> -e normal --locked --offline` 均返回 0，命令与原件在 `.local/round21-runs/round26-closure-*`。Cargo.lock 只挪动工作区 crate 依赖边，第三方依赖版本未调整。

显式原文提交仍由原事务处理。大小写、数字、标点、代码、邮件和 URL 继续遵循当前修订与应用确认，Applied、NotApplied、Unknown 及学习保护原有语义。自动中英混排仍待实现。翻译模型继续仅本地研发可用。M2M-100 与 MADLAD-400 权重准入仍受限，NLLB 延续旧排除结论。正式 gold 和词频权限未落实。

数据仍用 Round 25 固定目录与来源锁。真实外部工具、TSF、长测及生产分发分别等待用户授权和准入证据。
