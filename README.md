# 云笺 Cloudleaf

自研、离线优先的中文拼音输入核心。当前完成 M1 工程初始化与 M2 平台无关原型，可在命令行验证输入流程；Windows TSF、候选窗口与安装包从 M3 开始。

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
cargo build --workspace --locked --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
python scripts/check_workspace.py
```

工具链首次安装需要下载。workspace 没有外部 Rust 库依赖，后续 Cargo 构建、测试和核心输入都可离线运行。CI 对 Windows/Linux 执行同一套检查并上传基准 JSON；当前远程 CI 尚未运行，实际本地结果见[验收报告](docs/reports/m1-m2-validation.md)。

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
```

基准脚本先构建 release，再单独启动宿主，记录源码及二进制 SHA-256、工具链、硬件、冷加载、事件/查询的 P50/P95/P99/max 和进程峰值内存。结果只描述小型原型词典与当前机器，不能替代生产词库或 UI 测量。[本地验收报告](docs/reports/m1-m2-validation.md)附有实际结果及 41 条语料的[候选排序报告](docs/reports/m2-candidate-ranking.md)。

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

项目代码与手写原型数据采用 MIT 许可证；依赖及数据来源见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。[路线图](docs/roadmap.md)定义后续阶段，[任务记录](docs/m1-m2-tasks.md)逐项登记本次交付与验收。
