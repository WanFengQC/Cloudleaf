# Cloudleaf 离线数据管线

要求Python3.11+，仅标准库。所有命令从仓库根目录执行，PowerShell先设置`$env:PYTHONUTF8="1"`。本工具在后台或发布阶段运行；输入事件不调用Python、磁盘或网络。运行时TSV v2遵循[格式约定](../../docs/lexicon-v2.md)，尚未切换旧输入核心的原型数据。

## 固定Unicode17数据

来源与许可证已审核，精确hash见`data/sources/locks/unihan-17.0.0.json`。首次通过官方批量HTTPS地址取得，后续命中缓存只校验、不联网；变更hash、来源或许可必须重新人工审查，不能自动接受上游变化。

```text
python -m tools.data_pipeline fetch --lock data/sources/locks/unihan-17.0.0.json --output .local/data-sources/Unihan-17.0.0.zip
python -m tools.data_pipeline build --lock data/sources/locks/unihan-17.0.0.json --source .local/data-sources/Unihan-17.0.0.zip --license data/licenses/Unicode-3.0.txt --output .local/unihan-17-a
python -m tools.data_pipeline build --lock data/sources/locks/unihan-17.0.0.json --source .local/data-sources/Unihan-17.0.0.zip --license data/licenses/Unicode-3.0.txt --output .local/unihan-17-b
python -m tools.data_pipeline verify .local/unihan-17-a
python -m tools.data_pipeline benchmark .local/unihan-17-a --iterations 10000 --output .local/unihan-performance.json
python -m unittest discover -s tests -p test_data_pipeline.py -v
```

`build`纯离线，拒绝已有输出目录，不改变任何历史release。对两次输出的全部文件逐字节比较，而不是只看词条数。不同Python内置Unicode规范化版本会写入manifest；同版本、同source-lock和参数才能要求完整manifest逐字节一致。

每个release含：完整`records.jsonl`事实、`runtime.tsv`投影、拼音/文字`index.json`、质量`quality.json`、完整拒绝位置`rejected.jsonl`、`source-lock.json`、`NOTICE.txt`、`LICENSE.txt`和SHA-256资源`manifest.json`。unknown频率保持null/空；Unihan词性保持null，词条schema3仅保留来源明确的POS。合法源中当前音节表无法表达的读音单列`unsupported_runtime_readings`，歧义、运行时预算和许可/引用待审分别统计，均不算源坏行；`malformed_or_conflicting_records`另报。

原始英文释义不切成推测义项；Unihan的整个kDefinition保留一个原始字符串。超过256字符的运行时注释为空，完整内容留在事实层，质量报告登记数量。字符常用读音与英文字符释义不代表完整多音字、词短语、中英双向或现代词频。

`verify`从事实与拒绝JSONL独立重算不同文字/拼音键、缺失字段、超长注释、拒绝原因及unsupported/坏记录统计，并校验来源范围和未评测声明。原始行数、目标属性数、源重复/冲突等需要固定原件重建才能证明；自包含校验明确输出`source_only_statistics_rebuilt=false`，不能把包内这些计数当作已重新统计原件。

## 发布与回退

```text
python -m tools.data_pipeline activate .local/unihan-17-a --store .local/data-store
python -m tools.data_pipeline activate <下一版已校验release> --store .local/data-store
python -m tools.data_pipeline rollback --store .local/data-store
```

先校验所有资源及事实/投影/索引的语义关系，复制到按manifest哈希命名的不可变目录，再原子替换`current.json`，上一版保留。相同release重复激活不会抹掉上一版；回退目标损坏、指针越界、版本不兼容或写入失败都保持原指针。`.writer.guard`使用操作系统排他锁，退出自动释放；`.writer.lock`记录owner PID和启动标识，崩溃遗留时必须显式恢复。工具只写自己的release目录和指针，不迁移或删除用户词典。

```text
python -m tools.data_pipeline recover --store .local/data-store --owner-pid <已中断写者PID>
python -m tools.data_pipeline recover --store .local/data-store --owner-pid <已中断写者PID> --rollback
```

