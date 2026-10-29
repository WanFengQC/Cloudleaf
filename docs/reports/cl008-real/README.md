# CL-008 真实词条资源与精确压缩检查点

2026-10-22。**部分完成；CL-008退出条件未满足，不进入CL-010。** 新增显式`PositionDag`实验方法，默认仍为`ExactDag`，输入核心仍使用既有实现。设计先行`56cd796`、`96699ee`、`2a2a13c`，实现`3b69e51`、`db5397d`。本页及JSON没有独立语言正确答案，正式Top-1/5、MRR、词频和模型质量均为null。

## 数据与不可推断的范围

- Unihan17完整导入快照：44,431条单字词条，全部unknown频率；runtime SHA-256 `1b13b199440c0ca9b59074de9100f58c6a0b847b12a7b986e1b752084ff60f8e`。这是完整单字快照，不能当完整中文多字词库。Unicode-3.0许可和修改说明保留在[正文](Unicode-3.0.txt)及报告来源字段。
- 中文Wiktionary研究摘录：701条读音记录、521个不同多字词、532个拼音键，全部unknown；runtime SHA-256 `b47287d0fd405c225409653cd8d08f34f9fcb31ef8bb8c9b9e54dbcf5fd7741c`。C的`44cecd4`构建快照从已取得压缩数据固定前8,388,608字节派生66,593条完整原JSONL行，舍弃139字节残尾；不验证整包SHA或gzip CRC。`source_subset=true`、`research_only=true`、`full_source_download_complete=false`、`full_gzip_crc_verified=false`；来源顺序偏差很大，不是完整dump、正式词库或独立评测集。
- 多字来源为Chinese Wiktionary contributors，Tatu Ylonen/Wiktextract提取，CC BY-SA4.0；中文gloss没有转成英文。C已隔离已知权利标记，但逐页例外审查尚不完整，此批准仅限锁定研究摘录。保留[构建清单](wiktionary-build-manifest.json)、[来源审核锁](wiktionary-source-lock.json)、[质量报告](wiktionary-quality.json)、[署名与修改](wiktionary-NOTICE.txt)及[许可正文](CC-BY-SA-4.0.txt)。[搜索报告](wiktionary-prefix.json)和[逐页候选归属](wiktionary-candidate-attributions.json)中派生词条文本适用CC BY-SA4.0，工具代码独立适用MIT；未把上游材料重新授权为MIT。

两份快照分别完整载入，没有按探针筛词，也没有把二者拼接为更大词库。测试输入为先登记的24条人工**无gold资源探针**，并非词典自派生答案。没有train/dev/heldout、统计词频或授权训练语料，所有方法统一Structural；频率与语言模型特征禁用。非空候选数只说明机制可输出文本，不是准确率或语言覆盖率。仅按稳定ID打平会产生生僻单字组合，不说明语言合理。

## 冻结对照与证据

每配置一个独立release进程，全部24例保留，预热5次、采集30次；JSON的`samples_ns`保持采集顺序，分位数另用排序副本。每条计时包括规范化、建图及搜索；读盘/建索引另列`load_ns`。内存为Windows操作系统记录的进程PeakWorkingSetSize，含词库、索引、候选、宿主和样本。背景负载未控制，以下分位数均为**24例中各例分位数的最大值**，不是混合样本分位数，也不是真实TSF或产品性能。

[优化前原件](unihan-before.json)保留旧Exact的11例预算耗尽、8例非空、最大单例P95 284.485ms/P99 318.191ms，不用后来的成功覆盖失败。[新配对原件](unihan-after.json)在同一44,431条快照、同一探针和固定预算下重跑原8配置，并加4个Position配置。实现源码指纹`350599e5d94fb3a810babd5c7a1f8117c6f0782c911c15062a3c4121aef04798`、构建提交`db5397d62591ef97e5347d1f37ce9a361e074bff`。原件包含命令、二进制/数据/探针/协议指纹及完整环境。

| 配对配置 | 非空/全部24例 | 预算触发例 | Beam剪枝例 | 最大单例P95 ms | 最大单例P99 ms | 峰值MiB |
|---|---:|---:|---:|---:|---:|---:|
| Reference | 19 | 15 | 0 | 234.647 | 248.977 | 33.91 |
| 原Exact | 8 | 11 | 0 | 410.077 | 450.265 | 33.94 |
| Beam1 | 19 | 0 | 16 | 9.599 | 9.825 | 33.90 |
| Beam8 | 19 | 0 | 16 | 98.584 | 108.014 | 36.97 |
| Beam64 | 17 | 2 | 16 | 330.743 | 392.976 | 35.32 |
| Position | 19 | 0 | 0 | 41.087 | 53.481 | 33.93 |
| 原Exact/边128 | 3 | 20 | 0 | 3.390 | 4.555 | 33.93 |
| Position/边128 | 3 | 20 | 0 | 0.297 | 0.476 | 33.89 |
| 原Exact/扩展500 | 3 | 16 | 0 | 0.861 | 1.016 | 33.88 |
| Position/扩展500 | 3 | 16 | 0 | 1.068 | 1.387 | 33.93 |
| 原Exact/驻留128 | 1 | 18 | 0 | 0.506 | 0.563 | 33.93 |
| Position/驻留128 | 19 | 0 | 0 | 44.958 | 49.443 | 33.87 |

