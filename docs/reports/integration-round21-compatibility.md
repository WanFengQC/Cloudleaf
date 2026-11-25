# Round 21 工作区与分支兼容性

2026-11-25。整合工作区 `codex/integration-round21` 以 Round 20 输入检查点 `714f8c9439148b23d2ce34dc3a4e672d59598c1d` 为父提交。开始时远程输入提交与本地一致，`main` 为 `a564fd47b0f054ff360592af3ccb8d744021160f`。GitHub 返回的五个 PR 均开启且为 Draft。PR #3 使用实际返回的提交。

| PR | 起始远程 HEAD | 与输入检查点的共同祖先 | 输入独有 / PR 独有提交 |
| --- | --- | --- | ---: |
| #1 | `83a2666fa748b4ae9cba41a5880ba5353e71c9a3` | 同 PR #1 HEAD | 65 / 0 |
| #2 | `59dabdc2f8a3dc87cbd7f19ed89401ee607f7954` | `9af9bc1b2ebacc3ad76124cfa39d09de8eb77bd1` | 86 / 63 |
| #3 | `3f26f34e00c0775124f87e150a021f99fb52c567` | 同 PR #2 共同祖先 | 86 / 70 |
| #4 | `e0ab78c2786d652a3a19cb544b4dc83996e63264` | 同 PR #2 共同祖先 | 86 / 83 |
| #5 | `cca77457ca3943ee5ad90d566a8d9ef6b8795cd2` | 同 PR #1 HEAD | 65 / 27 |

PR #1 已在输入历史内。PR #2、#3、#4 的搜索、词典和 Windows 代码部分早已按路径引入，仍保留各自分叉。与实际 PR #5 HEAD 的 `git merge-tree --write-tree` 预览产生五个冲突，分别是 `.gitattributes`、`.github/workflows/ci.yml`、`Cargo.toml`、`Cargo.lock` 和 `THIRD_PARTY_NOTICES.md`。本轮按路径导入，合并预览退出码 1 与冲突原文留存于兼容清单。

## 导入与依赖

首次导入的 1,187 个新增路径均用 PR #5 的 Git blob 逐个核对。包含 12 个 app 文件、27 个 crate 文件、6 个配置、534 个文档及历史记录、385 个许可文件、4 个脚本、4 个 Python 测试及其素材、3 个转换工具、212 个 vendor 文件。新增内容保留翻译服务、凭据与原生面板，保留 Candle Marian、SentencePiece、本地模型验证/导入/回退及其回归。三个既有 Candle 测试 fixture 来自相同提交，真实 OPUS 权重留在 Git 外。随后只为统一政策与本轮安全修复改写翻译审计登记和现用许可清单。旧许可副本、模型来源锁和历史记录保留原件。

| 跨平台成员 | 应用成员 |
| --- | --- |
| `cloudleaf-types`、`cloudleaf-pinyin`、`cloudleaf-dictionary`、`cloudleaf-candidate`、`cloudleaf-input`、`cloudleaf-search`、`cloudleaf-user` | `cloudleaf-cli`、`cloudleaf-windows-ime`（目录 `apps/windows-ime`） |
| `cloudleaf-translation`、`cloudleaf-offline`、`cloudleaf-bridge` | `cloudleaf-translation-panel` |

统一锁由 Cargo 1.90.0 解析，沿用已有精确版本、来源和 checksum。项目 MSRV 与 Edition 保持 1.90.0 / 2024。共有 13 个工作区成员和 223 个外部依赖条目，其中 214 个来自 registry，9 个来自已审 vendor 路径。以 PR #5 既有锁作解析起点，加入输入侧原有 `windows 0.62.2` 及九个配套包。Windows 输入与翻译面板分别使用 `windows` 和 `windows-sys 0.61.2`，桥接仅接受文本与事务标识。两边继续负责自己的 Win32 类型、COM 生命周期和资源。

首次整合审计使用 RustSec DB `b9b8a3c2cb1f2a3c76ab17a551193568c9f8b7a4`，报告 `url 2.5.8` 命中 [RUSTSEC-2026-0592](https://rustsec.org/advisories/RUSTSEC-2026-0592.html)。在独立目录运行 Cargo 精确更新后，锁中仅 `url 2.5.9` 和其依赖 `form_urlencoded 1.2.3` 与原并集不同。两个官方 .crate 经 Cargo 校验并另核 SHA-256，Rust 最低版本分别为 1.63 / 1.51，均声明 MIT OR Apache-2.0。实际版本、来源、哈希与许可原件登记在 `Cargo.lock`、`config/audits/translation.json` 和 `licenses/translation/manifest.json`。先前失败审计保留。修复后同一数据库下的两组本地严格审计均退出 0，分别核验 34 / 213 个包的许可，绑定锁 SHA-256 `c08b7064ac08f4580ac92e67a903117242d6ed4554636ddd998cca6148655763`。

输入和翻译的许可证分别按 Cargo 元数据闭包验证。整锁审计执行同一严格 RustSec 政策。输入包闭包不包含联网与张量后端，由 Python 回归核对。九个 vendor 补丁及 212 个源文件由 `check_offline_vendor.py .` 核对实际位置、源锁和字节，Windows / Linux CI 均调用。两平台编译完整 workspace 并保留离线工具与无窗口桥接程序。Windows 集成 ZIP 含输入 EXE/DLL、面板及工具，Linux 集成 ZIP 含 CLI 与无窗口工具，两个包均携带各自许可文件和来源锁。产物验收以最终提交的 push CI 与回执为准。

## 集成边界

- PR #2 的独立 gold 评估和资源场景工具、PR #3 的语义发布与独立来源审核、PR #4 的长期准入与所有权审计继续由各自分支提供。输入基线原有的词典读取、用户词库、搜索与 Windows Host 保留。
- 翻译桥接同时保留 task、输入 revision 和 commit 三种身份，采用离线 worker 和显式确认。真实原生翻译面板与输入 Host 尚待同窗人工验收。窗口验收为 `not-run/user-paused`。
- 研究模型仍 `research_only`，正式质量、生产权重分发和物理断网验证各自待验收。TSF 注册、安装、8h/24h 长测保持 `not-run`。CL-010 保持关闭。