恢复只接受已确认退出的owner；活进程、PID复用或权限/状态不明均拒绝。Windows使用只读进程句柄查询，绝不用`os.kill(pid,0)`模拟POSIX探测。先检查指针及所有孤儿安全，再将固定`.publish-*`、`current.tmp`及owner锁移到`.recovered-*`隔离目录；不自动选择新版本，不删除缓存和用户词典。`--rollback`显式切回已校验previous。实际子进程退出/终止测试不代表物理断电持久性。

发布路径只接受普通本地目录：逐级`lstat`拒绝符号链接、Windows junction及全部重解析点，普通文件硬链接同样拒绝；检查包含父路径、元数据、release资源和恢复孤儿内部。恢复在创建guard之前及持有guard后重新校验。`current.tmp`独占创建，已有文件不覆盖；可捕获失败只清理本次创建且身份未变的临时文件，进程中断仍需显式恢复。该门禁不提供对其他进程并发替换路径的安全沙箱，云文件重解析目录不在支持范围。

## 固定真实词条研究摘录

完整Kaikki raw gzip原件提前断流，尚无全件SHA/CRC。`wiktionary-zh-prefix-jsonl`独立格式只用于固定前8MiB中的完整JSONL行，不放宽完整gzip门禁；来源锁的`source_subset/research_only`为true，不能当完整dump或正式词库。

```text
python -m tools.data_pipeline extract-prefix --prefix .local/data-sources/zhwiktionary-prefix-8388608.gz.partial --sha256 fdf3ae6918edb0e6af91cad20dfb51a2ecad8ce59ab026b4ab4e1e7f33581518 --output .local/data-sources/replayed-prefix.jsonl --metadata .local/data-sources/replayed-prefix.json
python -m tools.data_pipeline build --lock data/sources/locks/wiktionary-zh-20261001-prefix8388608.json --source .local/data-sources/replayed-prefix.jsonl --license data/licenses/CC-BY-SA-4.0.txt --output .local/wiktionary-replay
```

schema3保留中文gloss、原POS和明确英文翻译；九列v2 annotation仅投影英文翻译的word，缺英文为空。稳定`source_record_id`在`records.jsonl`映射`attribution_url`与源行SHA；消费者展示或派生报告须保留逐页署名/许可链。仅选择原词头，不自动简繁互转或用单字拼出词语读音。全部频率/频率观测仍null/unknown。媒体、引用实例不输出，可得额外权利标记记录隔离；逐页许可例外未全部人工复核，限制随来源锁保留。派生文本CC-BY-SA4与MIT工具代码分开。

这里的hash保证固定原件和内容完整性，不是发布者签名。`review_status=approved`是可信来源锁的人工审核结论，不会从任意自包含release获得真实授权。导入外部锁/派生包前必须核对可信Git来源、许可原文及hash；正式安装器的签名、断电持久性、跨版本用户词典迁移仍未交付。

## 固定完整CC-CEDICT词条包

第四轮重新审查了MDBG的开放数据许可和站点取得限制。MDBG页提供bulk，但footer禁止脚本访问，未找到明确例外，因此不自动请求该站的导出、查询或API。另行审核的GitHub分发者在固定commit中提供完整原始ZIP及CC BY-SA4授权，适用范围见`data/sources/locks/cccedict-20260604-mirror7019e1c.json`；没有取镜像解析代码、JSON、CC-CANTO或粤语数据。

```text
python -m tools.data_pipeline fetch --lock data/sources/locks/cccedict-20260604-mirror7019e1c.json --output .local/data-sources/cccedict-20260604.zip
python -m tools.data_pipeline build --lock data/sources/locks/cccedict-20260604-mirror7019e1c.json --source .local/data-sources/cccedict-20260604.zip --license data/licenses/CC-BY-SA-4.0.txt --output .local/cccedict-20260604-a
python -m tools.data_pipeline verify .local/cccedict-20260604-a
python -m unittest discover -s tests -p test_cedict_source.py -v
```

本地缓存名不使用上游文件名中的冒号，避免NTFS将其解释为备用数据流。新`cedict-zip-v1`固定单成员`cedict_ts.u8`、ZIP CRC、展开字节数/SHA和完整版本头/源条数；不在磁盘解包，拒绝路径、额外成员、链接及声明的特殊文件类型。带日期的完整镜像不等于最新官方版本或发布者签名。

