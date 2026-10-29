# 云笺 Cloudleaf

云笺的目标是算法完善、功能完整、响应流畅、翻译可靠、可长期维护的正式跨平台输入法。当前已有可独立测试的 M1/M2 Rust 原型；完整产品按 [v2 路线图](docs/roadmap.md)推进 Windows、核心算法和正式数据三个方向，再集成双语翻译与多平台。

当前边界：正式 TSF 适配及真实应用验收未完成，生产词库、整句候选/语言模型、纠错/学习和翻译尚未实现。64条手写词典及 CLI 演示不代表最终产品能力。最终50项可追踪能力见[产品范围](docs/product-scope.md)，近期CL-001..006状态见[执行记录](docs/cl001-cl006-progress.md)。

CL-007 独立开发分支已加入 TSF 原生服务、候选 UI、显式管理工具，以及有界原生事件 ownership、独立 watchdog、用户显式恢复与 COM/Win32 故障注入；尚未注册、激活或完成真实应用验收，不能作为可发布输入法使用。同步 barrier、异步事件和仍未关闭的资源/宿主风险见[原生实现约定](docs/cl007-implementation.md)及[可靠性约定](docs/cl007-async-ownership-v1.md)。

Round 5新增提前全局登记的监测worker回收及异常预算持续占用下的健康会话验证，详见[v5设计](docs/cl007-fault-isolation-v5.md)和[Round 5原始证据与阻塞](docs/reports/cl007/round5-isolation-status.md)。未析构Host/外部COM仍保活计费，真实应用和8小时/24小时压力尚未验收。

Round 6补齐流式原始事件/资源、周期检查点和严格身份的新段续跑，新增8h/24h专用压力档；实际只完成短档正常/中断/续跑/失败验证。详见[本轮设计](docs/cl007-round6-isolation-and-soak.md)与[原始证据及准入阻塞](docs/reports/cl007/round6-soak-status.md)。可运行长期档不等于已完成长期验收，系统输入法仍不注册/激活。

Round 7增加隔离环境/执行批准的只读长测预检，以及原始逐键、周期与最终分母一致性核验；无批准材料时8h/24h拒绝启动，短档不受影响。七类生命周期故障、部署恢复边界和待执行清单见[Round 7可靠性约定](docs/cl007-round7-reliability.md)。生产COM、预算和Unknown语义未改变。

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
python scripts/audit_windows_dependencies.py --output .local/windows-dependency-audit.json
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
