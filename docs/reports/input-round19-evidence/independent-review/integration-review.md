# Round19 独立只读接入审阅

2026-11-13。实际工作区：`C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf`。基线cf54c84，当前生产文件正由主任务修改，本记录是阅读时的中间状态，不是最终代码验收。未修改实际工程、未运行UI，未把本审阅当人工语言质量审核。

## 已核对的共享层

`cloudleaf-search/src/layer.rs` 的 Entries/Trie 通过Arc共享基础entry/node数组；用户尾段保持基础条目索引不变，修改的基础Trie节点采用clone-on-write，新的音节边仍进入原格构建与原搜索。`with_user_layer`只允许未分层基础快照，拒绝外部来源/非unknown频率/冲突ID及超过8的偏好输入。未发现该实现中可复现的词条丢失或错误索引引用。新集合不再保证连续slice/全局ID排序；当前唯一上下文binary_search已改为显式ID查找，阅读的其余索引调用仍正确。

所有默认Trie索引多了一次空changed映射检查、entries多一层shared集合分支，因此默认关闭仍需同条件性能比较，不能从逻辑等价推断零开销。内存估计是固定Rust布局的保守估计，不是RSS/allocator硬上界。

## 需要在接入完成时关闭的具体问题

1. **P1：新快照不能绕过原Host容量账本。** `adapters/windows/session.rs` 的recovery_budget和epoch_cost仅在构造时计算；`replace_search_snapshot/resources`本身不会更新该账本。必须为新用户delta、偏好向量、重绑字符缓存、worker用户字典、结果/草稿与新增选择凭据计费，包含同时存在的旧/新代。不能在释放旧资源前只计新代，也不能把Arc共享基础重复算成两个完整118.5MB词典。
2. **P1：删除后排队学习不能恢复已删除统计。** 阅读的temp UserDictionary在remove/update/clear_entries后没有改变learning_epoch，而record_selection只校验ID/指纹形状，不要求当前user:id存在。可重放序列：学习ON→保存SelectionEvent→删除词条→送达旧事件，可能重新创建该用户ID的偏好。已通知存储方向增加语义变更epoch/指纹校验及回归；主worker仍应核对selection绑定的快照版本。
3. **P1行为缺口：明确重试要延续真实来源。** 当前InputEngine.begin_commit清空pending_selection，retry_commit只复制失败text，导致NotApplied→显式重试→Applied丢失真实候选凭据。应保留失败选择元数据，重试时仅换commit attempt ID，不从text反推；Unknown始终不可重试。已通知主任务。
4. **热路径成本：不要每个词条线性扫描基础词典。** 当前begin_candidate_commit对path每条边调用entry_fingerprint(id)，其实现对123544条线性find。EdgeSource已保存entry_index，可直接读取真实entry进行指纹计算，或者在候选诊断构建时携带指纹。没有必要在Space路径做最多64次全词典扫描。

## 最小发布/预算流程

- 一个worker持有Store/字典状态、base快照/原模型，命令与结果队列都固定容量。只在worker读写磁盘和构建用户delta/模型重绑缓存；结果没有HWND、RcHost或COM。
- PreparedUserState携带操作序号、基础SHA、用户逻辑版本/学习epoch、Arc组合snapshot/reboundlanguage、容量数据及独立delta reservation。pending结果最多一份，不连续堆积完整代。
- STA发布前检查Host owner/epoch、quarantined=false、两引擎没有pending/failed提交、两边revision可增加、两边资源配置/模型绑定通过。只在所有预检通过后同时替换；预编辑/光标保留，revision增加，旧ID失效。
- Host基础reservation保持基础数据计费；新Prepared/active状态的reservation仅计共享基础以外delta和在途内存。旧Arc引用存在时对应delta reservation仍存活，直到实际释放。新选择凭据和管理列表/结果缓冲也包含在容量里。超过128MiB明确拒绝并保持旧可用快照。
- 存储保存与候选发布结果分别报告。如果保存成功但事务未决阻止发布，明确显示“已保存/待发布”，不能谎称当前候选已更新。保存结果Unknown需要reload核对，禁止自动重试文档提交。

## 可信学习核对点

候选选择发生前的revision与实际path产生SelectionEvidence；generic CommitSucceeded不保存统计。只有Window.sink写owned EDIT并按实际UTF-16核对Applied之后，才生成IsolatedOwnedWindow事件。windowless benchmark和mock不得使用该入口。事件绑定Host instance/epoch/commit attempt、snapshot/model/scheme及用户学习epoch；队列满只报告未保存学习，已Applied文字不重放、不回滚。

NotApplied、Unknown、取消、原文回退、旧revision/旧session和重复Applied都不能增加统计；显式重试的已知未写事务需要新attempt身份。学习OFF/clear和用户删除/修改都会使在途旧学习失效。学习只存ID/指纹/计数，普通日志不打印词条或整句。

## 窗口面板检查

temp `r19-host-review/user_panel.rs` 为同一Host的owned顶层管理窗口，中文EDIT没有继承测试主窗口的IMM解绑；工作回调无文件操作。Panel有HWND-owned Rc lease，NCDESTROY释放；主Window回调应捕获Weak，避免Window→Panel→callback→Window循环。关闭管理窗口只hide，主窗口退出时应明确close/destroy并停止worker，不能留下永久线程。

面板编译通过只是工具实现证据，不能替代实际添加/编辑/保存/reload/import/export/学习/删除及重启验证。真实词条数据和导出文件不得进入Git或CI产物。