125,012条源记录中接受123,544条，多字110,045条；1,468条不支持读音或超预算记录保持逐行隔离。事实层保留繁简原文、区分大小写的数字声调读音、旧版斜线分段义项和原行。没有自动繁简转换、从单字补读音或推断词性，全部词频仍unknown/null。稳定来源ID不包含可修订释义，相同身份的不同释义冲突全部隔离。英语原始释义存在不表示义项对齐或双向翻译已验收；241条过长注释只省略运行时投影，完整内容仍在事实层。

原授权README与LICENSE原文以无损UTF-8 JSON字符串保存在`data/sources/evidence/cccedict-mirror-7019e1c/upstream-documents.json`，重建Git blob逐份核验，保留上游原始空白。CC BY-SA4数据保留其署名、许可及修改/同许可义务，不能按本项目MIT代码许可重新标记。新完整词条包的双构建、质量、独立评测边界和恢复证据见[第四轮报告](../../docs/reports/cl009-round4.md)。

## Round 5：只读隔离审计与独立类型侧包

```text
python -m tools.data_pipeline.audit_isolations --lock data/sources/locks/cccedict-20260604-mirror7019e1c.json --source <已锁定完整ZIP> --release <已校验完整词条包> --output-dir <全新审计目录>
python -m tools.data_pipeline.linguistic_sidecar --sidecar <独立侧包JSON> --facts <records.jsonl> --facts-sha256 <事实文件SHA256>
```

第一条逐源行独立重放接受/隔离边界，固定原件应保留1,468条隔离，输出每条原文、读音token形状分类、源位置/原行SHA、原原因与未更改处置，附CC BY-SA4许可/署名。不会修改旧包、生产音节表或自动补读音；固定原件0来源冲突，通用冲突审计不在范围，遇冲突失败。`r5`、`xx5`、成音鼻音、无空格声调串、标点/数字/字母与预算维度分列，形状不是语言规范证明。

第二条只校验独立`cloudleaf-linguistic-sidecar-v1`的token比例、DF文档数和原生/派生Zipf类型、观测零/unknown、完整语料/预处理身份，以及POS/义项/有向翻译/短语引用和来源/双审声明。正计数float下溢不得记为真实0，派生Zipf母子不可换词/换语料。侧包不进入现有schema2/3或TSV v2，不自动训练/排序；自编fixture不是真实频率、注释或gold。格式正确不能证明许可证、reviewer身份或语言事实真实。

[设计](../../docs/cl009-round5-design.md)、[来源采用与模型边界](../../docs/cl009-round5-frequency-and-annotation.md)及[人工评测实际阻塞计划](../../data/plans/ime-human-gold-v1.json)分别登记。当前没有批准的现代token频率、正式POS/义项对齐或独立人工gold，不补0、不用词条量声明准确率。本轮证据另存，不覆盖Round 4冻结包和前轮报告。

本轮具体修改、逐条隔离审计、固定归档回归、双构建及恢复证据见[Round 5状态报告](../../docs/reports/cl009-round5.md)。报告保留未冻结工作态诊断的身份限制，正式结论绑定最终实现和新包摘要。

## 其他来源

2026-10-23新增[正式来源候选与质量门禁](../../docs/cl009-formal-data-review.md)，逐项区分许可声明、实际取得版本与项目采用决定；[第三轮报告](../../docs/reports/cl009-round3.md)追加固定两来源双构建、18条审计和真实中断恢复证据。候选清单不自动增加已批准来源，历史锁和七条审核记录不变。

审查登记见`data/sources/source-review.jsonl`。旧本地`cedict-text`锁仍须人工补齐固定原件hash、合法取得方式、CC BY-SA4完整许可及署名链；测试中的自编夹具仅验证语法/去重，不能代替新完整ZIP的取得证据。SUBTLEX-CH许可范围和wordfreq中文来源问题保持阻塞，其他候选也不因CC-CEDICT镜像批准而自动通过。

此版本索引只用于Python离线精确查询验证；Rust消费TSV后建立自身内存索引。索引查询时间不能作为中文候选延迟、输入法端到端延迟或语言质量证据。完整来源词条已可重放，生产读音/覆盖验收、统计语料/频率、完整双语数据、独立评估和产品资源预算仍未完成，CL-009不关闭。

## Round 6 独立离线工具

