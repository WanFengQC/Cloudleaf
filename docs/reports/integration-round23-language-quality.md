# Round 23 中文排序研发对照

正式 Top-1、Top-5、MRR 为 `null`。表中分数衡量与已知来源正文一致的程度。拼音由历史字典或研发轨迹提供，不能代表人工输入意图。

## 定点模型

词表由实际训练表面构建，多个读音词条共享观测，同一表面的出现次数不会增加词条原本的频率。未观测词映射到 UNK，控制验收时仍记录 unknown。一元概率为 (count(w)+1)/(全部入边计数+词表大小)。相邻模型采用 (count(p,w)+8×P(w))/(count(p)+8)。BOS、EOS 参与相邻模型，句内词语项用路径转移均值，混合为词语与字符定点均值。每单位是一比特的 1/1024。全序先比较拼音解释代价（含纠错），再比较可信用户优先级、结构加语言分值和原池顺序。每个研发候选的内部日志包含词条 ID、来源边、子分项与池位置。

最多重排 50 个已去重路径；保留召回上限和不完整预算结果。词语历史只在完整路径上读取，因此没有合并不同前词状态。旧模式保持原搜索路径。小型参考与优化重排比较 1,782 个完整候选排序与分解。

## 各组分母与召回

| 资源与召回 | 样本组 | 样本数 | 有正文 | 图可达 | 池含正文 | 池完整 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 固定 123,544 词条，结构先召回 | Round 16 Tatoeba dev | 32 | 32 | 13 | 2 | 13 |
| 固定 123,544 词条，结构先召回 | Round 17 新闻 dev | 40 | 40 | 9 | 1 | 40 |
| 固定 123,544 词条，结构先召回 | Round 18 输入方案 | 93 | 0 | 0 | 0 | 90 |
| 固定 123,544 词条，结构先召回 | Round 20 固定查询去重视图 | 93 | 0 | 0 | 0 | 90 |
| 固定 123,544 词条，结构先召回 | Round 23 冻结研发验证 | 40 | 40 | 25 | 7 | 25 |
| 固定 123,544 词条，字符先召回 | Round 16 Tatoeba dev | 32 | 32 | 13 | 12 | 12 |
| 固定 123,544 词条，字符先召回 | Round 17 新闻 dev | 40 | 40 | 9 | 4 | 34 |
| 固定 123,544 词条，字符先召回 | Round 18 输入方案 | 93 | 0 | 0 | 0 | 92 |
| 固定 123,544 词条，字符先召回 | Round 20 固定查询去重视图 | 93 | 0 | 0 | 0 | 92 |
| 固定 123,544 词条，字符先召回 | Round 23 冻结研发验证 | 40 | 40 | 25 | 22 | 24 |
| 补充 127,354 词条，字符先召回 | Round 16 Tatoeba dev | 32 | 32 | 13 | 10 | 12 |
| 补充 127,354 词条，字符先召回 | Round 17 新闻 dev | 40 | 40 | 40 | 8 | 30 |
| 补充 127,354 词条，字符先召回 | Round 18 输入方案 | 93 | 0 | 0 | 0 | 91 |
| 补充 127,354 词条，字符先召回 | Round 20 固定查询去重视图 | 93 | 0 | 0 | 0 | 91 |
| 补充 127,354 词条，字符先召回 | Round 23 冻结研发验证 | 40 | 40 | 40 | 29 | 34 |

冻结基础组含 15 项不可映射，覆盖全组 40 项分母。Round 18、20 查询原本没有期待正文，排序指标保持空白。连续按键、原文与并发性能使用完整 Round 22 的 171 事件，结果见性能报告。

