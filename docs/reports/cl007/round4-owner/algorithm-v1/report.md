# algorithm-v1 排序消融实测

本报告来自实际Rust核心；只评价手工诊断seed，不是正式产品算法成熟度结论。两个算法都忽略context且没有纠错预测。

数据：83条；dev与heldout组隔离，未使用dev调参，train为空。

| 算法 | 分组 | 样例 | Top-1 | Top-5 | MRR | 非空候选率 | gold覆盖 | P95 ms |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| production-rules-v1 | all | 83 | 0.4699 | 0.5783 | 0.5191 | 0.5783 | 0.5783 | 0.0064 |
| production-rules-v1 | lexicon_known | 66 | 0.5909 | 0.7273 | 0.6528 | 0.7273 | 0.7273 | 0.0029 |
| production-rules-v1 | dev | 16 | 0.5000 | 0.6875 | 0.5938 | 0.6875 | 0.6875 | 0.0042 |
| production-rules-v1 | heldout | 67 | 0.4627 | 0.5522 | 0.5012 | 0.5522 | 0.5522 | 0.0067 |
| production-rules-v1 | ambiguity | 4 | 0.5000 | 0.5000 | 0.5000 | 0.5000 | 0.5000 | 0.0081 |
| production-rules-v1 | common | 12 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0029 |
| production-rules-v1 | context | 6 | 0.0000 | 1.0000 | 0.4306 | 1.0000 | 1.0000 | 0.0024 |
| production-rules-v1 | correction | 15 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.003 |
| production-rules-v1 | extension | 2 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0008 |
| production-rules-v1 | homophone | 2 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0029 |
| production-rules-v1 | long | 4 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0099 |
| production-rules-v1 | mixed | 2 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0001 |
| production-rules-v1 | oov | 2 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0022 |
| production-rules-v1 | phrase | 9 | 0.5556 | 0.5556 | 0.5556 | 0.5556 | 0.5556 | 0.0042 |
| production-rules-v1 | prefix | 21 | 0.8571 | 1.0000 | 0.9286 | 1.0000 | 1.0000 | 0.0026 |
| production-rules-v1 | umlaut | 4 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0015 |
| frequency-only-ablation-v1 | all | 83 | 0.4699 | 0.5783 | 0.5171 | 0.5783 | 0.5783 | 0.006 |
| frequency-only-ablation-v1 | lexicon_known | 66 | 0.5909 | 0.7273 | 0.6503 | 0.7273 | 0.7273 | 0.003 |
| frequency-only-ablation-v1 | dev | 16 | 0.5000 | 0.6875 | 0.5938 | 0.6875 | 0.6875 | 0.0045 |
| frequency-only-ablation-v1 | heldout | 67 | 0.4627 | 0.5522 | 0.4988 | 0.5522 | 0.5522 | 0.0067 |
| frequency-only-ablation-v1 | ambiguity | 4 | 0.5000 | 0.5000 | 0.5000 | 0.5000 | 0.5000 | 0.0085 |
| frequency-only-ablation-v1 | common | 12 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0043 |
| frequency-only-ablation-v1 | context | 6 | 0.0000 | 1.0000 | 0.4028 | 1.0000 | 1.0000 | 0.0045 |
| frequency-only-ablation-v1 | correction | 15 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0029 |
| frequency-only-ablation-v1 | extension | 2 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0007 |
| frequency-only-ablation-v1 | homophone | 2 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.002 |
| frequency-only-ablation-v1 | long | 4 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.01 |
| frequency-only-ablation-v1 | mixed | 2 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0001 |
| frequency-only-ablation-v1 | oov | 2 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0022 |
| frequency-only-ablation-v1 | phrase | 9 | 0.5556 | 0.5556 | 0.5556 | 0.5556 | 0.5556 | 0.0043 |
| frequency-only-ablation-v1 | prefix | 21 | 0.8571 | 1.0000 | 0.9286 | 1.0000 | 1.0000 | 0.0037 |
| frequency-only-ablation-v1 | umlaut | 4 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0014 |

