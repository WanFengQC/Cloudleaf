# CL-009 Round 5：隔离事实、类型化侧包与人工评测前置设计

2026-10-27；固定基线 `81d85ad414b3ecae6af9573b881acbd9dfa56037`。先提交设计、再提交带真实红测的测试、最后实施工程代码。只改C独立离线工具/新测试/新文档与报告；schema2/3、TSV v2、生产音节表、A/B共享接口、默认引擎均不改。旧报告、旧冻结包、测试不覆盖；无CL-010或发布放行。

## 1. 完整隔离审计

新增 `tools/data_pipeline/audit_isolations.py`，命令行只读固定完整ZIP、来源锁和旧已验证release，在全新输出目录生成 `records.jsonl`、`summary.json`。先核ZIP SHA、唯一成员CRC/明文SHA/头部、release各摘要；独立按源行重建接收/隔离决策，不调用生产读音规范化或生产导入器计算答案。逐条核对旧rejected的reason/member/line/raw及接受记录的源行位置，要求源记录恰好分成接受与隔离，重复/缺失/越界/伪原文全部拒绝。

每条隔离保留繁/简、原读音、原始整行、UTF-8原行SHA（不含换行符）、member和1-based line、原拒绝原因、全部token判别、主分类/证据理由、源合法性审阅状态和未实施处置。不把ASCII形状等同合法普通话，不擅称逗号/间隔号/字母数字是源错误。`r5`单列儿化后缀、`xx5`未指定读音占位、m/hm/hng成音鼻音、其他数字声调但表中缺失音串各列；这些是可重查源表达/运行时差距，语言规范判定仍需人工/明确规范。词源含dialect/onom.只作原文提示，不能据此自动断言规范读音或纠错。UTF-8/预算/混合token不同维度保留，当前1条超16音节不放宽输入预算。

审计输出是CC BY-SA4数据派生物，复制来源LICENSE/NOTICE并记录输入/工具/输出身份；旧1,468条不删除或自动转入生产包。不从词头猜拼音或把完整英语段变语言gold。

## 2. 独立类型化侧包

新增 `tools/data_pipeline/linguistic_sidecar.py` 及其测试。侧包独立 `cloudleaf-linguistic-sidecar-v1`，不投影现有runtime；入口以显式提供的事实entry ID集合验证引用。来源必须含固定版本、数据/许可/审核SHA、批准scope；验证器只能证明声明一致性，不替代来源权利或人工审核真实性。

频率观测必须注明 `token_frequency / document_frequency / zipf`，corpus_id/version/SHA、统计时期、tokenizer id/version/config SHA、coverage和单位；所有整数拒绝bool，数值拒绝NaN/Infinity。

- token_frequency：numerator为出现token数、denominator为明确token总数，`value=numerator/denominator`，单位token proportion。0只在explicit observed且实际numerator=0、有效分母时表示观测零；未收录/低于截断/未观测均为null，不能填0。
- document_frequency：numerator为含词文档数，denominator为文档总数，单位documents；不能重标token比例。真实DF=0同样须有观测与文档分母。
- zipf：区分来源原生值与由同一侧包的已验证正token比例得到的 `log10(p*1e9)`；记录base10/per-billion/母观测ID，不能从DF换算、不能对p=0取对数、不能用缺词默认Zipf=0。原生统计仍须明确来源定义、基准与取样；真实0与unknown有独立coverage状态。

词性保留原tag与tagset版本；空缺为null。义项是人工或来源明确的稳定sense ID、语言、定义、证据及review，并非旧斜线段位置。翻译采用source/target sense的多对多有向关系（equivalent/broader/narrower/related），反向不得自动推导；短语包含kind、原文/组件entry引用及可不连续的组件顺序证据，非全部多字词自动标短语。所有非空注释都须来源证据及两名独立reviewer；有争议时另有不同adjudicator，未完成审核记录不能伪装正式。模型测试只有自编fixture，正式数据没有这些注释就保持null/空集合，不生成假的sense/POS/alignment。

## 3. 来源选择与人工gold计划

重新打开维护者/发布者primary许可及统计定义，保存URL、HTTP结果/实际时间、完整元信息字节SHA（Git文档另有blob/commit），不下载尚未授权的频率/句子包。THUOCL DF的MIT/商业许可不抹掉组成来源与年代，wordfreq的混合来源/默认缺词零不补现unknown，SUBTLEX独立包权利范围不推断通过。新增固定日期中文Wikipedia快照作为**有条件后续自产token统计候选**，须先逐页修订/作者/许可和例外过滤、确定分词器及年代代表性；本轮不取多GB原件，不把一般CC条款当逐页验收。

新增独立人工IME评测计划：自有/明确授权原始中文任务、用户意图/可接受输出与逐键输入由人工双审，仲裁有独立身份；训练/开发/heldout先做来源家族与跨来源近重复全局隔离，保管者独立于开发者，答案封存、版本/参数/源码/二进制冻结，正式gold不能由词典自动生成。取得许可、同意、代表性、样本量、标注人员和封存未完成时状态blocked、准确率null；不通过自填fixture解除B门禁，不自动联系外部作者/标注者。

## 4. 验证与退出条件

新增测试先真实失败（实现模块尚不存在），记录精确测试commit和UTF-8输出；随后最小实现并保留旧55 Python/60 Rust。包括篡改原行/原因/缺行、混合token/后缀/占位/预算、未知与真实0、DF伪token/无分母、Zipf错误母观测/公式/默认0、非法ID/跨源权限/孤儿sense/翻译/短语及review身份的反例。错误路径不覆写已有输出。

冻结源码后运行README locked fetch、debug/release、fmt、Clippy -D warnings、Rust/doc、资源/许可及全部Python；用新git archive/无pip venv、禁止网络的两独立build/verify完整包，全部九产物相同。对新包再做七点实际自有子进程发布/回退中断恢复，保留用户词典探针SHA；不是物理断电。保留R4源与runtime摘要，manifest因工具指纹变化另列，旧给B的2401包保持不动。新原始命令、失败、性能/内存、数据/源码/产物SHA全部进入Round5新目录。

完成范围只包含可重放审计、类型化工程模型/验证与真实来源决策；正式频率包、真实POS/sense/双向翻译注释、人工gold及生产语言覆盖仍未交付，分别列阻塞和下一证据。不以词条量替代质量。最终普通push后精确Windows/Linux CI由根核验，PR3仍Draft，不合main。