| 召回池 | 研发组 | 模式 | 首选 / Top-3 / Top-5 / Top-10 | 正文分母 | MRR | 不完整 / 不可执行 |
| --- | --- | --- | --- | ---: | ---: | --- |
| base-structural | Round 16 Tatoeba dev | structural | 0 / 1 / 1 / 1 | 32 | 0.0156 | 0 / 19 |
| base-structural | Round 16 Tatoeba dev | lexical | 7 / 7 / 8 / 8 | 32 | 0.2250 | 0 / 19 |
| base-structural | Round 16 Tatoeba dev | character-addone | 6 / 9 / 10 / 11 | 32 | 0.2385 | 0 / 19 |
| base-structural | Round 16 Tatoeba dev | character-witten | 5 / 9 / 9 / 11 | 32 | 0.2274 | 0 / 19 |
| base-structural | Round 16 Tatoeba dev | unigram | 0 / 0 / 0 / 0 | 32 | 0.0000 | 0 / 19 |
| base-structural | Round 16 Tatoeba dev | word | 1 / 1 / 1 / 1 | 32 | 0.0312 | 0 / 19 |
| base-structural | Round 16 Tatoeba dev | finite-character | 2 / 2 / 2 / 2 | 32 | 0.0625 | 0 / 19 |
| base-structural | Round 16 Tatoeba dev | mixed | 1 / 1 / 1 / 1 | 32 | 0.0312 | 0 / 19 |
| base-structural | Round 17 新闻 dev | structural | 1 / 1 / 1 / 1 | 40 | 0.0250 | 0 / 0 |
| base-structural | Round 17 新闻 dev | lexical | 1 / 4 / 4 / 4 | 40 | 0.0583 | 0 / 0 |
| base-structural | Round 17 新闻 dev | character-addone | 0 / 0 / 0 / 1 | 40 | 0.0031 | 0 / 0 |
| base-structural | Round 17 新闻 dev | character-witten | 0 / 1 / 1 / 2 | 40 | 0.0150 | 0 / 0 |
| base-structural | Round 17 新闻 dev | unigram | 0 / 0 / 0 / 0 | 40 | 0.0000 | 0 / 0 |
| base-structural | Round 17 新闻 dev | word | 0 / 0 / 0 / 0 | 40 | 0.0000 | 0 / 0 |
| base-structural | Round 17 新闻 dev | finite-character | 1 / 1 / 1 / 1 | 40 | 0.0250 | 0 / 0 |
| base-structural | Round 17 新闻 dev | mixed | 0 / 0 / 0 / 0 | 40 | 0.0000 | 0 / 0 |
| base-structural | Round 23 冻结研发验证 | structural | 0 / 0 / 0 / 2 | 40 | 0.0069 | 0 / 15 |
| base-structural | Round 23 冻结研发验证 | lexical | 9 / 11 / 12 / 14 | 40 | 0.2580 | 0 / 15 |
| base-structural | Round 23 冻结研发验证 | character-addone | 7 / 13 / 14 / 15 | 40 | 0.2521 | 0 / 15 |
| base-structural | Round 23 冻结研发验证 | character-witten | 7 / 15 / 17 / 19 | 40 | 0.2804 | 0 / 15 |
| base-structural | Round 23 冻结研发验证 | unigram | 0 / 0 / 0 / 0 | 40 | 0.0000 | 0 / 15 |
| base-structural | Round 23 冻结研发验证 | word | 2 / 2 / 2 / 2 | 40 | 0.0500 | 0 / 15 |
| base-structural | Round 23 冻结研发验证 | finite-character | 5 / 7 / 7 / 7 | 40 | 0.1500 | 0 / 15 |
| base-structural | Round 23 冻结研发验证 | mixed | 4 / 4 / 4 / 5 | 40 | 0.1031 | 0 / 15 |
| base-character | Round 16 Tatoeba dev | structural | 0 / 1 / 1 / 1 | 32 | 0.0156 | 0 / 19 |
| base-character | Round 16 Tatoeba dev | lexical | 7 / 7 / 8 / 8 | 32 | 0.2250 | 0 / 19 |
| base-character | Round 16 Tatoeba dev | character-addone | 6 / 9 / 10 / 11 | 32 | 0.2385 | 0 / 19 |
| base-character | Round 16 Tatoeba dev | character-witten | 5 / 9 / 9 / 11 | 32 | 0.2274 | 0 / 19 |
| base-character | Round 16 Tatoeba dev | unigram | 1 / 1 / 2 / 5 | 32 | 0.0512 | 1 / 19 |
| base-character | Round 16 Tatoeba dev | word | 1 / 4 / 6 / 7 | 32 | 0.0862 | 1 / 19 |
| base-character | Round 16 Tatoeba dev | finite-character | 6 / 9 / 9 / 10 | 32 | 0.2378 | 1 / 19 |
| base-character | Round 16 Tatoeba dev | mixed | 4 / 5 / 7 / 8 | 32 | 0.1594 | 1 / 19 |
| base-character | Round 17 新闻 dev | structural | 1 / 1 / 1 / 1 | 40 | 0.0250 | 0 / 0 |
| base-character | Round 17 新闻 dev | lexical | 1 / 4 / 4 / 4 | 40 | 0.0583 | 0 / 0 |
| base-character | Round 17 新闻 dev | character-addone | 0 / 0 / 0 / 1 | 40 | 0.0031 | 0 / 0 |
| base-character | Round 17 新闻 dev | character-witten | 0 / 1 / 1 / 2 | 40 | 0.0150 | 0 / 0 |
| base-character | Round 17 新闻 dev | unigram | 0 / 1 / 1 / 1 | 40 | 0.0083 | 6 / 0 |
| base-character | Round 17 新闻 dev | word | 1 / 2 / 2 / 2 | 40 | 0.0375 | 6 / 0 |
| base-character | Round 17 新闻 dev | finite-character | 0 / 1 / 2 / 3 | 40 | 0.0223 | 6 / 0 |
| base-character | Round 17 新闻 dev | mixed | 2 / 2 / 2 / 2 | 40 | 0.0500 | 6 / 0 |
| base-character | Round 23 冻结研发验证 | structural | 0 / 0 / 0 / 2 | 40 | 0.0069 | 0 / 15 |
| base-character | Round 23 冻结研发验证 | lexical | 9 / 11 / 12 / 14 | 40 | 0.2580 | 0 / 15 |
| base-character | Round 23 冻结研发验证 | character-addone | 7 / 13 / 14 / 15 | 40 | 0.2521 | 0 / 15 |
| base-character | Round 23 冻结研发验证 | character-witten | 7 / 15 / 17 / 19 | 40 | 0.2804 | 0 / 15 |
| base-character | Round 23 冻结研发验证 | unigram | 2 / 2 / 4 / 10 | 40 | 0.0806 | 1 / 15 |
| base-character | Round 23 冻结研发验证 | word | 3 / 3 / 6 / 11 | 40 | 0.1068 | 1 / 15 |
| base-character | Round 23 冻结研发验证 | finite-character | 8 / 15 / 16 / 18 | 40 | 0.2859 | 1 / 15 |
| base-character | Round 23 冻结研发验证 | mixed | 7 / 8 / 12 / 14 | 40 | 0.2153 | 1 / 15 |
| supplement-character | Round 16 Tatoeba dev | structural | 0 / 1 / 1 / 1 | 32 | 0.0156 | 0 / 19 |
| supplement-character | Round 16 Tatoeba dev | lexical | 7 / 7 / 8 / 8 | 32 | 0.2250 | 0 / 19 |
| supplement-character | Round 16 Tatoeba dev | character-addone | 5 / 7 / 8 / 8 | 32 | 0.1901 | 0 / 19 |
| supplement-character | Round 16 Tatoeba dev | character-witten | 2 / 5 / 7 / 9 | 32 | 0.1252 | 0 / 19 |
| supplement-character | Round 16 Tatoeba dev | unigram | 0 / 0 / 1 / 1 | 32 | 0.0063 | 1 / 19 |
| supplement-character | Round 16 Tatoeba dev | word | 0 / 0 / 1 / 1 | 32 | 0.0063 | 1 / 19 |
| supplement-character | Round 16 Tatoeba dev | finite-character | 4 / 7 / 8 / 9 | 32 | 0.1712 | 1 / 19 |
| supplement-character | Round 16 Tatoeba dev | mixed | 0 / 0 / 2 / 3 | 32 | 0.0172 | 1 / 19 |
| supplement-character | Round 17 新闻 dev | structural | 1 / 1 / 1 / 1 | 40 | 0.0250 | 0 / 0 |
| supplement-character | Round 17 新闻 dev | lexical | 1 / 4 / 4 / 4 | 40 | 0.0583 | 0 / 0 |
| supplement-character | Round 17 新闻 dev | character-addone | 0 / 0 / 0 / 1 | 40 | 0.0028 | 0 / 0 |
| supplement-character | Round 17 新闻 dev | character-witten | 0 / 0 / 1 / 3 | 40 | 0.0117 | 0 / 0 |
| supplement-character | Round 17 新闻 dev | unigram | 0 / 0 / 1 / 2 | 40 | 0.0086 | 10 / 0 |
| supplement-character | Round 17 新闻 dev | word | 1 / 1 / 2 / 3 | 40 | 0.0348 | 10 / 0 |
| supplement-character | Round 17 新闻 dev | finite-character | 0 / 0 / 2 / 5 | 40 | 0.0198 | 10 / 0 |
| supplement-character | Round 17 新闻 dev | mixed | 1 / 1 / 2 / 3 | 40 | 0.0348 | 10 / 0 |
| supplement-character | Round 23 冻结研发验证 | structural | 0 / 0 / 0 / 2 | 40 | 0.0069 | 0 / 0 |
| supplement-character | Round 23 冻结研发验证 | lexical | 9 / 11 / 12 / 14 | 40 | 0.2580 | 0 / 0 |
| supplement-character | Round 23 冻结研发验证 | character-addone | 7 / 13 / 15 / 20 | 40 | 0.2652 | 0 / 0 |
| supplement-character | Round 23 冻结研发验证 | character-witten | 6 / 14 / 18 / 21 | 40 | 0.2746 | 0 / 0 |
| supplement-character | Round 23 冻结研发验证 | unigram | 2 / 5 / 6 / 9 | 40 | 0.0998 | 6 / 0 |
| supplement-character | Round 23 冻结研发验证 | word | 1 / 5 / 5 / 8 | 40 | 0.0834 | 6 / 0 |
| supplement-character | Round 23 冻结研发验证 | finite-character | 5 / 14 / 17 / 19 | 40 | 0.2490 | 6 / 0 |
| supplement-character | Round 23 冻结研发验证 | mixed | 4 / 5 / 8 / 13 | 40 | 0.1432 | 6 / 0 |

