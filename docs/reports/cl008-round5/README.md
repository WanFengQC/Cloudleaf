# CL-008 Round 5：阶段观测与正式质量准备

2026-10-28。已实现同算法可选阶段观测和来源 lineage 准备核验，并保留真实词库完整矩阵。**正式独立语言评估仍阻塞；默认输入核心未替换，CL-008 未达到退出条件，不进入 CL-010。**

固定起点 `96949ddc32945e2e9dd9694f9ab7e6e1c4b0f65a`；本轮被测实现 `292cbd06614c376dbd1b8cc0a8172e099590e4db`，源码指纹 `b62f6f8bcb6420bbb4a0ee8ba0cab64d413b8b0e8176325c5d075a8ec3ccb1ef`（cloudleaf-source-v2）。本报告及后续发布提交只补证据，不将早期性能改绑到新版本。新精确 push 的双平台 CI 由共享 Round 5 汇总独立记录。

## 设计、修改范围与红绿测试

设计 `b9d4a38d991e421789e64b4815926c5fe981c001` 先登记[阶段观测边界](../../cl008-round5-profiling-design.md)，`e0c9ba3ff8b0684cfa473269e58dd4ff56645579` 再提交 Rust 回归，旧实现真实编译失败（缺 Phase/profile_query）。准备契约设计 `4daaaddffcdbe7fd64bae77ad8a5ba8bf264ae04`、测试 `153df372cf9574e77c7219cb44d3b8caaccb188f` 先于实现，旧脚本缺失真实失败。标注 review 覆盖范围补充设计 `308ad5514ff57236831fbd53f23c0c95123f75fd`、测试 `8fda303f4131bc5050a96c8a03d9ee9080656801` 保留两项拒绝缺失的真实失败；这两项是在当时尚未提交的工具草稿上取得，不将其伪称为某个已提交生产版本的失败。`292cbd0`另修正观测次数统计单位：calls/clock_reads 分位数没有 ns 后缀；修复前的真实断言失败原文同样保留。

原始失败与首轮绿测见 [red](red/profile-red.txt)、[准备缺模块](red/quality-red.txt)、[标注范围失败](red/annotation-scope-red.txt)、[计数单位失败](red/count-units-red.txt)、[首轮阶段绿测](red/profile-green-first.txt)。实际最终命令、返回码、耗时和原日志 SHA 见 [validation](checks/validation.json)。早期未提交草稿的日志只作开发轨迹，最终回归均绑定上述精确实现。

具体文件范围：

- `crates/cloudleaf-search/src/{profiling,lattice,search,lib}.rs`：旧入口保持签名和结果，用 Noop 观察器共用同一算法实现；显式 profile_query 才读钟，阶段包括 normalize、lattice、search_validate、traversal、嵌套桶排序、最终排序、上下文重排、物化和总时。
- `crates/cloudleaf-search/examples/resource_phase_probe.rs`：每例 plain/profile 奇偶反转顺序，计时外逐样本核对完整 Lattice/SearchResult；保存全部样本、错误、预算、剪枝及单独报告构造时间。差异立即输出已完成前缀和左右结果/图诊断，失败 stdout/stderr 在 Python 抛错前落盘。
- `scripts/evaluate_search_phases.py`、`scripts/evaluate_search.py`：新分析器校验全部分母、读钟次数、原数据/探针/协议/完整配置身份，保存原始输出；旧调用兼容。
- `scripts/check_quality_preparation.py`、`config/evaluation/quality-preparation-v1.json`：在原 R4 门禁之外核验共同预处理、全部来源 artifact/lineage、逐源标注 protocol/review 绑定及 review 全来源覆盖。原件换 ID 不能变 lineage，规范 lineage/unit 不跨 split。旧全局 near group 与来源局部 unit 规则保留。
- Rust profiling 与 Python phase/preparation 测试、两份设计、README 和本目录证据。没有修改共享类型、A/C、默认输入核心、旧搜索预算、原始数据或 Round 1–4 报告。

新增 Rust 三项回归覆盖四种搜索、四种评分及正常/预算/错误组合，并要求完整结果一致；Python 新增 13 项覆盖真实 release 宿主四算法、错误输出保全、历史完整配置、阶段空值、非法输入、丢分母、计数单位、跨来源 lineage 与 review 漏覆盖。全部夹具显式是机制数据，不作人工 gold。

