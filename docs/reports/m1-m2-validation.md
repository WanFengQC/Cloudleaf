# M1/M2 本地验收报告

验收日期：2026-10-22（Asia/Shanghai）。结论：M1 工程基线及 M2 平台无关原型的本地退出条件满足。远程 CI 已配置但尚未运行，Windows 系统输入法与真实应用验收属于 M3，尚未实现。

## 工程与行为

- 独立 Git 仓库，初始分支 main；未配置远程，未创建提交。源码版本通过下述 SHA-256 追溯。
- 六个 Rust crate，固定 Rust/MSRV 1.90.0，edition 2024，提交用 Cargo.lock 已生成；无外部 Rust 库依赖。
- `cargo fmt --all --check`、Clippy `-D warnings`、release 构建、资源/许可证/依赖校验通过。
- `cargo test --workspace --all-targets --locked --offline`：44 个单元测试 + 1 个覆盖 41 条固定语料的集成测试，全部通过；doc-test 检查通过（当前无可执行文档示例）。
- CLI 实测 `nihao <Space> zhongguo <Enter>` 只提交“你好”“中国”，`ni <PageDown> 2` 提交“倪”；缺失词典时明确提示，并以 `Zzzq <Enter>` 提交原始 `Zzzq`。
- 覆盖退格、取消、光标、分页、模式、失焦、异常字符、长输入、失败保留、显式重试、重复/迟到确认与注释隔离；实时路径没有网络、模型或文件读取。

## 实测环境与方法

| 项目 | 实测值 |
|---|---|
| 系统 | Windows-11-10.0.26300-SP0，AMD64 |
| CPU | 11th Gen Intel(R) Core(TM) i5-11400 @ 2.60GHz，12 逻辑线程 |
| 物理内存 | 31.77 GiB |
| 工具链 | rustc 1.90.0 (1159e78c4 2025-09-14) |
| 构建 | release，thin LTO，codegen-units=1 |
| 数据 | prototype-v1，64 条，3435 UTF-8 字节 |
| 候选配置 | 输入帧最多 50 条，每页5条 |
| 源码 SHA-256 | `81a42d0a6c89eee5350b73f3dfc4a22fdf8cc677f66336570bc4116857a70d05` |
| 二进制 SHA-256 | `f5a620c3b6378be4469fc61fd6cce57637b544db8d2f19615a89f7e450aa9ee8` |
| 测量 UTC | 2026-10-22T02:04:50.583538+00:00 |

命令：`python scripts/benchmark.py --iterations 1000 --output docs/reports/m2-performance.json`。脚本先执行固定工具链的离线 release 构建，启动独立进程测量并记录源码/二进制指纹，构建或测量中源码变化会拒绝报告。原始数据见 [m2-performance.json](m2-performance.json)。

冷测量只运行一次，范围是词典载入、引擎初始化到完整输入 nihao，耗时 **0.2895 ms**；它不是进程启动到 UI 首帧或第一次候选的耗时。

稳定阶段预热20轮，正式1000轮。固定序列为 nihao、zhongguo、xian、xi'an、zhongwen、shijie、cankao、nv、lve、zhongg、zzzz，各序列逐字输入后退格、取消。核心事件计时包含解析、索引/排序和返回帧，排除打印、UI及系统提交。查询单独计时，在计时外解析，不计候选排序。每轮79个事件、11个完整查询，分位数按排序样本的最近秩计算。

| 指标 | 样本数 | P50 ms | P95 ms | P99 ms | Max ms |
|---|---:|---:|---:|---:|---:|
| 核心事件 | 79000 | 0.0020 | 0.0050 | 0.0080 | 0.3608 |
| 本地词典查询 | 11000 | 0.0005 | 0.0013 | 0.0014 | 1.7233 |

独立基准进程峰值工作集为 **5.77 MiB**（6045696 字节），来源 `Windows GetProcessMemoryInfo.PeakWorkingSetSize`。包含 Rust 运行时、引擎、两个词典实例及样本数组；不能解释为输入法空闲内存或纯词典索引占用。

本次小词库下，事件 P95/P99 与查询 P95 均满足文档初始目标。背景负载未控制，未测生产规模词库、大量学习词条、不同机器、受控后台负载及长时间资源增长；也未计量 TSF、IPC、文本框提交或 UI 显示。上述数据不能外推为最终产品成绩。

## 可重放检查

README 记载完整构建与审计命令；`m1-m2-tasks.md` 登记逐项交付；[候选排序回归报告](m2-candidate-ranking.md)保留完整实际结果。CI 对 Windows/Linux 执行同一套命令并上传测量产物，远程运行结果须在连接托管仓库后另行记录。
