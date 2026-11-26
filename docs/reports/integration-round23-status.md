# Round 23 语言资源与排序整合

**partial**。仍在 `codex/integration-round21`。Round 22 基线 `d0ddf833a5c60bd661877946d080b97a9e6cf5c9`。实施代码与本地验证绑定 `f90d0e705c2c36792401313fdc07ae68fbda32a1`。最终交付提交与其 push CI、产物校验单另见 Git 外最终回执。

| 能力 | 状态 | 核对结果 |
| --- | --- | --- |
| PR2 评测与人工任务 | done | 导入已有路径，真实 Release 执行机械、固定词典、阶段、资源、限制参考和三道正式就绪门禁。24 个空白标注位可交接。 |
| PR3 语义资源与发布 | partial | 采用同一发布、恢复和严格审批。真实研究词语释义及候选注释已执行，正式 POS、有向关系、短语对齐及数据批准未齐。 |
| 真词语统计与 Rust 重排 | partial | 冻结来源和可复现 TSV，显式研究快照；完整差异包括真实退化。保留现默认引擎、输入完整性和回退。 |
| 旧搜索与候选兼容 | done | 837 组完整结果一致，预算失败 0，188,325 次旧机械候选对照无差异。新参考比较 1,782 项。 |
| 实际翻译质量 | partial | 用同一固定 Marian 双向生成 17 条，12 条受控完整性对照，保留漏报及保守提示。 |
| 独立正式评估与发版 | blocked | 独立人类 gold、审核者、仲裁与 heldout 保管未到位，正式 Top-1、Top-5、MRR 均 null。词频、语义和研究权重未正式放行。 |
| 桌面、TSF 与长测 | not-run | 真实窗口继续 not-run/user-paused。未安装、激活或注册 TSF，未启动 8h/24h。CL-010 关闭。 |

16 项本地检查通过。Rust 420 通过、3 忽略，Python 198 通过。原 50 项要求及边界已逐项复核。原六条开发线、main 和五个 Draft 的最终状态以独立远程快照和回执为准。

下一轮应先扩大经过许可的独立现代领域及真实意图标注，处理候选池召回损失，再比较端到端模型和混排。候选旁释义的真实窗口交互等待当前桌面暂停状态解除。

[语言质量](integration-round23-language-quality.md) · [数据资源](integration-round23-data-resources.md) · [翻译质量](integration-round23-translation-quality.md) · [性能](integration-round23-performance.md) · [50 项产品追踪](cloudleaf-product-readiness.md)
