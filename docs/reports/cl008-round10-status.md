# CL-008 Round 10：计数资源门禁与真实缺陷修复

2026-10-30（Asia/Shanghai）。本方向为 **partial**：计数合同及观测工具已实现、回归通过；正式字节内存、延迟、取消和独立语言质量退出门禁未通过，不替换默认输入引擎。

## 版本、设计和修改

起点 `2751b1512394e09c92bca2b4329456c3cd3c30de`；先行设计/反例提交 `02fb8eceb541e842235e7daa90725398da39634a`，设计见 [资源合同](../cl008-round10-resource-contract.md)。初实现 `b94bf4dedc41cd2895785f08d80d0ae54e68d015`；第二次独立代码审阅发现的反例提交 `f4df5a0`，最终受测实现 `f478c66c52eac7e43afe85bc7e5cfe96e2e51254`。本报告后续归档提交只新增文档/证据，最终推送SHA和该精确SHA的CI由共享Round 10汇总记录，不能用实现提交或旧CI冒充最终HEAD。

实际修改：`scripts/search_resource_contract.py` 提供单一观测入口合同；`scripts/evaluate_real_lexicon.py` 调用该入口校验案例身份及完整性矛盾；`scripts/check_search_resources.py` 校验新版观测/配对配置并保留原始失败；`crates/cloudleaf-search/examples/resource_probe.rs` 输出v2完整配置及原始输入回退；`crates/cloudleaf-search/tests/resource_contract.rs` 直接检验真实Reference部分完整路径、三类DAG/Beam预算终止及边覆盖；`tests/test_search_resources.py` 增加四项入口回归；已有`test_real_lexicon_evaluation.py` 增加旧实现会失败的反例。核心搜索`src`、共享接口、默认引擎、数据、词频及原有24探针均未改变。

## 故障与修复证据

- **P1 已修复**：旧compare只核案例ID，预算耗尽却`guaranteed=true`仍可成为完整oracle；输入/来源家族错配也可配对。`baseline-red/result.json` 和stderr保存旧实现实际code=1，之后不提高预算，直接拒绝观测矛盾。
- **P1 已修复**：初版严格门禁容许无预算/剪枝却`guaranteed=false`，导致完整oracle被跳过；合法探针全部错误拒绝也可通过。`completeness-red` 保存先行测试code=1及原文；修复只在新版完整配置入口要求完整性与真实公式一致，并按固定探针`kind`校验合法/InvalidInput。旧fixture兼容入口不虚构缺失配置。
- Reference在搜索预算耗尽后可保留已完成的完整路径子集，ExactDag/PositionDag/Beam丢弃部分路径；图边截断可搜索已有边，所有预算/剪枝均不保证完整。新增Rust测试逐边验证首尾覆盖，未复制算法。
- 准备阶段曾有内联命令引号语法错误及主机PATH未包含cargo导致的启动错误，未改生产代码；细节在`preparation-limitations.json`，明确只是终端输出描述而非原始子进程捕获。后续runner显式解析可执行文件，原始捕获不覆盖。

## 实际验证和结果

[完整证据目录](cl008-round10/checksums.json)包括最终11条命令、每条返回码/耗时/stdout/stderr，完整捕获见`raw-artifacts.json`，runner源码见`runners.json`。所有最终命令独立返回0：Rust 1.90.0、git diff/status、fmt、Clippy `--workspace --all-targets --locked --offline -- -D warnings`、Rust all-targets **83通过**、doc-test、release、`scripts/check_workspace.py`资源/依赖许可证审计、Python **89通过**、资源门禁。最初实现的完整回归/矩阵也单独保留；第二次修复后重跑同一套检查。没有删除旧测试，**466份历史报告保持字节不变**。

固定真实runtime **123,544条**，其中多字词110,045条，全部频率unknown；没有新增或补填词频。四方法采用同一24输入、Structural、k=5、context_pool=20、Beam宽度8以及每组相同预算。标准/边128/扩展500/驻留128四组，其余预算保持16384/200000/50000。每案例预热1、计时5、另执行1次保留结果。最终**384案例、1920计时样本、384预热及384结果调用**；计数合同通过，无可用完整oracle下的候选等价失败。所有候选丢失、排序变化、预算、剪枝、拒绝和时延原始样本均在`final-matrix`，不从失败分母去掉。

| 预算组 | Reference触预算/24 | Exact触预算/24 | Position触预算/24 | Beam触预算/24 | Beam剪枝/24 |
|---|---:|---:|---:|---:|---:|
| 标准 | 14 | 6 | 0 | 0 | 16 |
| 边128 | 18 | 18 | 18 | 18 | 16 |
| 扩展500 | 16 | 16 | 16 | 16 | 16 |
| 驻留128 | 16 | 16 | 0 | 16 | 9 |

每方法每组2个非法输入实际拒绝，合计32/384；预算分母包含这2例，并保留全部合法案例。标准组各方法最大案例P95分别 **99.1333 / 95.7221 / 4.5608 / 9.9357 ms**。每例仅5次的小样本P95/P99接近最大值，主机背景负载未控制，不能宣称算法最优或产品延迟达标。标准Reference/Exact的慢例及受限结果是**剩余实际资源问题**，未调高预算隐藏；计数通过不代表正式延迟通过。`resident_high_water`是活路径计数，未测单算法字节分配，不以整体进程峰值推断内存。

## SHA与重放

完整源码、工具、原始来源、CLI和probe身份在 [identity.json](cl008-round10/identity.json)；源码指纹为`cloudleaf-source-v2`，数据/工具独立列出，不将报告自包含为自身实现身份。

- 源码SHA-256：`3e6ceb8366a2e76c2075e31e131c60ce7628823ca14ccf8c0f4c3a7fc16d5d48`
- runtime：`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`
- 原件ZIP：`fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`
- probe二进制：`2d143a4f642079c96fcae6a3750528a419b19c8967e492883ab0dee49eb3abff`
- 门禁工具：`d655a09168f547649b2ff7b9ac2b0fa3d3c498f8ab8227d7af4ad9fb24fa560c`

按证据中实际命令执行`python -X utf8 scripts/check_search_resources.py --evidence .local/real-lexicon/cccedict-round4-evidence.json --output <新的空输出目录>`；需先有相同固定原件/许可证据，不重新下载其他版本。原件本地路径和每次进程命令均在原始报告。Windows/Linux不同编译产物另由各CI记录，不能假定本地EXE哈希与Linux一致。

## 剩余退出条件

**done**：实际评估工具错误修复、新版观测合同、同输入/预算小矩阵、原始失败保全、Rust/Python完整本地回归。

**partial**：确定性路径/工作计数合同通过；正式资源保证尚缺冻结延迟退出阈值、具有归属的字节内存/分配峰值及取消契约。未新增取消API，预算中止不冒充用户取消；输入Err的原文由宿主保留，搜索错误对象不提供回退字段。

**blocked**：独立真实人工gold、真实双审/仲裁/heldout人员与授权仍未交付；Top-1/Top-5/MRR继续null，未用探针或词典生成答案，不改变train/dev/heldout门禁。语义及词频准入依赖CL-009的真实数据，而非数量。

**not-run / not-implemented**：真实输入引擎集成、正式语言准确率、单算法字节内存评测、用户取消、真实TSF/8h/24h均未执行或未实现。新最终HEAD的push CI须待精确核验；历史成功不替代。保持Draft、main不合并、CL-010不放行。
