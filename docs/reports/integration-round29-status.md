# Cloudleaf Round 29

整合分支：`codex/integration-round21`。起始检查点为 Round 28 `d6db2c849385d5c3617592ca8e247fe55fa31b9e`，原验证运行 45920642794。重新核对了所有受保护分支和五个 Draft PR。产品功能规范和文件范围见[设计](../integration-round29-design.md)。

| 方向 | 结果与报告 |
| --- | --- |
| 固定预算简拼召回 | 两个冻结目标在实测输入帧均排第 1。第二条冻结轨迹的空格提交通过。原 43 个不完整图已恢复。详见[简拼证据](integration-round29-simplified-input.md) |
| 简拼延迟 | partial。171／243 键简拼 P95 为 21.373／29.367 ms，均高于 5 ms。243 键延迟本轮仍回归 |
| 词典资源 | 基础与补充槽独立保存、后台准备、双核心切换和回退。详见[词典运行时](integration-round29-dictionary-runtime.md) |
| 真实 Worker | 固定 171 键，单线程正常优先级八轮配对。汇总倍率 1.202×，四项门禁通过。单轮通过 5/8，稳定性仍 partial。详见[并发性能](integration-round29-worker-performance.md) |
| 历史行为与交付 | [兼容与发行](integration-round29-compatibility.md) |

## 实现与边界

生产代码提交为 `983a9be991b5e1de2a8d87d12fde8c98f5884538`。构图耗尽仍明确报告 GraphEdges，完整但不保证全局最优的候选只由受支持的按位置简拼路径发布。展开或驻留达到上限、多段混排与研究重排仍使用现有保护。词典托管采用已登记来源和独立资源修订，只有显式启用后才加载研究补充层。真实 Windows 双宿主完成修订 1–7、9–10 的收敛；损坏的修订 8 被拒绝。

基线全套的首次并行检查有三项原生恢复测试失败。相同提交安静重跑全目标通过，保留了失败分母、原断言及原始日志。本轮没有改变心跳、超时或系统调度。独立历史行为比对与测试原始证据见[交付索引](integration-round29-evidence/index.json)。

本地 16 项检查全部返回 0。Rust 全目标 484 通过、0 失败、7 条件忽略，Python 213 通过。固定真实词典及宿主的条件专项另行执行并返回 0。历史 837 组搜索、188,325 次机械候选对照、2,186 条参考候选与 1,548 条随机参考候选通过。480 条冻结研发结果与原版本一致。68 条混排回放共 1,676 行，除事件耗时外全部一致。63 条固定双向推理的输出及风险评估保持一致，另用实际新 Worker 复跑 Bridge 恢复。全套命令及返回码位于 `.local/round29-verified-results.json`。

最终推送按包含本报告的精确 HEAD 验证。Windows/Linux 的原始 job、四份产物与 CI 日志 ZIP、SHA-256、ZIP CRC、文件清单、许可和已保护引用记录于本机 `.local/round29-delivery-receipt.json`。

## Round 30 建议

先对 243 键简拼尾部的相交和评分耗时分开做精确剪枝验证。用小图穷举和现有两条固定目标证明候选排序与事务等价，再考虑更紧凑的状态或候选表示。用另一轮独立的长尾性能配对验证峰值，继续记录 Worker 的每对顺序及样本差异。

正式 Top-1、Top-5、MRR 均为 `null`，生产词频仍为 `unknown`。真实桌面验收保持 `not-run/user-paused`。TSF、8h/24h 及系统安装保持 `not-run`。CL-010 保持关闭。
