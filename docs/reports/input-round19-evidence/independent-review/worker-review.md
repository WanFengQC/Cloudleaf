# Round19 worker/UI 接入复核

2026-11-13。只读中间实现：实际`adapters/windows/user_worker.rs`和`isolated_input.rs`。主任务正在修改，以下是具体流程核对点，不是最终缺陷清单。未执行UI、未改实际仓库。

## 确认合理的部分

- 单worker+容量8命令队列、容量2结果队列，UI用try_send/try_recv。worker不捕获RcHost、COM或HWND，文件/索引构建均在worker。
- LocalHost仅在sink返回Applied时提取pending_selection，generic CommitSucceeded及原文回退不发学习；Window层另外要求当前active学习ON、快照身份一致。windowless benchmark不走Window学习分支。
- 组合发布在STA先对两个实际引擎做资源预检，再替换。没有磁盘等待；两个引擎保留同一Arc快照和模型，旧用户Prepared保留reservation直到切换释放。
- prepare预留用户delta后才构建Trie/模型，未提高原128MiB。管理命令先生成preview，保存失败不替换内存字典或当前候选。
- 学习OFF并不必清除既存偏好；当前保留旧偏好而停止新增统计，符合“关闭不删除”的范围。清除统计另操作。

## 具体需修正/补足

1. 初次open_users调用Panel::open之后未show；Panel样式无WS_VISIBLE，第一下管理按钮不会显示面板，第二下才走existing-users.show。
2. 用户层学习开启但busy时，Applied选择被无提示跳过。应明确显示未记录原因，或使用有界队列；不能无提示丢学习。文档提交不能重试。
3. pending发布被Unknown/未决事务挡住时面板仍显示“后台处理中”；应区分已保存/待发布并显示恢复条件。
4. 初始Store.load、reload/import/recovery的读取/解码、current.clone、dictionary.to_bytes在相应额度申请之前进行。当前prepare的bytes*8不能自动为此前和持久worker字典背书。需要独立worker数据额度及预读元数据准入。
5. 新SelectionEvidence是两引擎的持久状态，当前原Host固定容量按frame/diagnostic计费，没有把其entryID/fingerprint向量、失败重试凭据副本算入。
6. Work::Learn只有用户generation/learningepoch，没有活Host epoch取消令牌；LocalHost.reset使旧context失效后，排队旧事件仍可写统计。主线程可拥有Arc<AtomicU64>取消代，worker在实际更新前比较捕获代；不跨线程传Host。
7. preview在保存前生成listing的generation，再只更新Prepared.generation，UI显示保存前代号。修复元数据而非重新构建整个索引。
8. Snapshot目前内容hash只绑定base+启用TSV+偏好units；相同units的计数变动/编辑revision不改变hash。旧candidate仍受revision保护，但不能报告snapshot绑定了未参与哈希的用户generation。应增加显式用户逻辑身份，或准确限定内容hash并单独校验generation。

## 最小容量处理建议

在IO边界只读最新固定generation/import文件的metadata大小，先拒绝超过codec MAX_BYTES，再按有限解析/副本的保守成本申请worker数据额度；未获额度不读全文件。读取后核对UserDictionary::estimated_bytes、临时序列化/草稿副本和结果字符串。额度必须在字典存活期间持有，而不是仅等到Prepared被UI释放。

prepare额外计组合索引incremental bytes、偏好向量、重绑字符缓存、列表和临时TSV，并保留旧新代同时引用的费用。不要把共享基础再次计成118.5MB，也不要先drop旧token再争抢新token造成未覆盖窗口。基于大记录的预读准入可拒绝本次用户操作，基础输入继续可用；需在报告中列实际可接纳容量，不能宣称所有4096记录在所有模型下均可发布。

本审阅未引入新的安全框架或跨线程COM操作。