## 完整本地检查与明确拒绝

最终 15 项命令中 locked fetch、debug/release、fmt、Clippy `-D warnings`、**77 项 Rust**、doc、资源审计、**65 项 Python**、100/10 机械矩阵、1000 次原型基准、83 条旧诊断与 `git diff --check` 全部通过。正式旧 readiness 与新增 quality-preparation 命令均按实际材料不足返回 **2**，这是保留阻塞的结果，不能写作退出 0 的验收成功。[旧门禁](checks/language-readiness.json)、[准备报告](checks/quality-preparation.json)中 `prerequisites_ready=false`、`language_accuracy_valid=false`，Top-1/Top-5/MRR 为 null。

数据缺少合法固定的独立人工输入法语料、双审/仲裁、全局 lineage/近重复语义审核、冻结参数、封存 heldout 和独立保管者。工具核对文件与声明一致性，不能自动证明人工真实性、许可或语义近重复。C 的 typed sidecar 仍仅有机制夹具；实际词频/POS/义项/翻译/短语和独立 gold 未交付，需将来显式审核转换为本契约，不能自动放行。

## 固定真实词库阶段矩阵

保持 R4 24 条无 gold 探针 ×12 配置，每例 30 个 plain/profile 配对样本、5 次双入口预热，共 **8,640 个配对样本（17,280 次被测查询）**；未将预热记为测量。固定 runtime `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`，123,544 条，全部 unknown，其中 110,045 条多字词。来源为固定合法 CC-CEDICT 镜像而非官方最新或签名版本，源 ZIP `fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`；未新增语料、频率或语言模型。

阶段宿主二进制 SHA `67100ac583e788acd8442cdf089dde98592ee1bf2593ee500d7752c43c72e743`。完整 [summary](phases/summary.json) 绑定源/二进制/数据/探针/协议/历史报告 SHA；12 份配置 JSON、原始 stdout/stderr 与原始采集顺序样本位于同目录。源指纹在采集前后相同。全部 288 组案例的已报告行为字段与固定 R4 结果一致，所有 plain/profile 样本完整图/搜索结果相等；跨算法完整性仍只在参考完整时有意义，Reference 耗尽部分不能证明准确覆盖。

| 配置 | 完整 | 预算 | Beam剪枝 | 拒绝 | plain p95 ms | profile p95 ms | 宿主峰值 MiB |
|---|---:|---:|---:|---:|---:|---:|---:|
| reference | 8 | 14 | 0 | 2 | 163.8716 | 181.7937 | 185.41 |
| exact | 16 | 6 | 0 | 2 | 181.2275 | 198.4441 | 185.50 |
| beam1 | 6 | 0 | 16 | 2 | 1.5257 | 1.5870 | 185.46 |
| beam8 | 6 | 0 | 16 | 2 | 10.5677 | 11.9897 | 185.51 |
| beam64 | 7 | 0 | 15 | 2 | 68.0228 | 78.5217 | 185.34 |
| exact-edges128 | 4 | 18 | 0 | 2 | 1.8658 | 2.0775 | 185.30 |
| exact-expansions500 | 6 | 16 | 0 | 2 | 0.3758 | 0.4199 | 185.51 |
| exact-resident128 | 6 | 16 | 0 | 2 | 0.0951 | 0.0995 | 185.33 |
| position | 22 | 0 | 0 | 2 | 5.8861 | 6.5853 | 185.51 |
| position-edges128 | 4 | 18 | 0 | 2 | 0.3131 | 0.3587 | 185.29 |
| position-expansions500 | 6 | 16 | 0 | 2 | 0.4534 | 0.5096 | 185.45 |
| position-resident128 | 22 | 0 | 0 | 2 | 5.6629 | 6.0594 | 185.51 |

标准配置实际阶段 p95（ms；报告每配置保留 720 样本，阶段分位数只含实际进入的样本，未进入为 null；桶排序是嵌套阶段）：

