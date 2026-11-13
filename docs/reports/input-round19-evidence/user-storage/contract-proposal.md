# Round19 用户词库最小存储提案

仅提案，等待主任务设计提交后实现。不修改实际工作区。

独立 `cloudleaf-user` crate 依赖已有 `cloudleaf-pinyin` 和固定 `ring 0.17.14`。不引入数据库、JSON库、网络、Windows业务依赖或输入事件类型。宿主有界worker负责调用，本crate所有I/O明确是管理路径。

## 内存模型

- `UserEntry { id, revision, enabled, text, pinyin }`，按文本+规范读音去重，同文不同读音合法。
- `UserDictionary { generation, next_id, learning_enabled, entries, preferences, purge_history }`，内部字段私有；返回只读词条。
- ID `user:<递增u64>`，`next_id`持久化，不因删除回收。导入同ID不同内容报冲突，同内容同ID重复无操作，同文本读音不同ID报冲突。允许基础词典重复词，但由组合层标识用户偏好，基础ID不改。
- 词条上限10000，最多16汉字/16读音、拼音80字节/归一字母64字节、整个文件8MiB。具体范围可由主设计修改。只有入口校验中文、拼音、长度、重复，不在查询内部反复检查。
- preference独立`(stable_entry_id, entry_fingerprint) -> bounded_count`，只保存身份、指纹和计数，无整句/应用/标题；上限1000000饱和，最多10000键。
- 学习默认OFF，开启/关闭持久化；清学习只删除preference，不删手工词条。可信Applied及session/epoch/commit幂等由输入/Host控制，本crate不根据任意CommitSucceeded推断系统确认。

## 编码

确定性UTF8/LF，BOM和非法UTF8拒绝；所有字符串不能含控制字符，TSV无需转义。

```
# cloudleaf-user-v2
generation\t1
next_id\t3
learning\t0
entries\t2
id\trevision\tenabled\ttext\tpinyin
user:1\t1\t1\t云笺\tyun jian
user:2\t1\t1\t云笺\tyun qian
preferences\t0
id\tfingerprint\tcount
sha256\t<全部前文含LF的SHA256>
```

v1作为明确的旧格式迁移测试：只有generation/next_id/entries及id/text/pinyin列；加载后rev1/enabled、学习OFF，无伪造历史已有正式用户格式。未来版本拒绝；迁移失败保留原件。导入默认仅合并词条，不从文件静默启用学习；恢复备份是另一显式操作。

## 磁盘与并发

固定绝对根目录由平台传入；不可含父跳转，现有组件拒绝symlink/reparse，输入/导出路径在入口单次检查，不作为安全沙箱宣称。Windows实际路径由平台核实LocalAppData，测试用Git外独立临时目录。不可写源码/安装目录。

`Store::open(root)`创建/打开固定`.writer.guard`并调用Rust1.90稳定的File::try_lock，生命周期持锁；另一实例明确Busy并保留基础输入，不自动抢锁。进程异常后OS释放锁，无PID复用恢复框架。

管理文件：`generation-00000000000000000001.clu`、唯一pending临时文件及guard。保存检查expected_generation，create_new pending→写入→sync_all→校验实际字节→同目录rename到尚不存在的不可变generation→仅保留current与previous。不存在replace-existing行为，不依赖Windows stdrename覆盖已有文件。Unix可对目录sync，Windows目录刷盘未证明不宣称物理断电持久性。

启动最高generation异常时，不直接变空或静默fallback。返回RecoveryRequired和已校验旧generation，须用户明确恢复；恢复写成比已见最高版本更大的新generation，不覆写较新原件。只有损坏原件及备份都不可用时保留原文件并报错，基础输入可继续。

删除词条/clear标记purge_history，成功保存后删除所有本应用管理的旧generation和pending，避免删除记录留在自动备份。只清理固定命名文件，不递归删除根目录，不删除用户另存的外部导出。清理失败明确报告，不能宣称隐私删除完成。SSD物理擦除不保证。

保存before-publish故障保留旧文件和内存；after-publish但before-ack返回Unknown并要求重新加载，不能盲重试或宣称未保存。写入完成/显式恢复后再由主任务原子发布不可变组合快照。

## 计划API

`UserDictionary::new()`，`entries`，`entry(id)`，`add(text,pinyin)`，`update(id,text,pinyin)`，`remove(id)`，`set_enabled(id,bool)`，`clear_entries`，`set_learning_enabled`，`clear_learning`，`preference(id,fingerprint)`，`record_selection`；`to_bytes/from_bytes`，`import_entries`。

`Store::open(absolute_directory)`，`load`，`save(dictionary, expected_generation)`，`recover(generation)`，`clear_all`，`export(path)`/`import(path)`或以bytes交付宿主。错误仅固定类别/数字OS码，不泄漏词条/输入/路径。

## 可执行验证

先red测试真实新接口，然后实现并保存red/green。真实临时文件覆盖：不存在、空、损坏、备份损坏、未完成pending、旧/新版本、无效UTF8/拼音/重复ID/重复pair/容量、expected_generation冲突、两实例OS锁、导入冲突、删除重启、清统计重启。保存阶段注入真实写中断/StorageFull错误，不填满用户真实磁盘；进程中断不冒称物理断电。

已做工程可行性probe：Rust1.90 File::try_lock/unlock编译通过；MetadataExt::number_of_links在Windows1.90编译E0658，不能用std-only宣称硬链接完全门禁。需要时平台可采用已有Win32绑定，而不是往核心引入不明OS依赖。
