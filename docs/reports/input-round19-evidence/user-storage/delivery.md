# Round19 用户词库实现交付

实际工程没有修改。源码在`source/crates/cloudleaf-user`，仅Cargo.toml、src/lib.rs、src/store.rs及两个集成测试。父代理复制这五个文件，并按现有方式添加workspace成员及`cloudleaf-user.workspace = true`，由Cargo更新锁文件；不要复制临时独立workspace的Cargo.lock替换正式锁。

## 已实现

- 独立UserDictionary：增删改查、启用/禁用、一词多读音、稳定user:N、单调next_id；4096词条、1–16汉字、逐字对应合法规范拼音、最长64拼音字母/80字节；公开变更都在事务副本执行，出错不改原内存。
- UTF8/LF schema2 TSV包络+SHA256：包含generation/next_id/learning/learning_epoch/purge，entry revision/enabled/text/pinyin及独立偏好(id,fingerprint,count)。schema1实际迁移fixture，未来版本/CRLF/非法UTF8/重复/超量拒绝。文件8MiB、偏好10000、计数1000000。
- 学习默认OFF；Simulation拒绝，调用方必须核验真实Applied/会话/候选来源；相同token幂等、旧epoch拒绝、删改禁用后的旧user来源/指纹拒绝，CRUD/导入语义修改提升epoch。计数>=2才给log2偏好，最大8，不修改词频。
- Store显式绝对路径，拒绝Git目录、上级遍历、符号链接/reparse，OS File::try_lock持有期排他写入。`latest_generation()`在load损坏后也可读出最高代，`recovery_candidates()`列出可验证代。
- 保存临时create_new→写入→sync_all→重新读取校验→rename到未存在代→Unix目录同步→保留当前/前代。Windows不宣称目录持久性或物理断电保证。AfterPublish返回Unknown，要求reload核对，旧期重试Conflict。
- 损坏最新代显式RecoveryRequired，普通load/save不当空库覆盖。recover仅在最新代损坏时可回退到旧代，写更高代且保留所有原件；不能回滚一个更新有效代。clear_all明确删除全部的情况下可重置损坏档案为新profile，普通删词/清词不重用ID。
- 删除/编辑/清学习标记purge，成功保存清除应用管理备份/临时副本；若publish后中断，load完成清理，避免旧数据自动复活。用户显式外部导出不自动删除。明文，不宣称磁盘加密或SSD物理擦除。
- `estimated_bytes()`为Rust1.90容器/字符串容量保守估计，供父代理Host预算，非RSS硬上界。

## 验证

Rust1.90.0，16测试通过，0失败/忽略；fmt、clippy(all-targets locked/offline -D warnings)、doc均通过。包含真正子进程竞争锁与中断释放、保存各阶段注入、首存半写、重启恢复、删除后重启、schema1、future、坏备份、导入冲突和旧事件复活。实际StorageFull注入写入半文件后制造同类错误，不是把开发机磁盘填满；不是物理断电。

保留原始red：缺API编译、CRLF接受、recover删除原件、corrupt reset、删改后排队事件复活。所有命令/返回码/stdout/stderr/source SHA位于runs/*，汇总delivery-identity.json。

低层SelectionOrigin不是授权能力。宿主仍必须在实际确认owned EDIT写入后调用，benchmark/mock不能使用真实持久存储。recent token缓存256项且不持久化，宿主必须以新会话+commit唯一身份及原输入状态机保证迟到/重复Applied不再次进入；不能仅凭文本/普通CommitSucceeded学习。
