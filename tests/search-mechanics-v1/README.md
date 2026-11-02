# 搜索机制资源 v1

这些是项目自编MIT测试符号和合成转移计数，`language_accuracy_valid=false`。读音映射到甲乙等符号不表达自然语言词义，manual=1023只验证固定词频特征，unknown始终保持缺失。不能发布为正式产品词库，也不能用本表计算正式Top-1/Top-5/MRR。

权重在[先行设计](../../docs/cl008-design.md)中固定：每词-100、末音节前缀-300、已观测频率`floor(log2(frequency+1))*10`；Bigram为加一平滑的定点二进制对数（单位1024）。训练仅来自train.tsv，包含三个按multiplicity重复的符号序列。dev/test案例不会进入模型计数。13条机械案例为2条dev、11条test，家族隔离；这些是可见的机制测试，不冒充语言盲测。已有83条已见诊断seed未加入训练或调参。

`evaluate_search.py`按同一快照运行Reference、ExactDag、Beam64和Beam1，分别配Structural、Weighted、Bigram及ContextRerank。完整保留空输入、OOV、尾撇号等待和拒绝输入。对Exact与Reference要求逐例完整候选/分数/路径ID/特征一致；Beam有损差异保留在报告中。重排只处理基础Bigram选出的unique text最佳代表路径池，不能宣称池内所有切分的最佳上下文分。

重放：

```text
python scripts/evaluate_search.py --iterations 100 --warmup 10 --output .local/cl008-search.json
```

运行宿主的冷加载包括文件读取与Trie构建，计时查询包括规范化、建图与搜索。预热不计入样本。独立进程峰值含模型、运行时及样本数组，驻留路径计数包含当前路径和瞬时扩展克隆，但不是字节内存。环境背景负载不受控制；这些结果不代表正式词库、核心集成或Windows TSF/UI表现。
