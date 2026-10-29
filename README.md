# 云笺 Cloudleaf

云笺的目标是算法完善、功能完整、响应流畅、翻译可靠、可长期维护的正式跨平台输入法。当前已有可独立测试的 M1/M2 Rust 原型；完整产品按 [v2 路线图](docs/roadmap.md)推进 Windows、核心算法和正式数据三个方向，再集成双语翻译与多平台。

当前边界：默认输入核心尚未接入TSF、原生候选窗口、生产词库或独立搜索/语言模型；纠错/学习和翻译尚未实现。新增独立实验模块的范围见下文CL-008入口。64条手写词典及 CLI 演示不代表最终产品能力。最终50项可追踪能力见[产品范围](docs/product-scope.md)，近期CL-001..006状态见[执行记录](docs/cl001-cl006-progress.md)。

## 环境

- Git。
- [官方 rustup](https://rust-lang.org/tools/install/) 和 Rust **1.90.0**；`rust-toolchain.toml` 锁定工具链，MSRV 同为 1.90.0，edition 2024。
- Windows 使用 `x86_64-pc-windows-msvc`，安装 Visual Studio C++ 构建工具及 Windows SDK；公共核心也可在 Linux/macOS 编译。
- Python **3.11+**（CI 固定 3.12），只用于工程审计与性能报告，输入核心无 Python 运行时依赖。

Windows PowerShell 若尚未识别 Cargo，可在当前终端设置：

```powershell
chcp 65001 > $null
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$env:PYTHONUTF8 = "1"
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
```

## 构建与验证

```text
rustup toolchain install 1.90.0 --profile minimal --component rustfmt --component clippy
cargo fetch --locked
cargo build --workspace --locked --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
python scripts/check_workspace.py
```

工具链首次安装需要下载。M1/M2核心没有外部 Rust 库依赖；后续平台分支通过精确审计政策登记依赖，首次locked fetch后离线构建和测试，核心输入始终离线运行。CI 对 Windows/Linux 执行同一套检查并上传报告；M1/M2检查点 `26b5e45` 已在两个平台真实通过，见[基线与CI记录](docs/reports/cl001-cl002-baseline.md)。原型本地数据见[历史验收报告](docs/reports/m1-m2-validation.md)。

## 运行输入原型

```text
cargo run --locked --offline -p cloudleaf-cli -- --query nihao
cargo run --locked --offline -p cloudleaf-cli -- --replay "nihao <Space> zhongguo <Enter>"
cargo run --locked --offline -p cloudleaf-cli -- --replay "ni <PageDown> 2"
cargo run --locked --offline -p cloudleaf-cli -- --replay "nihao <Left> <Backspace> <Esc>"
cargo run --locked --offline -p cloudleaf-cli -- --help
```

前两个提交示例分别产生“你好”和“中国”；翻页数字示例提交“倪”。无参数进入逐行事件模式，`:quit` 退出。测试宿主在终端显示候选和提交，并立即模拟成功确认；实际 Windows 文本提交尚未实现。英文或未消费按键会显示“交回系统”，命令行自身不会模拟编辑器。

使用 `--dictionary <路径>` 加载版本化 UTF-8 TSV；文件缺失或损坏会明确提示，并使用空词典继续演示原文回退。原型词典仅 **64 条手写数据**，未命中时保留原始拼音；不拼接独立词条生成整句。最多 64 个 ASCII 输入字符，支持 `v` 表示 ü、`'` 显式分段、末音节前缀、光标编辑、每页 5 个候选、最多 50 个候选。完整规则见[实现约定](docs/m1-m2-design.md)及[数据格式](data/README.md)。

## 性能与排序报告

```text
python scripts/benchmark.py --iterations 1000 --output .local/m2-performance.json
cargo test -p cloudleaf-input --test corpus --locked --offline -- --nocapture
python -m unittest discover -s tests -p "test_*.py"
python scripts/evaluate_algorithms.py --output-dir .local/algorithm-v1
```

基准脚本先构建 release，从 Cargo 实际返回的产物路径运行宿主（兼容自定义 target 目录），记录源码及二进制 SHA-256、工具链、硬件、冷加载、事件/查询分位数和进程峰值内存。结果仅描述小型原型，不替代生产词库或UI测量。原有41条[候选回归](docs/reports/m2-candidate-ranking.md)继续保留。

独立算法种子与评估器分别登记来源/划分，比较现有规则与移除精确匹配优先的频率排序消融，保留空结果、超限、OOV和未实现能力的真实分母。报告包含Top-1/Top-5/MRR、逐例位置变化与分组延迟，详见[评估规范](docs/algorithm-evaluation.md)。手写种子不支持正式准确率或最终模型选型结论；DAG/整体候选、N-gram、纠错和学习的比较仍须后续实验。

## CL-008独立搜索实验

新增`cloudleaf-search`提供覆盖原输入区间的词条DAG、穷举参考/精确DP/有界Beam、可解释整数评分、预载Bigram和有限候选池上下文重排。默认输入引擎仍使用原候选实现，新模块尚未接入输入事件。正式语言语料、模型、纠错/简拼/双拼/学习/增量和集成仍未完成；不满足CL-008退出条件，不进入CL-010。

```text
python scripts/evaluate_search.py --iterations 100 --warmup 10 --output .local/cl008-search.json
```

该实验使用[机械资源](tests/search-mechanics-v1/README.md)，明确`language_accuracy_valid=false`；不输出正式输入法准确率。[实施状态与风险](docs/reports/cl008-progress.md)、[逐例对照和性能](docs/reports/cl008-search-mechanics-v1.json)及[真实v2数据只读载入](docs/reports/cl008-unihan-v2-load.json)分别记录证据和限制。原64/41/83条资源及其回归测试继续保留。

真实授权词条另走[预登记资源实验](docs/cl008-real-resource-protocol.md)，不混用机械语言模型，也不生成字典自派生gold：

```text
python scripts/evaluate_real_lexicon.py --evidence <数据审核证据JSON> --iterations 30 --warmup 5 --output .local/real-lexicon-search.json
```

证据JSON要求`schema_version=1`、`dataset_id`、`real_data=true`，`runtime`与`build_report`均含`path/sha256`；每个`sources`项含`id/version/url/license_id/review_status=approved/synthetic=false`，以及`license`和`audit`的`path/sha256`。相对路径以证据文件目录为基准。工具逐记录核对来源/许可和unknown空频率，核对的只是批准材料一致性，来源许可仍需C独立审核。正文候选继承其数据许可，不能按项目MIT重新发布。

输出包括完整词典冷载入、Reference/ExactDag/Beam1/8/64及三类收紧预算、全部探针的候选路径/来源/许可、采集顺序的延迟样本、独立进程峰值及新增/丢失/名次变化。没有独立语料时Top-1/Top-5/MRR为null；未完整Reference或截断整词边不被用作无损/覆盖收益证明。新工具的真实Rust进程自测随Python测试运行，使用另标的临时机械fixture。

资源证据暴露原ExactDag在无历史评分下的冗余状态后，另按[先行证明](docs/cl008-position-dag-design.md)加入显式PositionDag实验，仅支持Structural/Weighted。它按位置压缩状态，Bigram/ContextRerank明确拒绝；旧ExactDag、Reference及默认输入引擎保持原样。资源工具追加同预算的PositionDag四项配置，完整参考/旧Exact可比较时逐路径校验，不据资源探针宣称语言质量或最终最优。

[第四轮前置门禁](docs/cl008-round4-evaluation-gates.md)核验正式评估的来源许可、独立人工gold声明、家族隔离、封存、参数冻结和预测完整分母，不读取封存答案、不计算准确率。当前登记文件明确缺少人工标注、独立保管者及正式语料，命令应返回2并输出拒绝报告；不能填入机制fixture使其显示就绪。

```text
python scripts/check_language_readiness.py --plan config/evaluation/language-readiness-v1.json --output .local/language-readiness-new.json
python scripts/evaluate_restricted_oracle.py --evidence <原有Unihan证据JSON> --output-dir .local/restricted-oracle-new
```

第二个命令按先冻结的读音/稳定ID规则选择最多26条真实记录，以独立Python字符串匹配和穷举对照现有Rust四策略及预算故障配置。它只验证新的受限词条图，不解除完整词库Reference耗尽的限制，不把词典生成内容作为语言gold。两个工具均拒绝覆盖已有输出；原始样本、指纹和失败分母保留。

[第四轮隔离修复补充](docs/reports/cl008-round4-followup/README.md)修复两个获批来源间同一近重复组可以跨split的门禁缺口，保留原版本3项真实失败与修复后的完整检查。近重复组使用全局标识，来源局部段落编号仍按来源限定；既有34,060条测量保留原源码身份，不改绑到此修复。

## 模块

Round 7补充[任务交付与评分观测约定](docs/cl008-round7-design.md)：只打开获批train/dev任务核C的13字段摘要、来源及三协议审核绑定；上下文和全局意图组须由独立人工交付，不打开heldout答案。旧配额、加密封存及参数冻结继续生效，实际材料缺失返回2、语言指标null。错误预测明确丢失原文回退时拒绝。阶段宿主现在使用schema2，新增嵌套scoring（特征/整数/转移及EOS，排除路径克隆/文本拼接）；默认未读钟入口和候选算法不变。旧九阶段报告仍绑定旧实现，不能重新标为本轮评分数据。

Round 8交付[真实人工gold外部执行包](docs/acceptance/cl008-round8/README.md)：列明单一C人员/授权入口、真实双审/仲裁和保管职责、上下文/handoff待填模板、私有明文与公共索引的边界和三个实际门禁命令。人员及授权均待定，模板不是批准包，不产生答案或签名；不重复搜索矩阵，不替换默认核心。

Round 9新增[单人私有作者队列](docs/acceptance/cl008-round9/README.md)：导出未划分任务、核实际授权文件与声明、分开报告个人标注完整和当前摘要下的审核声明。工具不填身份/答案、不接受已划分包、不把不同ID证明为独立真人，正式ready始终false、指标null；旧正式三门禁和独立保管要求不变。

```text
python scripts/check_ime_task_handoff.py --plan config/evaluation/language-readiness-v1.json --preparation config/evaluation/quality-preparation-v1.json --handoff config/evaluation/ime-task-handoff-v1.json --output .local/ime-task-handoff-new.json
```

Round 6在独立研究搜索中维护稳定有序unique候选桶，保留原预算/遍历与完整候选字段；阶段宿主在计时外保存完整Result Debug，以便优化前后同源宿主逐字段对照。设计见[稳定桶约定](docs/cl008-round6-search-design.md)，完整同机旧新矩阵、原始失败及剩余门禁见[Round 6证据](docs/reports/cl008-round6/README.md)。未测单算法allocation/live/RSS，不以宿主峰值代替；默认输入核心与正式质量门禁不变。

Round 5另提供[可选阶段观测](docs/cl008-round5-profiling-design.md)和[真实评估来源家族准备契约](docs/cl008-round5-quality-preparation.md)。观测入口与未读钟旧入口共用算法；逐样本核对结果、预算与图，记录归一化、建图、搜索验证、路径遍历、嵌套桶排序、最终排序及物化。嵌套阶段不可相加/相减推导CPU成本，profile开销不作产品延迟。

```text
python scripts/evaluate_search_phases.py --evidence <R4固定来源证据JSON> --historical docs/reports/cl008-round4/cccedict-full.json --output-dir .local/round5-phases-new
python scripts/check_quality_preparation.py --plan config/evaluation/language-readiness-v1.json --preparation config/evaluation/quality-preparation-v1.json --output .local/quality-preparation-new.json
```

后一命令在实际独立人工来源/lineage/标注审核/封存不足时返回2并保留null语言指标；声明一致不证明人工真实性。新目录拒绝覆盖旧证据，旧Round1–4样本仍绑定原版本。默认输入引擎未替换，CL-008/CL-010不放行。

| crate | 职责 |
|---|---|
| `cloudleaf-types` | 平台无关事件、候选帧、稳定错误和提交确认协议 |
| `cloudleaf-pinyin` | 有界全拼分段、歧义与末音节前缀 |
| `cloudleaf-dictionary` | 严格词典校验、载入时建立内存索引 |
| `cloudleaf-candidate` | 确定性排序、去重与有界候选生成 |
| `cloudleaf-input` | 输入状态、光标、选择、模式和生命周期 |
| `cloudleaf-search` | 独立实验的全输入词条图、同图搜索对照与整数语言模型；尚未接入输入核心 |
| `cloudleaf-cli` | 测试宿主与独立基准入口 |

产品核心不直接写日志，不保存完整输入历史。候选注释不进入提交文本；提交失败保留请求，显式重试需要新 ID；宿主先确认提交，再派发下一事件。日志及错误规范详见[设计约定](docs/m1-m2-design.md)。

代码与项目手写数据采用 MIT 许可证；来源见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。[算法设计](docs/algorithm-design.md)、[平台契约](docs/core-platform-contract.md)及[Windows设计](docs/windows-tsf-design.md)定义后续并行边界；契约模型测试不等于TSF实装。[M1/M2任务记录](docs/m1-m2-tasks.md)保留历史成果。
