# CL-001 / CL-002 基线与真实远程CI

日期：2026-10-22（Asia/Shanghai）。仓库：[WanFengQC/Cloudleaf](https://github.com/WanFengQC/Cloudleaf)。

## CL-001：已完成

在原始目录与公共源码实际审查后，对42个明确文件建立检查点 `26b5e45da8e4d77a59d57817db82ac814b5e4b0a`（`chore: establish validated M1-M2 baseline`），随后推送用户指定的空远程仓库main。未强推，也未纳入本轮新规格/评估文件。target、.local、个人配置、日志和密钥文件被忽略；未发现真实凭据或未授权资源。完整暂存diff只保存在忽略的本地审查目录。

基线前已复跑：git diff检查、fmt、Clippy警告拒绝、44个单元测试、41条事件回归的1个集成测试、doc-test、workspace release构建、数据/许可/零外部依赖审计。工具链Rust/MSRV 1.90.0。

已重新生成 `m2-performance.json` 并同步历史验收表：源码SHA-256 `81a42d0a6c89eee5350b73f3dfc4a22fdf8cc677f66336570bc4116857a70d05`，二进制SHA-256 `f5a620c3b6378be4469fc61fd6cce57637b544db8d2f19615a89f7e450aa9ee8`。本次与先前源码一致。测量发生于初次commit之前，JSON的git_head=null是实际状态，不能事后伪填检查点SHA；Git中的对应报告和源码可由该检查点重放。

原型本机重测事件P95/P99为0.0050/0.0080ms，查询P95为0.0013ms，峰值工作集约5.77MiB。79,000事件、11,000查询、64条人工词典。这些仅描述该微型原型和当前负载，不证明生产/端到端成绩；原始报告保留实际时间和环境。

## CL-002：基线真实通过

[Core CI运行39709545895](https://github.com/WanFengQC/Cloudleaf/actions/runs/39709545895) 由上述精确提交的push触发，最终状态success。

| Job | ID | 实际结论 |
|---|---:|---|
| core (windows-latest) | 119597327112 | success |
| core (ubuntu-latest) | 119597327401 | success |

两个job的固定工具链、格式、Clippy、单元/事件测试、doc-test、依赖/数据审计、release基准和上传步骤全部成功。性能报告产物已实际上传：

| Artifact | ID | ZIP字节数 | SHA-256（GitHub登记） |
|---|---:|---:|---|
| core-performance-ubuntu-latest | 12396940168 | 917 | `sha256:3660bdf48aa34475c8981a2206cf52939e283601be38599540fa2585b14e09ac` |
| core-performance-windows-latest | 12396925242 | 947 | `sha256:5f5396a8459cac1b5f123ff242834434c3187334135461f5adc385a5aca32892` |

机器可读摘要见 [cl001-cl002-ci.json](cl001-cl002-ci.json)。以上核实了运行、步骤及产物元数据，未下载ZIP，不将远程机器性能值混入本机报告。GitHub产物有过期时间，未来若过期需重跑，不冒充仍可下载。

后续新代码/数据/契约测试使用新的提交和CI结果，不能沿用这次成功冒充后续版本已通过。当前Linux证据为公共核心CI，不等于Linux输入法系统适配；Windows证据也不是真实TSF/应用兼容性验收。
