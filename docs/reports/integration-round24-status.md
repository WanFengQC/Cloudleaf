# Round 24 交付状态

本轮在 `codex/integration-round21` 继续。Round 23 最终基线为 `ff09385ebcca323b5f68051ee9bb56c0070696de`，其实现提交为 `f90d0e705c2c36792401313fdc07ae68fbda32a1`。本轮先记录设计 `dd234b3` 和翻译约定 `709cb5a`。召回配置与实际探针冻结于 `c87ee64ce15797fec895b0aed1893c60251c5123`，完整本地回归绑定证据清单的实现提交。最终提交、原始 CI 及四份构建产物身份由 Git 外 `.local/round24-delivery-receipt.json` 绑定。

| 方向 | 状态 | 本轮证据 |
| --- | --- | --- |
| 候选丢失诊断 | done | 找到 15 条词面覆盖缺失，并分别定位结构池截止、字符池截止和驻留预算。尚无因果证据的旧去重、合并与剪枝保留 unknown。 |
| 新召回与词语模型 | partial | 串行配额交错和有界词语上下文束已实现，均保留默认关闭。独立整路径遍历对照 2,186 个候选。新冻结样本保留近似丢失、配对退化及预算失败。两方案均不放行。 |
| 分词与统计 | done | 在相同训练和各自固定词典上实际双构建最少词元与概率分词。原有计数格式和模型上限保留。生产词频保持 unknown。 |
| 扩大语言来源 | blocked | 新来源许可和获取条件完成初审，未获授权的新原件未下载。新冻结 40 条和训练无分区家族重合，但共享全球来源。 |
| 旧算法兼容 | done | 固定 837 对旧结果完全一致，预算失败为 0；188,325 次完整机械候选比较无差异。 |
| 输入并发性能 | partial | 固定 171 键三个重复，词汇模式空闲／推理 P95 为 6.77／16.50 ms，词语研究为 2.54／4.36 ms。并发比值分别为 2.44、1.72，均未通过 1.5 上限。 |
| 翻译模型对照 | partial | 核对 M2M-100、MADLAD-400、NLLB-200。新增权重获取、内存、速度和双向推理待授权。固定 Marian 新跑 26 条原始译文并复现旧 17 条。 |
| 关键语义保护 | partial | 受控错误复核从 4/12 到 10/12，5 条等价对照未触发；保留复杂角色或否定范围漏报，译文仍需确认。 |
| 正式评估与语义发布 | blocked | 24 个待标注槽，正式授权样本、真实作者、签名双审、仲裁及封存仍未交付。正式 Top-1、Top-5、MRR 保持 null。真实语义侧包现有研究功能和审批状态沿用。 |
| 权重切换、软分词计数、强化解码 | not-implemented | 新模型接入计划保持于现有 Provider 与 WorkerClient 协议。未引入强制占位符或自动修译。 |
| Windows 自然交互以及长期稳定 | not-run | 真实桌面为 not-run/user-paused。保持系统输入法、TSF 和 8h/24h 测试状态。CL-010 继续关闭。 |

完整源码改动按交付提交范围复查。搜索新增 `cloudleaf-search/src/recall.rs`、候选回归及无窗口例程。训练与冻结工具继续使用既有 `build_word_context.py` 和 `freeze_word_context_validation.py`。翻译只增加既有完整性检查器的条件、回归和研发探针，沿用请求、推理和上屏协议。Windows 与 Linux CI 保留原工作项，研究诊断例程加入两份整合产物。

16 项本地检查绑定 `473fa9d3e2553ded61e5947def7916ae46027b3d`，返回码全部为 0。Rust 426 项通过、3 项忽略，Python 201 项通过。五项独立旧搜索、阶段和资源工具返回 0，三项独立语言评估／人工交接门禁各返回预期的 2。原分配探针触发工作区 unsafe 禁令，已移到独立例程。原格式检查与专项测试失败日志保留。精确推送 CI 及发布包的本地核验不替代 Windows 真实应用验收。

[召回](integration-round24-recall.md) · [语言质量](integration-round24-language-quality.md) · [数据资源](integration-round24-data-resources.md) · [模型选型](integration-round24-model-selection.md) · [翻译质量](integration-round24-translation-quality.md) · [性能](integration-round24-performance.md) · [50 项追踪](cloudleaf-product-readiness.md) · [本地检查与哈希清单](integration-round24-evidence/index.json)