[设计](../../docs/cl009-round6-design.md)和[人工任务交付映射](../../docs/cl009-round6-annotation-handoff.md)定义本轮范围。`annotation_workflow`支持待填队列、双审/仲裁绑定、全局split隔离和不可覆盖冻结，真实12任务仍缺人员/授权/答案；不自动产生gold。`source_assertions`只从既有批准研究片段输出多字词源POS/义项/英语翻译断言及双向lookup，反查不新增反向翻译事实；`corpus_gate`只核固定官方revision与实际准入证据，不对未审wikitext伪造词频。

```text
python -m tools.data_pipeline.source_assertions --source <固定JSONL> --lock data/sources/locks/wiktionary-zh-20261001-prefix8388608.json --output-dir <全新研究目录>
python -m tools.data_pipeline.corpus_gate --revision <固定官方API响应> --page-id 342 --revision-id 94816134 --output <新准入报告JSON>
```

缺rights/作者历史/提取/分词证据时准入exit2、token/DF/Zipf均null；现有生产包、解析/发布/回滚和输入核心不变。所有工具只在离线工程路径调用，无实时输入依赖。

## Round 9：固定Ngrams原件只读分析

```text
python -m tools.data_pipeline.ngram_analysis --source .local/round8-ngram-acquisition/1-00000-of-00001.gz --totalcounts .local/round8-ngram-acquisition/totalcounts-1 --output .local/round9-ngram-analysis.json
```

仅接受R8固定两个SHA；无网络、不改输入、不覆盖输出。完整扫描核CRC，逐年原字段、标签形态、异常全分母和有限原行SHA例子见新报告；诊断异常退出2而不删除后续记录。标签不是POS事实，field3和不是去重DF，全部field2合计不是覆盖率。缺失不补0、production=false、正式token/DF/Zipf和语言指标null。派生统计按原编纂CC BY3保留署名/许可/修改说明，代码许可另计；不授权原书全文。详细语义推断与采用阻断见[设计](../../docs/cl009-round9-ngram-analysis.md)。

## Round 7：来源矩阵和审核失效

```text
python -m tools.data_pipeline.source_review --output <全新矩阵JSON>
```

矩阵逐项重放八条来源审查、三个固定锁及已选Wikipedia固定版本的R6未通过快照，核对许可文本/证据和已登记原件SHA。缺取得日期、权限、词频仍null；已有正式词条范围不变，候选和研究摘录不升级production。来源声明与现有审核结论分列，不是自动法律批准器。

人工任务的两review及生效仲裁现在还必须提供`review_binding_sha256=sha256(canonical({protocol_sha256,preprocessing_sha256,lineage_audit_sha256}))`，冻结manifest记录binding schema1。任一处理/来源家族协议变更使旧审核失效；旧R6审核缺字段须重新审核，不能自动迁移。任务摘要和family-index八字段保持，但B仍需显式核对新审核版本，不能凭相同形状放行。

[Round 7设计](../../docs/cl009-round7-design.md)和[正式频率/语义发布前契约](../../docs/cl009-round7-semantic-release.md)列明当前类型侧包与正式发布的差距。没有真实人工/授权/频率、内容绑定审核和可信发布清单时，不存在可批准的production语义侧包或独立gold。
# Round 10独立语义bundle

`python -m tools.data_pipeline.semantic_bundle --help`提供候选build/verify与受显式外部可信审批清单约束的activate/rollback/recover。旧词典schema2默认API保持，新sidecar不进入当前输入引擎。pending内容可读但生产拒绝；当前真实20项仍pending。操作、审批文件边界及签名尚未实现的限制见[说明](../../docs/acceptance/cl009-round10/README.md)和[实测报告](../../docs/reports/cl009-round10-status.md)。

# Round 9固定Ngrams只读诊断

新增`python -m tools.data_pipeline.ngram_analysis --source <R8固定gz> --totalcounts <R8固定totalcounts> --output <新报告路径>`，仅接受固定SHA，完整CRC后输出原字段/年份/缺项与异常分母。不会下载、构建词频或批准production；输出拒绝覆盖，异常/partial返回2并保留报告。统计解释和限制见[设计](../../docs/cl009-round9-ngram-analysis.md)及[实测报告](../../docs/reports/cl009-round9-status.md)。