全部失败、OOV、未支持策略、超长与无候选样例均保留在分母。lexicon_known仅表示gold存在于固定词典，不代表当前输入可检索到它。纠错零预测precision=null；required样例recall=0。JSON包含按group及split/category的完整指标，raw-results.tsv保留100次计时样本，case-comparison.tsv逐项保留候选与排名变化。

| 样例 | split | gold | 现有规则rank | 频率消融rank | 消融rank改善 | 候选顺序改变 |
|---|---|---|---:|---:|---:|---|
| dev-001 | dev | 好 | 1 | 1 | 0 | False |
| dev-002 | dev | 号 | 2 | 2 | 0 | False |
| dev-003 | dev | 好 | 1 | 1 | 0 | False |
| dev-004 | dev | 好 | 0 | 0 | 0 | False |
| dev-005 | dev | 好好 | 0 | 0 | 0 | False |
| dev-006 | dev | 国 | 1 | 1 | 0 | False |
| dev-007 | dev | 过 | 2 | 2 | 0 | False |
| dev-008 | dev | 国 | 1 | 1 | 0 | False |
| dev-009 | dev | 国 | 0 | 0 | 0 | False |
| dev-010 | dev | 果子 | 0 | 0 | 0 | False |
| dev-011 | dev | 学习 | 1 | 1 | 0 | False |
| dev-012 | dev | 学习 | 1 | 1 | 0 | False |
| dev-013 | dev | 学习 | 0 | 0 | 0 | False |
| dev-014 | dev | 学习 | 1 | 1 | 0 | False |
| dev-015 | dev | 界 | 1 | 1 | 0 | False |
| dev-016 | dev | 界 | 2 | 2 | 0 | False |
| held-001 | heldout | 中 | 1 | 1 | 0 | False |
| held-002 | heldout | 中文 | 1 | 1 | 0 | False |
| held-003 | heldout | 中心 | 1 | 1 | 0 | False |
| held-004 | heldout | 中华 | 1 | 1 | 0 | False |
| held-005 | heldout | 中文 | 1 | 1 | 0 | False |
| held-006 | heldout | 中心 | 1 | 1 | 0 | False |
| held-007 | heldout | 中华 | 1 | 1 | 0 | False |
| held-008 | heldout | 中 | 0 | 0 | 0 | False |
| held-009 | heldout | 中国 | 0 | 0 | 0 | False |
| held-010 | heldout | 中文 | 0 | 0 | 0 | False |
| held-011 | heldout | 是 | 1 | 1 | 0 | False |
| held-012 | heldout | 时 | 2 | 2 | 0 | False |
| held-013 | heldout | 事 | 3 | 3 | 0 | False |
| held-014 | heldout | 市 | 4 | 4 | 0 | False |
| held-015 | heldout | 是 | 1 | 1 | 0 | False |
| held-016 | heldout | 世界 | 1 | 1 | 0 | False |
| held-017 | heldout | 世界 | 0 | 0 | 0 | False |
| held-018 | heldout | 世界 | 0 | 0 | 0 | False |
| held-019 | heldout | 西 | 1 | 1 | 0 | True |
| held-020 | heldout | 希 | 2 | 3 | -1 | True |
| held-021 | heldout | 先 | 2 | 2 | 0 | False |
| held-022 | heldout | 先 | 2 | 2 | 0 | False |
| held-023 | heldout | 咸阳 | 0 | 0 | 0 | False |
| held-024 | heldout | 兴安 | 0 | 0 | 0 | False |
| held-025 | heldout | 安 | 1 | 1 | 0 | False |
| held-026 | heldout | 安 | 1 | 1 | 0 | False |
| held-027 | heldout | 安 | 0 | 0 | 0 | False |
| held-028 | heldout | 参 | 1 | 1 | 0 | False |
| held-029 | heldout | 参 | 1 | 1 | 0 | False |
| held-030 | heldout | 参 | 1 | 1 | 0 | False |
| held-031 | heldout | 参 | 0 | 0 | 0 | False |
| held-032 | heldout | 考 | 1 | 1 | 0 | False |
| held-033 | heldout | 考 | 1 | 1 | 0 | False |
| held-034 | heldout | 考 | 0 | 0 | 0 | False |
| held-035 | heldout | 人 | 1 | 1 | 0 | False |
| held-036 | heldout | 人 | 1 | 1 | 0 | False |
| held-037 | heldout | 人 | 1 | 1 | 0 | False |
| held-038 | heldout | 人 | 0 | 0 | 0 | False |
| held-039 | heldout | 工作 | 1 | 1 | 0 | False |
| held-040 | heldout | 工作 | 1 | 1 | 0 | False |
| held-041 | heldout | 工作 | 1 | 1 | 0 | False |
| held-042 | heldout | 工作 | 0 | 0 | 0 | False |
| held-043 | heldout | 工作 | 1 | 1 | 0 | False |
| held-044 | heldout | 云 | 1 | 1 | 0 | False |
| held-045 | heldout | 云 | 1 | 1 | 0 | False |
| held-046 | heldout | 云 | 0 | 0 | 0 | False |
| held-047 | heldout | 笺 | 1 | 1 | 0 | False |
| held-048 | heldout | 笺 | 1 | 1 | 0 | False |
| held-049 | heldout | 笺 | 0 | 0 | 0 | False |
| held-050 | heldout | 您 | 1 | 1 | 0 | False |
| held-051 | heldout | 宁 | 1 | 1 | 0 | False |
| held-052 | heldout | 女人 | 0 | 0 | 0 | False |
| held-053 | heldout | 旅行 | 0 | 0 | 0 | False |
| held-054 | heldout | 略 | 0 | 0 | 0 | False |
| held-055 | heldout | 虐 | 0 | 0 | 0 | False |
| held-056 | heldout | 绿 | 0 | 0 | 0 | False |
| held-057 | heldout | 女性 | 0 | 0 | 0 | False |
| held-058 | heldout | 我们 | 0 | 0 | 0 | False |
| held-059 | heldout | 北京 | 0 | 0 | 0 | False |
| held-060 | heldout | 今天天气很好 | 0 | 0 | 0 | False |
| held-061 | heldout | 我在学习中文 | 0 | 0 | 0 | False |
| held-062 | heldout | 你好世界 | 0 | 0 | 0 | False |
| held-063 | heldout | 云笺2026 | 0 | 0 | 0 | False |
| held-064 | heldout | 工作2026 | 0 | 0 | 0 | False |
| held-065 | heldout | 中国 | 0 | 0 | 0 | False |
| held-066 | heldout | 是 | 0 | 0 | 0 | False |
| held-067 | heldout | 中文中文中文中文中文中文中文中文中文中文 | 0 | 0 | 0 | False |