原Exact本次P95比旧次更慢，反映重复测量和背景负载波动，未删去。Position对部分短例也有退化，例如`short-shi` P95从0.313ms到0.491ms；不能只挑改善例宣称最优。其标准配置仍有13个词/短句/长串探针P95超过协议5ms诊断目标，且长串P99达到53.481ms，不能宣称输入响应已达标。

新Position在同一真实规模的扩展和路径驻留数有明确减少，例如`zhongguo`从103,696扩展/1,951驻留降至2,063/16；长句`woxiangqubeijing`从触及200,000扩展上限到22,563扩展/26驻留。固定完整Reference仅7例可用，原Exact完整11例；Position在这11例逐项文本、分数、ID路径、特征和状态一致，0差异。其余11个合法探针虽有完整Position输出，Reference及旧Exact都耗尽，**没有这11例完整独立参照的验证结论**。2例非法输入仍拒绝，不充当完整搜索正确性。所有预算/Beam剪枝原因及相对输出的新增、丢失、名次变动逐例保留在JSON和[可读对照](per-case.md)；旧Exact触预算后的5条“新增”是可用输出变化，不是候选质量收益。

整词边对照只有在整图未截断且原Exact完整时才`comparison_valid=true`；其余可观察增删不能当候选格收益。Beam输出有16例被剪枝，不能用低延迟推断无损。

## 真实多字摘录的配对

[完整结果](wiktionary-prefix.json)同样运行全部12配置、24探针、5预热/30样本，源码指纹相同。22例合法搜索均有完整Reference，Position/原Exact结果逐项一致，所有配置0预算触发、0Beam剪枝。只有`zhongguo`、`beijing`、`xuexi`、`prefix-zhongg`四例非空；全部空、OOV、等待和2例非法拒绝都保留，4/24不是准确率。

原Exact最大单例P95/P99为0.0243/0.0324ms、进程峰值5.42MiB；Position为0.0154/0.0165ms、5.42MiB。稀疏701条摘录未对Beam宽度构成压力，不能据此选择生产预算，亦不能外推大规模多字词库性能。中文/英文释义、POS与数据质量属于C的来源报告，本搜索资源实验未评估翻译可靠性。

## 回归与下一步

[检查原件](validation.json)记录build、fmt、clippy、Rust all-targets/doc、资源审计和Python全套命令及返回码，全为0：**74项Rust、26项Python**通过。旧测试全部保留。既有105输入×4评分×3个k的1260组Reference/Exact/宽Beam比较保持通过；另对105输入×2个可加评分×3个k增加630组Position完整差分，并补同文多切分、ID打平、前缀/边界、512字母最大频率加法上界、历史评分拒绝及瞬时驻留预算测试。

[机械release实验](mechanical-search.json)保留13例×4评分对照，原Exact/Beam64逐例相同，Beam1原有4/4/5/5例差异仍保留。[原型事件基准](prototype-performance.json)为旧64条词典、79,000事件，P95/P99 5.3/9.5μs；它与真实词条搜索、TSF及正式质量分开，不用于产品性能结论。旧83条种子诊断也已重放，未据此调参或转称正式准确率。

已完成的是许可快照消费、可重放资源工具、真实规模问题暴露和可解释的精确状态压缩实验。尚缺完整多字正式词库、授权train/dev/封存heldout、真实频率/模型、纠错/模糊音/简拼/双拼/学习/增量及默认核心接入。正式评估的家族隔离、冻结和质量目标已[预登记](../../cl008-real-resource-protocol.md)，均处于未评估；B/A/C各自退出条件保持未满足。

本地重放（证据描述指向C的固定快照，不能替换成同名的新构建）：

```powershell
python scripts/evaluate_real_lexicon.py --evidence .local/real-lexicon/unihan-evidence.json --iterations 30 --warmup 5 --output .local/real-lexicon/unihan-after.json
python scripts/evaluate_real_lexicon.py --evidence .local/real-lexicon/wiktionary-evidence.json --iterations 30 --warmup 5 --output .local/real-lexicon/wiktionary-prefix.json
```

外部路径可按报告内runtime/build/source材料SHA重新绑定；证据描述原件包含在对应结果的`evidence`字段。C后续修正证据逻辑文件名的构建版本不回写本报告历史快照；数据和代码指纹分别保留。
