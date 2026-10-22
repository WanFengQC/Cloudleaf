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

每个release含：完整`records.jsonl`事实、`runtime.tsv`投影、拼音/文字`index.json`、质量`quality.json`、完整拒绝位置`rejected.jsonl`、`source-lock.json`、`NOTICE.txt`、`LICENSE.txt`和SHA-256资源`manifest.json`。unknown频率保持null/空，词性保持null。合法源中当前音节表无法表达的读音单列`unsupported_runtime_readings`，不能称作源数据错误；`malformed_or_conflicting_records`另报。

原始英文释义不切成推测义项；Unihan的整个kDefinition保留一个原始字符串。超过256字符的运行时注释为空，完整内容留在事实层，质量报告登记数量。字符常用读音与英文字符释义不代表完整多音字、词短语、中英双向或现代词频。

## 发布与回退

```text
python -m tools.data_pipeline activate .local/unihan-17-a --store .local/data-store
python -m tools.data_pipeline activate <下一版已校验release> --store .local/data-store
python -m tools.data_pipeline rollback --store .local/data-store
```

先校验所有资源及事实/投影/索引的语义关系，复制到按manifest哈希命名的不可变目录，再原子替换`current.json`，上一版保留。相同release重复激活不会抹掉上一版；回退目标损坏、指针越界、版本不兼容或写入失败都保持原指针。排他`.writer.lock`拒绝并发写者；进程异常留下锁时，人工确认没有运行写者后移除。工具只写自己的release目录和指针，不迁移或删除用户词典。

这里的hash保证固定原件和内容完整性，不是发布者签名。`review_status=approved`是可信来源锁的人工审核结论，不会从任意自包含release获得真实授权。导入外部锁/派生包前必须核对可信Git来源、许可原文及hash；正式安装器的签名、断电持久性、跨版本用户词典迁移仍未交付。

## 其他来源

审查登记见`data/sources/source-review.jsonl`。CC-CEDICT不自动下载：本地`cedict-text`锁须由人工补齐固定原件hash、合法取得方式、CC BY-SA4完整许可及署名链，随后用相同build命令导入。测试中的自编三行夹具只验证语法/去重，不代表CC-CEDICT正式数据已经取得。SUBTLEX-CH许可范围和wordfreq中文来源问题保持阻塞。

此版本索引只用于Python离线精确查询验证；Rust消费TSV后建立自身内存索引。索引查询时间不能作为中文候选延迟、输入法端到端延迟或语言质量证据。生产词/短语、统计语料/频率、完整双语数据、独立评估和产品资源预算均未完成，CL-009不关闭。
