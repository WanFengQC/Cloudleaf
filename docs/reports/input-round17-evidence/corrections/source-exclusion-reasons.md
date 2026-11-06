# Round17 报告勘误

提交7ec66968087d33a5257c885ac50c08b158ffa92c的报告将dev/heldout排除原因写反。按固定任务原件重新核对：

| 分区 | ready | 超长 | 缺字 | 无合适片段 | 保守来源标记 | 合计 |
|---|---:|---:|---:|---:|---:|---:|
| dev | 40 | 1 | 1 | 3（page49046） | 3（page53152） | 48 |
| heldout | 40 | 5 | 0 | 3（page52237） | 0 | 48 |

不更改任何冻结任务、输入、模型、候选结果或质量分母；heldout候选仍未运行。页面53152的保守误报仍保留，不补位。原815个证据文件、原工具和index均保留原字节，新增本勘误与修正后的报告生成源另由最终Git提交及外部交付回执绑定。

原件与结构汇总见`corpus/primary/acquisition-manifest.json.gz`、`corpus/primary/selection-before-content.json.gz`、`corpus/frozen/summary.json`及`corpus/audit/dev-denominators.json`。修正从报告生成源进行，未手改原始报告数据。
