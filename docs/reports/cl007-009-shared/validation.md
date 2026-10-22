# CL-007/008/009共享实现检查点

日期：2026-10-22。设计先行提交 `c085601`，共享代码 `8506c71489bf3d1a305b4f6a03f1b8da9f63c45f`，主分支仍为 `a564fd4`。本检查点不包含A/B/C具体生产能力，不进入CL-010。

## 实际变化

保留六个原型crate及全部旧测试，新增只读 `key_disposition`、受修订和候选ID约束的 `select_candidate`、经校验的 `InputConfig`、UTF-8到UTF-16偏移辅助与提交三态值类型。旧默认分页5/候选50、事件协议及排序保持不变。平台仍须独立负责原生事务、Unknown隔离及故障恢复。

审计器改为按 `config/audits/*.json` 精确合并成员、外部包版本/许可/来源与资源政策；重复、缺失、越界、来源/许可变化、坏格式或哈希失败均拒绝。`core.json`仍登记原6个crate和3份资源。许可allowlist不等于漏洞审计，A引入绑定时须追加实际安全数据库审查。

## 本地证据

在Windows 11 10.0.26300 x64 / i5-11400 / Rust1.90.0执行：

- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked --offline -- -D warnings`通过。
- `cargo test --workspace --all-targets --locked --offline`：60个测试函数通过，其中56个原测试完整保留；旧事件语料41条全部通过。新增4项覆盖预判与派发对照、只读不变性、过期/不存在候选、配置边界和代理对UTF-16偏移。
- doc-test通过，`python -m unittest discover -s tests -p "test_*.py"`：14项通过。新审计故障注入验证版本/许可/来源、政策重复/路径逃逸、坏资源/缺失登记；原11项不删改。
- `python scripts/check_workspace.py`通过：6个crate、0个外部Rust依赖，原64/41/83三份资源哈希、条数、来源及许可均相符。

干净代码提交后重跑release基准与原诊断消融；[性能原始记录](performance.json)与[完整诊断摘要](algorithm-summary.json)保留实际HEAD、机器、源码/二进制指纹及测量时间。源码SHA-256为 `06944bfc5acda08ff6bef80af440479a80593cfc6b8064995faf274f5b553099`，与实现提交对应。

79,000事件P50/P95/P99为0.0019/0.0048/0.0105ms，11,000查询P95为0.0006ms，峰值工作集6,008,832字节。仅为64条手工原型词典在本机背景负载下的进程内微基准，不含TSF、真实应用或生产词库；不同运行波动不构成算法改进证据。

旧83条诊断数据的Top-1/Top-5仍为39/83、48/83；heldout仍为31/67、37/67，生产规则MRR仍为0.5012437810945274。超长输入的1个预期不支持错误保留分母。没有使用已见heldout调参，也不把这些成绩称为正式准确率。

## 并行分支

| 方向 | 工作区 | 分支 | 先行设计 |
|---|---|---|---|
| A | `C:/Users/EDY/.codex/worktrees/cl007-windows/Cloudleaf` | `codex/cl007-windows` | `7ddc016` |
| B | `C:/Users/EDY/.codex/worktrees/cl008-search/Cloudleaf` | `codex/cl008-search` | `3807d87` |
| C | `C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf` | `codex/cl009-data` | `21ba6f6` |

各分支同步共享实现后独立检查、记录SHA与局限。正式退出条件仍见各方向状态报告和路线图，本共享检查点不代表CL-007/008/009完成。

## 远程检查

代码及上述报告提交 `e26c749f20b70a0c4cc4a020f58929b2a96d2626` 的[草稿PR #1](https://github.com/WanFengQC/Cloudleaf/pull/1)已建立。[Core CI 39715719807](https://github.com/WanFengQC/Cloudleaf/actions/runs/39715719807)两个job均success：Windows `119617221515`、Linux `119617221863`，所有检查、微基准及旧消融通过。

实际存在的产物：Windows `12398394720`（16,396字节，SHA-256 `1a89cf3540f50ed24ad62267a51da1a607b1903c61f3109717d3e831082ce71b`），Linux `12398334870`（24,023字节，SHA-256 `16081836255745fcb00c290ef6d32b788530d74efa0bf85408b5b2783a50d61d`）。本段为检查完成后的文档追加，不声称本段已包含在该被测提交中。

## 极限状态审查追加

二次审查发现：提交ID耗尽时，旧预判仍返回Handle，但执行会拒绝新提交。修复 `1258311b2945ad9db97ecf7fae52188a67642f19` 在不改变状态的前提下仅阻止会创建提交的键，编辑/导航/取消仍沿既有语义；既有耗尽测试新增一致性与只读断言。原60项Rust、14项Python及fmt/Clippy/doc-test/资源审计重新通过。

干净修复提交后重新执行1000轮基准和旧83条双方案消融。最终[性能记录](performance-final.json)来源SHA为 `fe952611868165681e97c69b867fc3c179e6662a0043bb4c6c7a9dad3b76cd2e`；79,000事件P95/P99为0.0057/0.0116ms，进程峰值5,996,544字节。旧诊断排名与上述结果相同，未调参。旧报告对应旧实现SHA而保留；最终微基准仍只描述原型与当前机器，不是生产性能。
