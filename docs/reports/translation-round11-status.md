# Round 11：中英翻译子系统

2026-10-30。本轮交付独立的 Rust 翻译模块、可运行的词典研究检索、用户主动配置的在线 API，以及后台任务和确认状态机。它没有接入默认输入引擎。离线整句推理、正式语义内容、语言质量评测和真实 UI/TSF 仍未交付，不进入 CL-010。

## 版本与工作区

- 新工作区：`C:\Users\EDY\.codex\worktrees\translation-round11\Cloudleaf`，分支 `codex/translation-round11`；[独立 Draft PR #5](https://github.com/WanFengQC/Cloudleaf/pull/5)，base 为共享分支。
- 共享基线：`83a2666fa748b4ae9cba41a5880ba5353e71c9a3`；设计先行提交：`020f15a1281643997cacaa6d52be9b1e4d650299`。
- 初始实现及失败回归：`346ee48a78165f19f557d495e9a824ad8b62965d`；修复后完整被测实现：`292d4981be6209ee1ee7ee97cd2ec3b5d8e8cc54`。
- 原 main：`a564fd47b0f054ff360592af3ccb8d744021160f`。旧 PR #1–#4 的 head、Draft 与工作区保持不变。没有合并、重设基底或强推。
- 新结论分别绑定源码提交、原始数据与派生 catalog、执行工具和实际 Cargo 产物。早期专项运行时的 HEAD 尚为设计提交，另有未提交源文件；对应原始 source SHA 保留，不能当作该设计提交的代码验收。

## 修改范围与行为

| 文件 | 实际交付 |
|---|---|
| `crates/cloudleaf-translation/src/types.rs`、`provider.rs` | 请求/结果/方向/任务/取消/错误、能力、注册及配置；中文/英文/Auto 与词/短语/句子区分 |
| `scheduler.rs` | 固定 worker、有界队列/任务表、总超时含排队、终态不回退、取消和版本隔离缓存 |
| `dictionary.rs` | 预载严格 JSON catalog、来源/许可/索引、原始 gloss 和反查；外部 production policy 与当前映射审核绑定 |
| `online.rs`、`secret.rs` | 用户主动配置的 Chat Completions HTTP 实现、逐请求授权、限额/超时/取消/状态错误、会话密钥存储 |
| `offline.rs` | 离线模型接口和可用性元数据；没有权重或推理 backend，缺模型返回明确错误 |
| `interaction.rs` | 原文保留、当前结果显示/复制/确认 intent、失焦/替换/取消和迟到结果隔离 |
| `examples/dictionary_lookup.rs`、crate README | 独立词典查询命令和 API/生命周期使用约定 |
| `scripts/export_translation_dictionary.py` | 从固定 schema 2 文件包导出研究 catalog、来源/许可/修改说明和可复现报告 |
| Rust 四组集成测试、Python 两组测试 | 实际实现回归；网络只用本地 TCP 服务器，fixture 不作为 gold 或真人授权 |
| workspace/lock、精确依赖政策、许可原件、审计脚本、CI | 独立翻译依赖；核心 production 依赖闭包不含网络或 translation |

实现文件变更见[证据索引](translation-round11-evidence/index.json)；本报告与证据归档另作文档提交。`cloudleaf-input` 和 `cloudleaf-types` 相对共享基线没有修改；没有复制 CL-007/008/009 的方向实现，也没有改变词典 schema 2、正式资源登记或旧测试。

## 已实现与边界

默认离线。创建服务只建立固定后台 worker；`submit/poll` 不做提供者 IO，不等待网络、磁盘或模型。默认 2 worker、8 排队、64 保留任务；配置允许 1–4 worker，队满/任务表满返回 Busy。取消/失效后迟到成功不能重新变为成功。显式后台 `shutdown()` 才等待实际 worker 退出，Drop 发取消并分离线程。非合作插件或系统 DNS 可能延迟实际退出，既有预算继续占用，不制造无限替补线程；资源边界属于单个 service，尚无全进程 service 生命周期管理器。

缓存限制条数和文本 payload 字节，精确输入/方向/kind/provider/config/data 版本参与 key；命中绑定新任务和上下文。没有把 payload 上界表述为分配器总内存峰值，没有持久化输入。默认不缓存在线或整句结果。

在线提供者实际支持中译英、英译中、Auto 源语言及三种任务。用户必须填写完整 endpoint/model，启用配置、允许服务联网，再对当前提供者版本逐请求授权；密钥存在不构成发送同意。配置版本变更两处必须一致。HTTPS 正常证书校验，HTTP 只允许显式字面 loopback，禁止 redirect、retry、系统代理及跨服务回退。401/403、429、断连、超时、无效/拒答/截断/tool-call、大响应均有真实 HTTP 路径测试。错误/Debug 不输出密钥、完整输入、URL 或响应正文。在线输出为 Generated，不能等同于已人工审核的正式语义。

SecretStore 已有接口与零化 owned secret 的会话内存实现；Windows Credential Manager、macOS Keychain 和 Linux Secret Service 尚未实现。真实在线 API、真实密钥、离线模型与翻译质量均未测试。Auto 在线识别委托模型，不宣称已有检测质量证据。

面板取消保留原文；复制与确认只返回意图，不调用剪贴板、TSF 或输入核心。确认只消费一次当前上下文的 Translation 结果；研究释义/反查不可确认为正式翻译。没有实现系统提交，Unknown 提交不得从面板推断已应用或自动重放。

## 实际词典与数据身份

读取 CL-009 现有固定包，不重新下载或改变批准范围：

| 身份 | SHA-256 |
|---|---|
| CC-CEDICT 固定 ZIP | `fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a` |
| 原包 manifest | `5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5` |
| 原 records.jsonl | `168f4579a7d72c87c41a9b2bde0d5f65f1537c65b64e107ea60bbf409f7fd072` |
| 两次研究 catalog | `9edc7d980dcedfdbc400a7246b9806ae880fda7b745ef840c3f3ab5248134b98` |
| 最终实现 dictionary_lookup.exe | `9df76bda94aa19be0e8e39803fe1dfe2e1cdf9c0daedf12660092e410cdafaf1` |
| 最终 Cargo.lock | `116f12b275519228a93170454285ca1e5775723cd313970d8b7c446ccf8b3c67` |

123,544 条记录、198,381 个原始 gloss；catalog 为 32,109,703 字节。两次导出内容及哈希一致，0 条 Translation mapping。真实查询 `你好` → `hello; hi` 为 ResearchOnly/DictionaryGloss；完整 `hello; hi` → `你好` 为 ResearchOnly/ReverseLookup，不拆单词、不生成逆向等价翻译。Auto 明确命中与 Production 拒绝（exit 2/DataNotApproved）均实跑；最终 Cargo 返回的二进制再次重放四条路径，返回码 0/0/0/2 符合预期。

原始 CC BY-SA 4.0 许可/署名/来源锁和修改说明随研究投影保留。批准范围仅原始词头/读音/gloss；没有生产 POS、义项对齐或短语映射。production 需独立外部策略、当前数据摘要和真实审核文件；测试只证明机制一致，不能证明签署者身份、数据权利或密码签名。没有改变 Unihan、Wiktionary、词频 unknown 或现有 CL-009 发布器。

## 原始失败与修复

| 实际失败 | 证据与处理 |
|---|---|
| 新 API/词典能力尚未存在，初始测试编译失败 101 | core/dictionary red 保存；是新增能力缺失，不冒充旧输入核心缺陷 |
| offline 锁定缺 tokio-macros，exit 101 | 明确构建期联网 fetch，再锁定离线验证；没有运行期网络 |
| 首次 RustSec exit 1：4 条 rustls 漏洞、tokio unsound 警告 | 原始 JSON/lock/tool/数据库身份保留；实施前固定 rustls 0.23.46、tokio 1.54.1，未忽略告警 |
| core helper 变量遮蔽编译失败 | 保留旧测试源与日志，修正 helper 命名；未删除断言 |
| 本地 HTTP fixture 10 测 4 fail | Windows accepted socket 可能继承 nonblocking；明确设 blocking，保留首次/单例失败；11 测最终通过 |
| Production policy 没有绑定实际审核文件，9 测 1 fail | 保留旧实现和测试 SHA；补外部审核文件哈希、当前内容绑定、独立审核/仲裁声明验证。真实身份仍未证明 |
| 旧 service 任务 ID 重用，迟到 failure 污染新 epoch，5 测 1 fail | 在精确 `346ee48` 重放 exit 101；`292d498` 改为所有 snapshot 先校验 originating_context；完整回归 5 测通过 |
| 许可副本的上游原始空白触发 diff check，exit 2 | 不修改原件；仅许可原件目录保留 -text 和原始空白，JSON 清单照常查空白。逐字节审计验证全部副本 |
| 归档工具二次运行新增 singleton 身份 JSON 后误按日志 envelope 读取，exit 1 | 原始 KeyError 保留；按已登记的日志 group 校验，不用文件名前缀猜 schema，保留逐项原字节断言 |

另保留早期面板源复制脚本 basename 冲突：首次目录中的 test 副本覆盖同名 source 副本，该目录只有 SHA，不能宣称保存两份原件；精确提交重放改用 src/tests 前缀，两个原件均保存。补充工具身份时一次 PATH 查询和一次临时命令语法错误仅在会话转录中，不是产品回归结果；真实工具路径改为显式解析。

## 完整本地回归

环境：Windows 11 build 26300/AMD64、Rust 1.90.0、Edition 2024、Python 3.12.10。完整被测源码 `292d4981be6209ee1ee7ee97cd2ec3b5d8e8cc54`。下面每条命令独立捕获退出码，全部为 0，详见 [full 原始记录](translation-round11-evidence/round11-full.json)：

```text
git diff --check
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
cargo build --workspace --release --locked --offline
python scripts/check_workspace.py
python -m unittest discover -s tests -p "test_*.py" -v
python scripts/audit_translation_dependencies.py --audit-binary ACTUAL_CARGO_AUDIT --output REPORT
python scripts/benchmark.py --iterations 100 --output REPORT
python scripts/evaluate_algorithms.py --output-dir REPORT_DIR
cargo build -p cloudleaf-translation --example dictionary_lookup --release --locked --offline --message-format=json
```

共 102 Rust 测试通过（保留旧 64，新增 translation 38：tasks 12 / online 11 / dictionary 10 / interaction 5），0 ignored；文档测试通过，22 Python 通过（旧 14 + 新 8）。新增方向依赖的 136 个精确外部包及 242 份许可原件已核验。4 个包归档未附原件，另按 Cargo VCS 固定 commit 获取上游 MIT/Apache 文件及 HTTP 身份，不猜授权。RustSec 0 漏洞/警告；工具 cargo-audit 0.22.2，SHA `f4d0d54145787f959f2ebcac8bd288ef90a86f8e512cad26b49aea321e7e1b53`；数据库 commit `ee1a49a2769a24a596afc1c20cb33748d2769586`。这不是全部供应链或法律审查通过证明。

旧 64 条输入原型的基准和消融均按 README 重跑并完整保留；7,900 event / 1,100 query 样本仅检查旧原型未回归，不是 translation 延迟或中文准确率。词典载入的单次诊断与完整 stdout 保留，未作真实输入线程延迟、逐算法内存或长期资源结论。未运行真实服务、长测、UI/TSF、独立 gold；正式 Top-1/Top-5/MRR 保持 null。

## CI 与证据读取

本轮只推送新独立分支。实现 `292d498` 对应 [push CI 41038663430](https://github.com/WanFengQC/Cloudleaf/actions/runs/41038663430) 已核验成功：event=push、精确 head、Windows/Linux 两个 job 各 20 个步骤全部成功/跳过，两个未过期产物身份匹配。Linux digest `sha256:2da193b948b870462951ad552f7540fba84969d184184f8ba87baff98f88fd8c`，Windows digest `sha256:72474feed226ff97c03baf7ec199aa6c741d816b510862a150489b904627d47d`。元数据原始响应和连接器解码日志见 [CI 证据](translation-round11-evidence/round11-ci.json)，没有用 PR merge-preview 代替。最终报告提交会产生新 head，再检查它自己的 push CI；该最终身份与原始响应保留到本机 `.local/round11-delivery-receipt.json` 和 PR #5 正文，避免把报告自引用写成已被测版本。

[索引](translation-round11-evidence/index.json)登记各组 SHA、源码差异及排除的两个本地大 catalog；组内 utf8 字段经 UTF-8 编码可还原原始日志字节，包括 CRLF，base64 字段用于非文本。所有副本校验长度/SHA，历史报告不覆盖。源代码 SHA 与工具身份、数据 SHA、二进制 SHA 分开登记。完整固定原件及 32 MB 投影留在 `.local`，不随代码重复发布。Windows/Linux CI 证明构建与工程回归，不证明真实应用、人员、数据许可真实性或语言质量。

## 状态与下一步

- **done**：独立模块与接口、有界任务/缓存、取消/超时/错误、在线实际 HTTP 适配及本地服务器验证、原始词典预载检索/反查、面板确认与输入核心不变测试、依赖审计与原始失败归档。
- **partial**：production dictionary 机制与有向映射结构可测试，但真实可发布语义映射为 0；在线协议机制可测试，但没有实际服务兼容性或语言质量证据。
- **blocked**：真人语义审读/授权、独立 gold、授权离线模型、隔离 Windows 环境/实际回滚/执行授权；不编造以解除门禁。
- **not implemented / not run**：模型推理、平台密钥存储、真实 UI/剪贴板/TSF、默认输入引擎翻译集成、真实在线服务验证、长期资源和真实应用验收。

下一步先实现平台 SecretStore 和用户设置/翻译面板宿主，再在单独获批环境验证真实提供者与模型；正式内容须走 CL-009 真实语义审核，语言指标须由 CL-008 独立 gold 支持。独立 PR 依赖共享分支的人工决定，后续集成需重新核基底与完整 CI；本轮无合并授权，不能由技术审核代签。**不放行 CL-010。**
