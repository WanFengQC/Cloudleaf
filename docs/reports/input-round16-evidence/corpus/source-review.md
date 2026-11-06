# Round 16 真实上下文语料来源审查

2026-11-06，结论：Tatoeba 固定周导出可用于本地、显式启用的研究训练；不自动批准生产发布。原件、当前owner及许可声明均不是独立人工gold或用户输入实录。

## 选择及许可范围

- 采用 [Tatoeba 官方导出](https://tatoeba.org/en/downloads) 中 Mandarin Chinese (`cmn`) detailed/base/tags、cmn-cmn 与 cmn-eng links。当前快照 Last-Modified 为2026-10-31；URL会每周更新，准确版本由本地原件SHA绑定。
- 官方 [复用说明](https://en.wiki.tatoeba.org/articles/show/using-the-tatoeba-corpus) 和已实际取得的 [terms §6](https://tatoeba.org/en/terms_of_use) 将文本默认许可定义为 CC BY 2.0 FR；[CC 法律原文](https://creativecommons.org/licenses/by/2.0/fr/legalcode) §3允许复制、派生、分发，§4要求署名、许可及衍生说明，没有NC或SA条款。[英文deed](https://creativecommons.org/licenses/by/2.0/fr/deed.en)明确商业利用及改编在条件下允许。
- 详细第4列为 current owner，不保证 original creator。保留每句ID、owner、原句URL、时间和原件版本；不把owner虚构为已签授权人或标注员。官方FAQ推荐Tatoeba来源链接及许可署名，terms同时要求作者署名；正式分发前仍须确认完整署名处理。
- 官方 [规则](https://en.wiki.tatoeba.org/articles/show/guidelines) 禁止上传侵权、AI生成句和非公众真实人物的隐私内容，但官方同时声明无法穷尽审核，不能把来源规则当成每条原文都已审查的证据。已标记copyright/quote/by/问题标签和email形状记录在本轮隔离。
- 音频不在此许可与下载范围；没有下载、训练或再分发音频。CC0中文导出仅1行，不能成为足够规模的上下文源。
- 未采用 UD Chinese-GSDSimp：虽然仓库标CC-BY-SA4.0，[UD官方许可说明](https://universaldependencies.org/contributing/licensing.html) 明确GSD类注释许可未必覆盖原文权利；没有以注释许可推定全部文本已授权。未下载大规模Wikipedia或不明权利的输入法/用户语料。

## 实际身份与范围

| 原件 | 字节 | SHA-256 |
|---|---:|---|
| cmn_sentences_detailed.tsv.bz2 | 1727096 | 374dc649bd61ed5597c58ad9f098edb831e7bbe4ea0831f82947bec1fe5c8bb8 |
| cmn_sentences_base.tsv.bz2 | 516974 | d7edaa6c5096f98bd55fe7eed3c9c5797ef8aa024cd72d4079d55f9d628b348e |
| cmn_tags.tsv.bz2 | 8080 | 52da76642b6a485e7702a0c965062a697bc84cd9696ebf218336ae0da63ce764 |
| cmn-cmn_links.tsv.bz2 | 4224 | fd3fb1ac4c43a8ba795c1dd9c193e37fdfc3ddfcf88cb5694f6db369521be37c |
| cmn-eng_links.tsv.bz2 | 481706 | 417b2697222806aaf22bf7f510dd5eed246869a1ef40d7d80c6b43f4758b54b8 |

五个数据压缩包均完整读到EOF、通过bzip2 CRC、严格UTF-8解码；详细TSV解压8648703B，SHA43c4e88355e5897aa20a1b1e73852a1edb70b6cd51a426be4dda6f08ca545005。原件89248句/89248独立ID，6字段形状全部符合。973个current owner，不等于973名已核验原作者。原文1044514 Unicode scalar，最长500；创建/修改时间存在0000-00-00未知哨兵，不把它解释为有效时间。日期最大2026-10-30。

语料包含简繁混合及众包学习/翻译句，12289条标original、76724条translation、235条unknown；不是自然抽样的现代中文IME分布。没有自动简繁转换、词频/DF/Zipf推导或人工标签生成。

## 冻结处理结果

`prepare.py` 只读取固定原件，可用 `--source-dir` 和 `--output-dir` 重放，输出目录必须新建；入口同时验证五个压缩原件及五个解压TSV的完整SHA。

- 提取原文中连续Han范围字符，非Han截断；每片段2–128 Unicode scalar。BOS/EOS只代表这些保留片段边界，未必是语义完整句边界。
- 历史4目标及明确简繁对应的**包含子串**原句排除43条；与其原文/base/英文共根/近重复同族的另1句排除。没有训练目标答案表。
- 原句总隔离699；原因允许重叠：问题/可能侵权/quote/by等来源标签654、历史直接43、历史关联1、email/URL/号码形状4。标签稀疏，未标记不证明质量或权利。
- 按base、cmn-cmn、共享eng连接和规范化全文归族；长度≥8规范化字符，三字符shingle集合Jaccard≥4/5也连接。83362族，最大19句。此标准不保证捕获所有语义改写或完整跨语言翻译家族。
- 固定家族SHA规则按80/10/10划分后，heldout > dev > train优先消除相同完整片段跨分区重复；规则先于模型运行冻结。train/dev/heldout序列全文SHA交集0，非train原文及规范化全文SHA黑名单与train原文交集0。
- train实际86737片段/716681字；dev11067/90722；heldout11146/90869。train未达100k序列、1M字上限，无容量截断。train.txt SHA1878767d1f0076011cac0be2c67afdfa0256a961030a80d91503a145d007590e。
- **所有分区仍同属tatoeba-community全局来源家族**，只能作engineering-only内部隔离，不能升级为独立人工gold或正式语言质量证据。没有读取保留集来选择模型参数。

完整分母与SHA见 `prepared-v3/summary.json`；逐句原文、ID、current owner、URL、时间、家族、隔离理由见Git外 `source-lineage.jsonl`，不建议把整份原文/人员owner清单加入Git或发布包。

## 实际失败与独立核验

首次源处理严格要求全部cmn-eng边引用存在的Chinese ID时返回1，发现85条边引用不在detailed导出中的ID；当前周官方导出不保证链接闭包。随后将这些85条边明确隔离，未伪造对应句，原件不变，失败记录及悬空边清单保留。

优化近重复前缀连接另用完整all-shingle参考对照：58933条长度≥8的原文，1064644候选对，参考176条近重复边，优化同176，missing/extra均0；见 `verify-near.json`。

真实train片段产生5082字符词表、143560个BOS/字符/EOS转移键、803418次真实转移观测，见 `actual-char-transitions.json`。这些是所选学习例句片段中的观测，不是正式词频、模型质量或现代IME准确率。模型训练与Rust推理需由主任务代码实际执行后单独记录。