## 退化与失败

以旧字符 Witten–Bell 为对照。以下比较保留同分母的无候选和预算失败。在选型上保留旧默认。

| 召回池 | 研发组 | 新模式 | 改善 | 退化 | 无正文匹配 | 有正文但低位 |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| base-structural | Round 16 Tatoeba dev | word | 0 | 10 | 21 | 0 |
| base-structural | Round 16 Tatoeba dev | mixed | 0 | 10 | 21 | 0 |
| base-structural | Round 17 新闻 dev | word | 0 | 2 | 38 | 0 |
| base-structural | Round 17 新闻 dev | mixed | 0 | 2 | 38 | 0 |
| base-structural | Round 23 冻结研发验证 | word | 2 | 17 | 21 | 0 |
| base-structural | Round 23 冻结研发验证 | mixed | 4 | 16 | 19 | 0 |
| base-character | Round 16 Tatoeba dev | word | 2 | 9 | 21 | 0 |
| base-character | Round 16 Tatoeba dev | mixed | 3 | 6 | 21 | 0 |
| base-character | Round 17 新闻 dev | word | 1 | 0 | 38 | 1 |
| base-character | Round 17 新闻 dev | mixed | 2 | 0 | 38 | 0 |
| base-character | Round 23 冻结研发验证 | word | 2 | 17 | 19 | 0 |
| base-character | Round 23 冻结研发验证 | mixed | 5 | 12 | 19 | 0 |
| supplement-character | Round 16 Tatoeba dev | word | 0 | 8 | 23 | 1 |
| supplement-character | Round 16 Tatoeba dev | mixed | 0 | 8 | 23 | 1 |
| supplement-character | Round 17 新闻 dev | word | 3 | 1 | 36 | 0 |
| supplement-character | Round 17 新闻 dev | mixed | 3 | 1 | 36 | 0 |
| supplement-character | Round 23 冻结研发验证 | word | 5 | 17 | 17 | 1 |
| supplement-character | Round 23 冻结研发验证 | mixed | 7 | 16 | 17 | 0 |

每个改善、退化与低位案例的 ID 和名次见[脱敏对照](integration-round23-evidence/round23-public/language-observations.json)。原始来源句、候选和打分回放留在 Git 外的 `.local/round23-language-*`。新词模型的 1,782 次参考对照仅验证机械行为。固定旧搜索 837 组与 188,325 次机械比较仍分别记录。

PR2 的正式独立质量协议继续采用家族 70/15/15 与 heldout 至少 2,000 家族。作者、两名独立审核者、仲裁、独立保管和真实使用授权未齐之前，正式指标保持空值。24 个不预填答案的待标注任务位见[人工任务包](../acceptance/round23-ime-annotation/README.md)。
