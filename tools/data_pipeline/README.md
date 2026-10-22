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

## 其他来源

审查登记见`data/sources/source-review.jsonl`。CC-CEDICT不自动下载：本地`cedict-text`锁须由人工补齐固定原件hash、合法取得方式、CC BY-SA4完整许可及署名链，随后用相同build命令导入。测试中的自编三行夹具只验证语法/去重，不代表CC-CEDICT正式数据已经取得。SUBTLEX-CH许可范围和wordfreq中文来源问题保持阻塞。

此版本索引只用于Python离线精确查询验证；Rust消费TSV后建立自身内存索引。索引查询时间不能作为中文候选延迟、输入法端到端延迟或语言质量证据。生产词/短语、统计语料/频率、完整双语数据、独立评估和产品资源预算均未完成，CL-009不关闭。
