# 云笺 Cloudleaf

云笺的目标是算法完善、功能完整、响应流畅、翻译可靠、可长期维护的正式跨平台输入法。当前已有可独立测试的 M1/M2 Rust 原型；完整产品按 [v2 路线图](docs/roadmap.md)推进 Windows、核心算法和正式数据三个方向，再集成双语翻译与多平台。

当前边界：TSF、原生候选窗口、生产词库、整句候选/语言模型、纠错/学习和翻译尚未实现。64条手写词典及 CLI 演示不代表最终产品能力。最终50项可追踪能力见[产品范围](docs/product-scope.md)，近期CL-001..006状态见[执行记录](docs/cl001-cl006-progress.md)。

上段保留main历史基线。当前`codex/input-round14`为独立中文输入集成试验：复用CL-008搜索、CL-009固定已审词条和CL-007真实COM/候选窗口；默认构造仍为原型整词路径，试验须显式启用。已有代码不等于真实TSF应用或正式语言质量验收，main和原五PR不合并。

```text
python scripts/verify_input_lexicon.py --release FIXED_RELEASE --output NEW_RECEIPT.json
cargo run -p cloudleaf-cli --release --locked --offline -- --search-lexicon FIXED_RELEASE/runtime.tsv --search-sha VERIFIED_RUNTIME_SHA --query nihao
cargo run -p cloudleaf-windows-ime --bin cloudleaf-ime --release --locked --offline -- --review-input-host FIXED_RELEASE/runtime.tsv VERIFIED_RUNTIME_SHA
```

完整固定包在Git外，不下载新数据；凭据不是签名。第二项是真实核心候选，第三项只核Windows Host的执行/投影与恢复预算，不注册或激活TSF，也不向真实应用提交文字。原始候选有明显排序错误，词频unknown保持None，正式指标仍null。设计与完整限制见[Round14设计](docs/input-round14-design.md)。

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

## 模块

| crate | 职责 |
|---|---|
| `cloudleaf-types` | 平台无关事件、候选帧、稳定错误和提交确认协议 |
| `cloudleaf-pinyin` | 有界全拼分段、歧义与末音节前缀 |
| `cloudleaf-dictionary` | 严格词典校验、载入时建立内存索引 |
| `cloudleaf-candidate` | 确定性排序、去重与有界候选生成 |
| `cloudleaf-input` | 输入状态、光标、选择、模式和生命周期 |
| `cloudleaf-cli` | 测试宿主与独立基准入口 |

产品核心不直接写日志，不保存完整输入历史。候选注释不进入提交文本；提交失败保留请求，显式重试需要新 ID；宿主先确认提交，再派发下一事件。日志及错误规范详见[设计约定](docs/m1-m2-design.md)。

代码与项目手写数据采用 MIT 许可证；来源见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。[算法设计](docs/algorithm-design.md)、[平台契约](docs/core-platform-contract.md)及[Windows设计](docs/windows-tsf-design.md)定义后续并行边界；契约模型测试不等于TSF实装。[M1/M2任务记录](docs/m1-m2-tasks.md)保留历史成果。

## 显式研发语言评分

在预检的固定词典参数后增加`--language-model <TSV> --language-sha <SHA256>`，CLI才加载独立模型。Lexical以结构分为先、只在结构平局时比较词汇观测；Bigram使用保留上一词状态的ExactDag。模型须与完整词典SHA及stable entry ID对应，当前格式仅接收`mechanical/research_only`训练材料，不能把原始研究计数标成生产频率。默认宿主和输入引擎保持原路径。

离线训练入口为`cargo run -p cloudleaf-input --example train_language --release --locked --offline -- RUNTIME RUNTIME_SHA SEQUENCES METADATA_HEADER NEW_OUTPUT`。序列原件逐行使用stable entry ID、以单个空格分词，元数据记录实际原件SHA、来源、许可、分词协议和train划分。该工具复用既有Bigram训练器、校验原件和词典身份，拒绝覆盖输出；不会生成授权、人工gold或自动启用产物。按键路径只使用预加载只读模型。格式和限制见[Round 15设计](docs/input-round15-design.md)。

Round16另有显式`kind=character_bigram`、`tokenization=unicode-scalar-v1`：同一训练入口逐行读取未经分词的正文，使用BOS/Unicode字符/EOS实际转移的add-one定点对数概率。整条正文同分，不依赖词典切成几个词；不叠加每词结构扣分或研究一元计数。ExactDag按最后模型字符状态计算后缀最优界再有界枚举，PositionDag拒绝该历史评分，Beam仍为近似对照。模型与预载词典实例共同编译，新词典实例须在该实例上重新载入模型。原结构/词汇/词级评分和默认引擎不变。

`python scripts/prepare_context_corpus.py --source-dir FIXED_TATOEBA_CACHE --output-dir NEW_DIRECTORY`只处理本轮固定SHA的官方原件，保留来源家族、近重复、train/dev/heldout、历史目标排除与逐句署名链接；不下载、不批准生产或生成人工gold。完整原文、模型及owner记录留Git外，内部研发划分不构成独立质量评测。字符Bigram仍可能误选同音字、过度补全前缀或偏向较短正文，详见[Round16设计](docs/input-round16-design.md)。
