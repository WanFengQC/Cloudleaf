# Cloudleaf Windows 翻译面板

独立 Windows 桌面程序，复用 `cloudleaf-translation`。不安装或注册输入法，不向其他应用自动输入，不监听剪贴板或其他应用文字。

```text
cargo build -p cloudleaf-translation-panel -p cloudleaf-translation-worker --release --locked --offline
target\release\cloudleaf-translation-panel.exe
```

首次构建须按根 README 执行 locked fetch。界面采用 Win32 标准控件，支持多行 Unicode、粘贴、清空、方向/模式选择、开始/重译/取消、研究结果显示及显式复制。

本地研究词典需先按 translation crate 的说明从已固定数据包导出 catalog，随后在设置填写路径并应用，或显式启动：

```text
cloudleaf-translation-panel.exe --dictionary PATH_TO_CATALOG_JSON --settings-dir PATH_TO_SETTINGS_DIRECTORY
```

启动不会下载词典或模型。研究词典只做词头/gloss查询与完整gloss反查，标记 ResearchOnly，不提供整句或虚构等价翻译。离线模型先按[模型工具说明](../../crates/cloudleaf-offline/README.md)从已授权固定原件转换、校验和导入；在面板选择离线提供者，填写导入后的catalog并应用。第一次离线翻译请求启动和面板同目录的 `cloudleaf-translation-worker.exe`，由其校验并加载模型。中英双向单词、短语及整句任务返回 Generated 结果。设置未配置、工作进程缺失及模型损坏分别报告原因。没有自动语言检测、量化或GPU后端。

默认设置目录为 `%LOCALAPPDATA%\Cloudleaf\TranslationPanel`。schema3保存非敏感配置并迁移schema1/2；重启保持离线。离线catalog、在线模型名称和词典路径分别保存；提供者、模型、端点、超时和缓存修改后点击应用。开始翻译后，状态栏根据后台进度显示工作进程启动、验证、加载、推理及错误。原文或方向变化、新请求、取消、失焦和配置应用使旧任务失效。设置 JSON 可选 `offline_glossary`，填写本地 UTF-8 TSV 路径以检查显式术语。格式及大小限制与隔离宿主一致。风险提示保留原始译文，仍可按原有方式显式复制。

在线 API 没有默认端点或模型。填写完整 HTTPS Chat Completions 地址及模型；密钥单独保存到 Windows 当前用户 Generic Credential，按端点隔离。密钥不写 JSON 或日志。显式启用联网并应用后，还需逐次勾选发送授权再点击开始；授权在构造该次请求时消费，密钥存在不代表允许联网。在线连接超时范围1–60秒，总超时范围1–300秒，连接不得大于总时限。本机 HTTP 需另行允许，仅限字面 loopback；不允许 redirect、系统代理或自动重试。

复制只在点击时写 CF_UNICODETEXT；失败不丢译文。密码框只掩码显示，保存操作后清空自有输入，不承诺Windows内部副本已全部擦除。重置设置不会自动删除已存凭据，需要针对相应端点显式删除。

关闭先取消任务并等待后台退出。词典和在线模式最多等待 2 秒，离线模式最多等待 4 秒。更换设置前要求离线工作进程回收确认；若无法证明旧进程已退出，面板显示错误并保留失败状态。

非 Windows binary 明确返回不支持，跨平台 service/settings/controller/offline仍参与 Linux 测试；没有明文凭据回退。离线推理是本地Candle CPU，Python只用于离线转换；权重不随应用分发。模型生产分发、正式语义内容、独立语言质量和 TSF 集成尚未验收；真实断网需另有环境证据，不从本地推理或CI推断。