| 配置 | normalize | lattice | validate | traversal | 桶排序 | 最终排序 | 物化 |
|---|---:|---:|---:|---:|---:|---:|---:|
| reference | 0.0024 | 0.0563 | 0.0002 | 182.5240 | 76.8681 | 0.0007 | 0.0050 |
| exact | 0.0030 | 0.0634 | 0.0003 | 201.8525 | 86.8739 | 0.9286 | 0.0052 |
| position | 0.0019 | 0.0513 | 0.0002 | 6.8862 | 3.5045 | 0.0003 | 0.0038 |
| beam1 | 0.0013 | 0.0375 | 0.0001 | 1.4395 | 0.1820 | 0.1922 | 0.0025 |
| beam8 | 0.0022 | 0.0501 | 0.0001 | 11.2645 | 4.1227 | 1.2502 | 0.0046 |
| beam64 | 0.0026 | 0.0546 | 0.0003 | 85.1385 | 48.1811 | 1.2148 | 0.0051 |

第一张表的总时 p95 为 24 类混合探针全部原始样本的总体分位数，第二张表阶段分位数只含实际进入该阶段的样本；两者分母不同，不能逐项相加比较。每例原始阶段/总时、空阶段、错误、整词截断和候选来源仍可复查；失败及预算没有删除。[失败/预算/剪枝索引](failure-and-budget-index.json)定位全部 175 组相应配置案例及原报告 SHA。本轮无观测改变行为的失败；算法预算失败仍真实存在。

`bucket_sort_dedup` 嵌套在 traversal，query_total 包含观测读钟/记录开销，不能相加阶段或相减伪造独立净 CPU 时间。ContextRerank 在本固定 Structural 矩阵未进入，以 null 保留；它的实际进入由 Rust 机制回归覆盖，不能声称已用正式模型评估。plain 与 profile 配对时间用于识别观测成本，不声称搜索优化、算法最优、产品/TSF 延迟或现代语言准确率。峰值是整个比较宿主，包含预载词库、两份格/结果和累积 JSON，不是单个搜索算法的独立内存成本。桌面环境并非受控实验室。

## 身份、许可、复现和剩余退出条件

[provenance](provenance.json)记录本地原件/发布件双 SHA，文本发布仅 UTF-8/LF 与行尾空白规范化；[raw-logs](raw-logs.json)的每项 text 按 UTF-8 编码可以还原原始日志并核验 SHA。原数据许可和署名保留在 [LICENSE](cccedict-source/LICENSE.txt)、[NOTICE](cccedict-source/NOTICE.txt)；原始候选/正文/报告中的派生词条数据按 CC-BY-SA-4.0 分发，项目 MIT 不覆盖这些数据。工程代码为 MIT。

[历史清单](historical-evidence-preserved.json)逐项核对起点所有已有报告 Git blob；旧 34,060 条样本保留原身份，不覆盖、不改绑。本轮后续发布头只补报告，源码身份仍为被测 `292cbd0`。

```text
cargo fetch --locked
cargo build --workspace --locked --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
python scripts/check_workspace.py
python -m unittest discover -s tests -p "test_*.py"
cargo build --workspace --release --locked --offline
python scripts/evaluate_search_phases.py --evidence <已审核R4证据JSON> --historical docs/reports/cl008-round4/cccedict-full.json --iterations 30 --warmup 5 --output-dir <全新目录>
python scripts/check_quality_preparation.py --plan config/evaluation/language-readiness-v1.json --preparation config/evaluation/quality-preparation-v1.json --output <全新JSON>
```

完整其他命令及实际绝对路径见 validation；发布的来源证据保留原机路径，异机重放须在核验相同 SHA 后显式调整本地文件路径，不能改变内容身份。

- 完成：研究入口阶段观测、同实现差分/失败分母、固定完整真实词库矩阵、质量来源准备核验、完整回归及旧证据保全。
- 部分完成：独立语言质量前置契约与资源对照；契约机制可运行，实际独立材料未齐。
- 未完成/阻塞：正式人工 gold、train/dev/heldout 与来源语义审计、真实现代词频和合法语言模型、正式质量门禁、纠错/模糊音/简拼/双拼/学习/增量、默认核心接入及 Windows 端到端验收。
- 下一步：C 与独立标注方交付可授权材料和显式 lineage/review 映射，冻结开发参数后再由独立保管者运行 heldout；按质量、资源与可靠性证据比较方案。PR #2 保持 Draft，不合 main，不进入 CL-010。
