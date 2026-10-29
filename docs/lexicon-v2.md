# Lexicon v2：数据与搜索分支共享格式

此设计先冻结后实施。旧 `# cloudleaf-dictionary-v1` 和64手写词条作为历史回归保持原样。

富记录JSONL由C保存词条、读音、原始义项/字段、nullable词频/词性及完整来源/许可；它是事实层。运行时投影使用UTF-8、LF、无BOM的TSV，首行 `# cloudleaf-lexicon-v2`，第二行严格九列：

```text
id\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id
```

上述 `\t` 表示实际制表符。每行字段数固定，无转义；字符串不得含tab/换行/控制字符或首尾空白。id稳定唯一，pinyin由单空格分隔的小写规范音节，text非空，annotation可空。source/source_record_id/license_id必须可在清单追溯。无法表达的坏记录在质量报告明确拒绝，不静默清洗掉语义。

frequency可空且kind为unknown/manual/count。unknown必须为空，不能用0替代未知；manual仅用于已有手写测试频率，不当作语言事实；count仅在有授权统计来源、时间/范围/单位及校验时使用。缺失词性、义项分割依据或多读音同样保留缺失，不编造。

B自持内存Lexicon解析v1/manual与v2，不更改原型Dictionary。unknown禁用词频观测特征；若算法使用统一先验，它是带版本的模型参数而非伪造数据值。按键路径仅读已载入索引，不读取TSV/JSONL或SQLite文件。

C的manifest登记来源/版本/输入校验、许可文件、变换规则、投影及富记录的SHA-256、条数与质量统计。原件缓存不进入产品；实际分发派生资源单独保留许可/版权。原子升级只切换已经校验完整的release，损坏或版本不兼容不能覆盖当前可用版本。

Unicode17 Unihan只提供字符级常用读音和部分英文原始释义，不等于正式词/短语/现代词频/完整多义词典。C输出这个合法真实范围的资源并如实记录其他数据缺口；B的算法机制实验不能因此宣称正式语言评测完成。
