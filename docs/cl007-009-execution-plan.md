# CL-007 / CL-008 / CL-009 实施计划

日期：2026-10-22。检查起点为本地/远程main `a564fd4`，工作区干净，Rust 1.90.0 / Edition 2024；MSVC/Windows SDK及真实交互Windows会话可用。计划先提交，代码随后独立提交；不直接向main合入大规模变更。

## 事实与阶段

计划制定时，main 的六个crate只有输入原型及CLI，没有TSF生产模块、全输入候选格或正式数据管线。独立分支的后续实现见[并行审阅状态](reports/cl007-009-review-status.md)，不能将计划基线误读为分支现状。平台协议测试是模型；64词条、41事件、83已见种子保持不变，仅作回归/诊断。三个方向各自达到退出门禁前不开始CL-010。

| 阶段/分支 | 修改范围 | 前置与交付 |
|---|---|---|
| `codex/cl007-009-contracts` | 本计划、平台契约v2、lexicon-v2、依赖审计政策；types/input只读预判、候选ID选择、分页配置及测试；审计器和CI获取依赖 | 先提交设计，再提交共享代码；复跑全部旧测试/基准。主分支保持原样 |
| A `codex/cl007-windows` | `adapters/windows/**`、`apps/windows-ime/**`、注册卸载/真实应用脚本、`config/audits/windows.json`、A设计/状态/报告、分支Cargo/lock/CI | 基于共享提交，锁定审核过的COM绑定；完整事件/焦点/组合/候选UI/提交/恢复；真实应用验收单列 |
| B `codex/cl008-search` | `crates/cloudleaf-search/**`、参考/差分、机械数据与评估/基准、`config/audits/search.json`、B设计/状态/报告、分支Cargo | 自持已加载Lexicon，不改原型；直接构建覆盖原输入区间的词条DAG；同图同评分比较穷举、精确DP及Beam |
| C `codex/cl009-data` | `tools/dictionary-import/**`、来源/许可、格式/索引/升级/质量工具、真实源派生资源/清单、`config/audits/data.json`、C设计/状态/报告 | 许可审查后才获取源。先实现Unicode 17 Unihan可复现管线；中文词/短语/双语/现代词频分别登记缺口，不伪造完整覆盖 |

三条方向使用三个独立工作区，不在同一checkout写接口。工作区路径和实际SHA在建立后记录。每条方向先有独立设计提交，再实现、测试、基准和检查点。共同基线的types/input/审计器如需追加变更，由协调分支实施，随后显式同步；不得三边各改一个不兼容版本。

## 共享边界

平台v2在保留旧事件与行为的基础上新增只读接管判断、修订+稳定候选ID选择、受校验的候选页/输出配置、UTF-16偏移辅助。`Applied/NotApplied/Unknown`采用公共值类型，原生事务和隔离仍归平台；Unknown不映射成允许重试的失败。Unicode预编辑、任意选区、外部编辑重基等未实装能力必须明确列缺口。

B自持Lexicon和新搜索结果类型，不改变原型Candidate以强塞尚未验证的LM字段，不把pinyin原型的32路径裁剪当作唯一候选图。A在CL-010前可使用既有核心检验平台集成，不能据此声称成熟算法已接入。DAG是表示，DP是搜索算法；不得把它们错误地包装成互斥算法名称。

C发布富记录JSONL及[lexicon-v2运行时投影](lexicon-v2.md)，frequency缺失必须为空且kind=unknown，不产生以0冒充真实词频的v1导出。B禁用unknown的频率特征，legacy手写频率标manual。词典导入/模型加载发生在按键路径外。

## 依赖和许可证预审

A首选 `windows = =0.62.2` / `windows-core = =0.62.2`，官方包声明MSRV 1.82、MIT OR Apache-2.0，与本项目1.90兼容；以实际Cargo.lock/metadata对所有传递依赖逐项审核并写精确清单。[windows包](https://docs.rs/crate/windows/0.62.2/source/Cargo.toml.orig)、[windows-core包](https://docs.rs/crate/windows-core/0.62.2/source/Cargo.toml.orig)、[COM宏线程说明](https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-implement.md)。核心继续forbid unsafe，仅原生crate允许经说明的unsafe，线程亲和COM禁用默认agile。

C首选Unicode官方固定17.0.0 Unihan，保留Unicode License V3及版权notice。[Unicode17数据](https://www.unicode.org/Public/17.0.0/ucd/Unihan.zip)、[许可](https://www.unicode.org/license.txt)、[UAX38](https://www.unicode.org/reports/tr38/)。kMandarin只是最常用读音，不是完整多音字表；新版已移除kFrequency，不可生成真实现代词频。CC-CEDICT为CC BY-SA 4.0，必须署名/同许可处理派生；下载站自动访问条款及原件取得方式核实前不自动导入。wordfreq复合数据含额外署名/许可来源，未逐一审查前不整体引入。

审计按[依赖与资源政策](dependency-audit-policy.md)扩展精确allowlist，不能取消旧成员/资源、许可证或哈希门禁。CI先locked fetch再offline构建；按键路径始终离线。

## 测试与报告

- 共享：全部旧fmt/Clippy/单元/41事件/宿主契约/Python/资源检查，新增只读预判、过期候选、配置边界及UTF-16测试；release微基准只作为原型对照。
- A：Windows构建/FFI审查、生命周期/异步/重入/故障、注册撤销与权限、原生候选/布局；在独立临时窗口记录记事本/浏览器/多行编辑器真实文本、事件、OS/app版本及延迟。模拟模型不替代真实应用。
- B：同一图/评分的参考穷举与DP差分，Beam宽度/预算及候选去重对照；整数特征/N-gram/上下文方案隔离。训练/dev/测试按家族隔离，禁止83已见seed调参；机械/合成实验标language_accuracy_valid=false，正式选型需要新授权语料。
- C：相同输入/配置两次输出字节一致；原始来源/许可/校验、坏记录/冲突/重复/缺失频率、索引/查询、原子升级/回退、破坏资源故障注入；真实词库质量与管线正确性分开验收。

每条状态表分别使用完成、部分完成、未完成、外部阻塞，列精确提交、命令、失败/资源数据与证据。退出条件未满足时不进入CL-010，也不将分支局部完成称为产品完成。

## 风险与处置

| 风险 | 处置/禁止事项 |
|---|---|
| TSF排队不等于已写入、写后失败或COM重入 | 保留会话/修订/事务身份及三态，未知隔离，不盲目重发；不持可变借用跨COM调用 |
| 注册/真实测试影响用户系统或窗口 | 仅Cloudleaf独立标识与临时测试窗口，记录原状态、可撤销；不改其他输入法，不触碰现有文档 |
| 原生GUI自动化能力或应用权限不足 | 先确认真实桌面和可用工具；无法获得原生证据就登记未完成，不能改用模型冒充 |
| 搜索爆炸或去重导致top-k漏召回 | 图边/状态/路径/内存共同预算；小规模参考差分及截断计数，不仅限最终候选数 |
| 缺训练语料/真实频率 | 禁止用heldout拟合，unknown保持缺失；机制实验不代表语言准确率或最优方案 |
| 数据再分发/署名/同许可或站点条款不清 | 先审核后获取；未授权资源不纳入产品，真实范围不足保留部分完成 |
| 旧审计/CI硬编码阻碍扩展 | 精确政策分文件扩展；保留旧测试和资源，不删门禁换通过 |
| 三方向接口漂移/过早合并 | 共同契约提交、独立分支/工作区、逐方向review；main未经检查不动 |
