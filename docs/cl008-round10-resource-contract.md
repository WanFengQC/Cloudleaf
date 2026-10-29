# CL-008 Round 10：搜索计数资源合同与离线门禁

2026-10-30，先审实际R9代码再实施。默认输入引擎、四种搜索算法和共享接口不改；本轮先修评估入口的真实缺口，再交付可重放资源门禁。

## 结果与预算

GraphEdges、Expansions、ResidentPaths是确定性工作/活路径计数上界，不是墙钟或字节内存。Reference耗尽搜索预算时保留已完成完整路径子集；ExactDag、PositionDag、Beam耗尽搜索预算时返回空候选；图边截断仍可在已有图搜索。预算或Beam剪枝不能声明guaranteed。ContextRerank只保证有限召回池，PositionDag不支持历史评分。Err时原输入由调用者保留；Ok必须raw_fallback等于输入。本轮不增加用户取消API：预算终止不是用户取消。

现`evaluate_real_lexicon.compare`仅核案例ID，可能把budget非空却guaranteed=true的错误观测当完整oracle，也未核row输入/家族。先加旧实现失败反例；新增单一`search_resource_contract.py`验证概念，供旧compare和新gate复用，验证只在入口进行。

## 实施范围与格式

新增`check_search_resources.py`、合同验证模块及Python测试；`resource_probe.rs`显式输出`cloudleaf-search-resource-observation-v2`、完整configuration和真实raw_fallback。旧报告保留旧schema/源码身份，不补虚构字段；新版gate明确拒绝旧观测。Rust新增真实partial/预算/完整路径测试，不复制搜索实现。

使用现获批固定词条证据和24资源探针，Structural禁用词频/模型，output_k=5，输入512字节/256字母。四方法仅method不同；Beam宽度固定8。每组预算四方法完全相同：标准16384/200000/50000；边128/200000/50000；扩展16384/500/50000；驻留16384/200000/128。全部拒绝/预算/候选差异保留，完整参考才作oracle；无完整参考时不拿受限Reference证明正确性。每例预热1、测5次，保留原样本和P50/P95/P99/max，P99小样本不稳定，不宣称更快或产品达标。无需重跑旧大矩阵。

门禁核冻结输入/词库/配置/预算绑定、案例分母、候选k/正文唯一、计数上界、guaranteed/预算/剪枝一致、回退及方法的搜索预算partial约定，保存新增/丢失/排序差异。Rust测试另核候选edge覆盖完整输入。产物含源码/数据/工具/二进制SHA及实际命令。全Rust/Python/fmt/Clippy/doc/release/依赖资源审计照常，旧测试和失败保留。

## 不放行边界

`count_contract_passed`只表示本次计数和差分合同；`production_resource_ready=false`，字节内存/分配峰值未测，整体进程峰值不充当单算法内存。原5ms/10ms工程延迟目标只诊断，不当正式冻结的资源退出阈值。正式gold仍未交付、Top-1/Top-5/MRR为null，默认引擎不替换、不进入CL-010。