排名0表示未命中，改善值计算时按51处理；它不是平均质量收益的替代指标。不同方案需在更大独立词库/语料、长句和真实平台上对照，不能只凭seed或这一次消融决定最终模型。

## 产物来源

```json
{
  "measured_at_utc": "2026-10-23T16:04:19.379393+00:00",
  "source_sha256": "88fcfefd694709e4304108b9001d00fc86e306f67755ab95381d84cfbccf3027",
  "source_fingerprint_schema": "cloudleaf-source-v3",
  "dataset_sha256": "d8bf45fcb578db3c7ed1a2d7cce5c5e98f126de70dcfe3cfe69eb4becc0618fb",
  "binary_sha256": "0fbd457e42ca0a97cfb5995a9ca932d2d96a44995cb814356d4f1e983aa492fc",
  "dictionary_sha256": "d4a87e958b23097253a4601b9ae373f02964e3e37729bb7f560e20835d5bb5f0",
  "rustc": "rustc 1.90.0 (1159e78c4 2025-09-14)",
  "git_head": "5c0ba9d651038eca5b3fcb53dba3f6835cf13a0c",
  "git_tracked_dirty": false,
  "environment": {
    "os": "Windows-11-10.0.26300-SP0",
    "cpu": "Intel64 Family 6 Model 167 Stepping 1, GenuineIntel",
    "machine": "AMD64"
  },
  "profile": "release",
  "warmup_per_case": 20,
  "measurements_per_case": 100,
  "candidate_limit": 50,
  "context_used": false,
  "correction_capability": false
}
```
