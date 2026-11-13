# Round19 隔离宿主与用户词库接入审阅

实际工作区：`C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf`；基线`cf54c84d400b627dfa9b246b3800a82d65f061b8`。只读审阅，不修改实际仓库、运行UI、测试或提交。规则以AGENTS与Round19完整附件为准。

## 已有精确钩子

- `adapters/windows/isolated_input.rs::LocalHost::process`先通过实际引擎/投影处理，调用owned EDIT sink，Applied分支立即给两引擎发送`CommitSucceeded`。
- `InputEngine::acknowledge_commit`成功后调用`clear_context`，candidate/path/preedit全部丢弃。必须在这一步前关联选择来源；不能只用commit.text猜条目。
- `select_candidate`已经核revision与candidateID；数字键、Space/Enter目前直接将文本交`begin_commit`。改成内部`begin_selection(candidateID)`/`begin_commit(text, selection:Option<...>)`，只让明确候选选择分支填写必要证据。标点隐式确认、无候选Enter原文回退、未知字符拼接均不学习。
- `reconfigure_search`/`replace_search_snapshot`/方案切换已先拒绝pending、failed；用户发布需同样先预检两引擎，保留原文和光标，刷新revision并使旧candidateID失效。Unknown先由Host拒绝。
- `input_review::load_host_k`是显式研发加载入口，原文件/模型载入已在普通窗口出现前执行。新增用户worker不能进入默认TSF/CLI路径。

## 最小学习证据契约

建议私有 `PendingSelection` 在begin_commit分配ID时形成，包含commitID、候选revision、candidateID、词典/用户层/模型/方案版本、当前搜索路径stable entry IDs。只保留必要ID/版本与计数，禁止保存原文整句、应用/窗口标题。

核心可提供只读 `pending_selection(commit_id)`，但generic `handle(CommitSucceeded)`绝不触发学习，也不自行创建“真实成功”。宿主在执行真实sink前捕获证据，Applied并且epoch仍同一、用户learning开关已开时才消费一次。证据用`take`或一次性票据消除重复确认；NotApplied保留事务但不学习，Unknown/取消/失焦/换epoch清除学习资格。

必须明确确认来源：`IsolatedOwnedWindow`、未来`WindowsTsf`、`Mock`。当前真实Win32 `Window::sink`通过`SetWindowTextW`及回读确认后可形成隔离Applied；`benchmark`当前`|_| Applied`以及所有测试closure是Mock，不得调用持久学习。来源参数应由Windows入口固定，不能从普通`CommitSucceeded`或外来数据自称可信。

`Applied`返回后，提交ack和学习任务入队各自独立：队列满不能撤销已经成功写入的文档，也不能自动重试提交。学习队列满明确显示学习未保存；有界同ID统计批次可合并，不能静默假装已落盘。关学习状态变化必须让旧排队学习任务失效（配置generation/worker顺序关闸），清统计不能被旧任务重新填回。

## 单worker及所有权

一条长生命周期worker拥有UserStore、未发布草稿和索引构建；`sync_channel`有界命令队列+有界结果队列。命令带递增operation ID、预期持久revision、用户配置generation。UI只`try_send`，满时返回繁忙，不阻塞按键；`WM_TIMER`在所属STA用`try_recv`取结果，无需跨线程传Rc<Host>、HWND裸指针或COM资源。

worker接收Arc只读基础快照/模型材料，执行校验、迁移、保存/导入/导出、组合词典构建与必要模型重绑定。结果是预构建不可变`PreparedUserSnapshot {operation_id,baseSHA,userGeneration,snapshot,model,cost}`。主线程检查结果仍是当前请求，先预检两个引擎/128MiB预算，再一次发布；事务忙时仅保留一个有界待发布结果或明确拒绝，绝不强行清空pending。老snapshot由Arc自然保持至无引用。

不要在WM_CHAR、WM_KEYDOWN、选词提交路径读取文件、保存、生成TSV或重建123544条索引。管理按钮WM_COMMAND也只入队，避免UI长卡。关闭时先拒绝新管理请求并显示未完成保存状态，worker的退出/JoinHandle回收必须有已完成证明；不在STA无限join。

## 管理入口与实际中文编辑

复用同一Host，新增明确按钮/F9打开owned工具窗口（不是另一套输入法）。列表显示ID、文本、拼音、启用、修订，EDIT用于文本/拼音，命令有添加/更新/启停/删除/清空、保存/重载、导入/导出、学习开关/清统计及状态。每个提交命令固定当前选择的stable ID/revision，不能用列表行号当持久身份。

现有隔离主窗口在WM_CREATE把自己和全部现存子控件的IMM上下文解除，目的是避免系统IME截获拼音测试。管理词条文本需要正常中文输入：可用单独owned manager HWND保留其系统IME（同一Host），或者在主窗口IMM解绑之后创建中文管理EDIT，明确其不参与核心拼音WM_CHAR。不要不经验证把中文输入框也禁用IME。用户显式打开后才激活管理器，异步完成不抢焦点；返回测试区由用户点击/明确按钮聚焦。开启管理器触发原失焦保全，不自动提交组合。

## Windows标准目录与替换语义

固定`windows=0.62.2`已有COM绑定，增加`Win32_UI_Shell`可调用`SHGetKnownFolderPath(&FOLDERID_LocalAppData, flags0,None)`；NULL token表示当前用户。取出的PWSTR转PathBuf后用既有`CoTaskMemFree`释放，追加固定`Cloudleaf/UserDictionary`子路径，不接受未校验相对路径写入store。启动/worker初始化时解析，不在按键路径使用。不要硬编码C盘用户名或把`LOCALAPPDATA`环境字符串当成唯一经过平台核实的路径。

保存同目录临时文件→完整校验→`File::sync_all`→替换。Windows可用`ReplaceFileW`带应用管理的备份处理已有主文件；首次创建单独处理。`REPLACEFILE_WRITE_THROUGH`官方明确不支持，不能作为刷盘保证。失败1176/1177存在名称/备份状态变化，不可一概宣称“失败即旧主文件完整不变”；保留所有候选并验证版本/hash，回报错误，重启恢复按证据选择，不能自动把较旧备份覆盖新有效主文件。多实例需要持久revision冲突检查+真实独占锁（失败后拒绝写），不能只靠进程内队列。

官方原始资料：

- https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath （当前用户token及CoTaskMemFree责任）
- https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid （FOLDERID_LocalAppData是PERUSER）
- https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew （替换/备份及失败状态；WRITE_THROUGH不支持）
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers （刷指定文件，不等于物理断电验收）

## 仍需真实执行的验收

实际中文CRUD→候选含新词→选词→关/开学习→持久计数生效→重启加载→删除后重启不再候选；双实例冲突、损坏/满盘/写中断恢复、旧候选与Unknown拒绝发布。以上本审阅未执行，不能以接口或建议标done。
