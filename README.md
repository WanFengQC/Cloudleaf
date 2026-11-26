# 云笺 Cloudleaf

云笺的目标是算法完善、功能完整、响应流畅、翻译可靠、可长期维护的正式跨平台输入法。当前已有可独立测试的 M1/M2 Rust 原型；完整产品按 [v2 路线图](docs/roadmap.md)推进 Windows、核心算法和正式数据三个方向，再集成双语翻译与多平台。

Round 21 隔离工作区把输入核心、独立[翻译服务](crates/cloudleaf-translation/README.md)、[真实 Marian 离线推理](crates/cloudleaf-offline/README.md)与[Windows 翻译面板](apps/cloudleaf-translation-panel/README.md)纳入同一 workspace。headless 桥接通过现有候选身份和提交事务工作，见[桥接设计](docs/integration-round21-design.md)。正式语言质量与原 Windows 系统验收仍有独立门禁。最终50项能力见[产品范围](docs/product-scope.md)。

输入保留 `codex/input-round14` 的 CL-008 搜索、CL-009 固定词条和 CL-007 COM/候选窗口实现。默认构造沿用原型整词路径，下面示例显式使用固定词典。真实 Windows 应用操作及 TSF 验收的进度与 headless 结果分开记录。

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
cargo install cargo-audit --version 0.22.2 --locked
python scripts/audit_windows_dependencies.py --output .local/input-dependency-audit.json
python scripts/check_offline_vendor.py .
python scripts/audit_translation_dependencies.py --output .local/translation-dependency-audit.json
```

工具链首次安装需要下载。M1/M2核心没有外部 Rust 库依赖；后续平台分支通过精确审计政策登记依赖，首次locked fetch后离线构建和测试，核心输入始终离线运行。CI 对 Windows/Linux 执行同一套检查并上传报告；M1/M2检查点 `26b5e45` 已在两个平台真实通过，见[基线与CI记录](docs/reports/cl001-cl002-baseline.md)。原型本地数据见[历史验收报告](docs/reports/m1-m2-validation.md)。

## 运行输入原型

```text
cargo run --locked --offline -p cloudleaf-cli -- --query nihao
cargo run --locked --offline -p cloudleaf-cli -- --replay "nihao <Space> zhongguo <Enter>"
cargo run --locked --offline -p cloudleaf-cli -- --replay "ni <PageDown> 2"
cargo run --locked --offline -p cloudleaf-cli -- --replay "nihao <Left> <Backspace> <Esc>"
cargo run --locked --offline -p cloudleaf-cli -- --replay "nihao <Space> YuN <Raw:@example.org>"
cargo run --locked --offline -p cloudleaf-cli -- --help
```

前两个提交示例分别产生“你好”和“中国”；翻页数字示例提交“倪”。无参数进入逐行事件模式，`:quit` 退出。测试宿主在终端显示候选和提交，并立即模拟成功确认；实际 Windows 文本提交尚未实现。英文或未消费按键会显示“交回系统”，命令行自身不会模拟编辑器。

`<Raw>` 显式提交当前原始组合。`<Raw:后缀>` 直接追加字面内容，例如把 `YuN` 提交为 `YuN@example.org`。提交保留原大小写，经正常确认，并跳过候选学习。回放标记中的后缀不能包含空格。

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
| `cloudleaf-translation` | 独立后台翻译、词典检索、用户自定义 API 和确认状态机 |
| `cloudleaf-offline` | [固定本地 Marian 模型](crates/cloudleaf-offline/README.md)的 CPU 整句推理、校验及导入管理 |
| `cloudleaf-translation-panel` | [独立 Windows 面板](apps/cloudleaf-translation-panel/README.md)、设置、系统凭据和显式剪贴板复制 |
| `cloudleaf-bridge` | 输入候选、翻译任务与宿主提交事务的无窗口桥接 |

产品核心不直接写日志，不保存完整输入历史。候选注释不进入提交文本；提交失败保留请求，显式重试需要新 ID；宿主先确认提交，再派发下一事件。日志及错误规范详见[设计约定](docs/m1-m2-design.md)。

代码与项目手写数据采用 MIT 许可证；来源见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。[算法设计](docs/algorithm-design.md)、[平台契约](docs/core-platform-contract.md)及[Windows设计](docs/windows-tsf-design.md)定义后续并行边界；契约模型测试不等于TSF实装。[M1/M2任务记录](docs/m1-m2-tasks.md)保留历史成果。

## 显式研发语言评分

在预检的固定词典参数后增加`--language-model <TSV> --language-sha <SHA256>`，CLI才加载独立模型。Lexical以结构分为先、只在结构平局时比较词汇观测；Bigram使用保留上一词状态的ExactDag。模型须与完整词典SHA及stable entry ID对应，当前格式仅接收`mechanical/research_only`训练材料，不能把原始研究计数标成生产频率。默认宿主和输入引擎保持原路径。

离线训练入口为`cargo run -p cloudleaf-input --example train_language --release --locked --offline -- RUNTIME RUNTIME_SHA SEQUENCES METADATA_HEADER NEW_OUTPUT`。序列原件逐行使用stable entry ID、以单个空格分词，元数据记录实际原件SHA、来源、许可、分词协议和train划分。该工具复用既有Bigram训练器、校验原件和词典身份，拒绝覆盖输出；不会生成授权、人工gold或自动启用产物。按键路径只使用预加载只读模型。格式和限制见[Round 15设计](docs/input-round15-design.md)。

Round16另有显式`kind=character_bigram`、`tokenization=unicode-scalar-v1`：同一训练入口逐行读取未经分词的正文，使用BOS/Unicode字符/EOS实际转移的add-one定点对数概率。整条正文同分，不依赖词典切成几个词；不叠加每词结构扣分或研究一元计数。ExactDag按最后模型字符状态计算后缀最优界再有界枚举，PositionDag拒绝该历史评分，Beam仍为近似对照。模型与预载词典实例共同编译，新词典实例须在该实例上重新载入模型。原结构/词汇/词级评分和默认引擎不变。

另有显式`kind=character_bigram_witten_bell`作为平滑对照，按实际不同后继数插值到同语料字符一元分布；没有超参数调优或heldout使用。研发比较未证明其质量优于add-one，两种模式均未获正式质量批准，不能按算法名称或历史四句通过结果默认选用。

`python scripts/prepare_context_corpus.py --source-dir FIXED_TATOEBA_CACHE --output-dir NEW_DIRECTORY`只处理本轮固定SHA的官方原件，保留来源家族、近重复、train/dev/heldout、历史目标排除与逐句署名链接；不下载、不批准生产或生成人工gold。完整原文、模型及owner记录留Git外，内部研发划分不构成独立质量评测。字符Bigram仍可能误选同音字、过度补全前缀或偏向较短正文，详见[Round16设计](docs/input-round16-design.md)。

## Round17覆盖实验

`python scripts/prepare_input_supplement.py --baseline-release FIXED_RELEASE --source-archive FIXED_CEDICT_ZIP --output-dir NEW_DIRECTORY [--model RESEARCH_MODEL]`只从固定批准记录和原件投影基线缺失的繁体单字读音，生成独立`layer.tsv`、逐条来源、许可、修改说明及研发合并快照。原123544条runtime逐字节保留为合并文件前缀，新增frequency仍unknown；该层没有自动获得生产质量批准。可选模型仅改变词典SHA绑定，计数与训练来源不变，必须由真实Rust reader在新词典上重编译缓存，不能把旧模型直接套到新实例。

显式将合并快照及对应新SHA传给既有`--search-lexicon/--search-sha`，才会使用补充层；默认词典不变。`coverage_probe`示例读取实际词典、模型和`case TAB pinyin TAB source_text`，检查完整词条格的正文路径并输出真实Top10/预算。词典导出的拼音可能选择多音字的非常用读法，所有人工确认和正式指标保持未交付；可达性不是准确率。

Windows研发程序的`cloudleaf-ime.exe --interactive-input-host RUNTIME SHA [MODEL MODEL_SHA]`创建普通键盘测试窗口，复用实际Host、双引擎和候选窗口；不注册或激活TSF。请先点击窗口空白区再键入拼音，Space/数字选词、方向键/翻页、Enter确认或未识别原文回退、Escape取消。正文只写入本窗口拥有的文档；Unknown按钮模拟写后未知，停止继续输入，人工核对后才能点击新会话，绝不重放旧事务。这个入口没有粘贴或UTF-16代理对输入支持，不构成系统应用验收。

Round18在同一隔离窗口增加F6循环全拼/全拼简拼混合/小鹤双拼、F7切换n/l双向模糊音、F8或“纠错开关”按钮切换有界全拼纠错。窗口显示当前配置；进入双拼会关闭纠错，双拼下拒绝启用纠错（未实现）。按钮适用于系统拦截F8的环境，修饰键组合不切换设置。仅该交互入口显式使用K10、每页5条以验证多页，原只读review/benchmark仍K5；默认核心、词典及系统设置不变。

平台无关API为`InputEngine::reconfigure_scheme(SchemeConfig)`，需要显式整句搜索后端。事务待确认/失败时拒绝切换，原文和光标保留、revision增加，旧候选不可再提交。七组模糊音逐项/方向配置；拼写纠错仅在整个按键串不存在完整精确音节解释时尝试一处编辑，不能由词典缺词推断拼错。精确、前缀、缩写、模糊及纠错解释成本先作独立全序比较，不叠加到研究语言概率。原始按键不改写，超过解释/交集/搜索预算保留原文回退。默认全部关闭，微软/自然码尚未核实完整原表，未实现。

`--benchmark-input-host RUNTIME SHA MODEL MODEL_SHA EVENTS_TSV`复用同一隔离控制器，分别记录两个引擎、语义比较、候选帧和内存文档事务；无窗口模式明确不测HWND更新。实际键盘窗口另外输出候选窗口更新时间。两者都不是原生TSF或其他应用的端到端延迟。

Round19的同一窗口新增“用户词库 / 本地学习”。管理面板可添加、编辑、启停、删除词条，保存、重载、导入、导出及显式恢复；管理操作在一个有界后台线程执行。基础词典共享只读索引，用户词进入完整候选格；旧事务未收尾时拒绝发布。学习默认关闭，只有该自有文档实际Applied的当前候选才能派发学习；Unknown、失败、取消、原文回退、普通成功事件和benchmark不持久学习。后台繁忙时明确报告此次学习未记录，文档不重试。现有基础频率unknown不变，手工/学习偏好单独用于显式研发后端。

用户数据使用当前用户LocalAppData下`Cloudleaf/user-dictionary`，为有SHA校验的版本化UTF-8明文文件，不是加密存储。最多4096个手工词条，每条1–16汉字、逐字对应合法读音；同词多读音分别管理。只允许一个写实例。损坏/未来格式不能当空库覆盖；需用户明确恢复有效旧代。删除/清空会清理应用管理备份及临时副本，不删除用户自行导出的文件，也不承诺物理擦除SSD。文件错误保留原内存与基础输入。测试时可显式设置`CLOUDLEAF_USER_TEST_PROFILE`为字母数字/连字符标识，使用`Cloudleaf/Research/标识/user-dictionary`独立研发档案；不得将私人词库加入Git或产物。
