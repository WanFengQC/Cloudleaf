# Round20 Windows 隔离输入窗口人工验收

**当前全部步骤not-run/user-paused。** 本文件只根据固定Round19源码准备人工步骤；Round20若改变控件/行为，执行前按最终源码和回执重新核对。它不恢复桌面自动化，也不批准TSF、安装、默认输入法更改、8h/24h或私人数据采集。

## 执行前由用户准备

- 选择自己的空闲桌面，关闭其他Cloudleaf测试实例；仅使用原创非隐私测试词。记录本次源码HEAD、EXE SHA、词典/模型SHA、Windows版本/架构、时间和执行者，不编造已执行记录。
- 使用回执中经过验证的`cloudleaf-ime.exe`。它的`--interactive-input-host`是普通自有文档窗口，不是系统输入法；不能省略参数后尝试安装入口。
- 设置新的ASCII研发档案，例如`Round20-Manual-A`（仅字母数字与连字符，1–64字符），避免读取原私人档案或R19证据。实际目录由当前用户LocalAppData解析为`Cloudleaf/Research/Round20-Manual-A/user-dictionary`。文件明文，只在本地；不要把整个目录、词条文件或导出文件上传Git/CI。
- 固定原词典为123544条，SHA `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`；通过已有verify_input_lexicon校验其完整release。模型参数可省略做结构模式；若使用研发模型，填其实际SHA并将其标research_only，不能把两种条件混作同次验收。

下面仅为**人工执行模板，Codex本轮未运行**；EXE实际路径须从最终回执填写，不能把占位路径当作已存在产物：

```powershell
$env:CLOUDLEAF_USER_TEST_PROFILE = 'Round20-Manual-A'
$Exe = '填写最终回执中的已验证cloudleaf-ime.exe绝对路径'
$Runtime = 'C:\Users\EDY\.codex\worktrees\cl009-data\Cloudleaf\.local\round5-clean-6d3d4e1\release-a\runtime.tsv'
$RuntimeSha = '03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
& $Exe --interactive-input-host $Runtime $RuntimeSha
```

退出后可在同一个PowerShell移除本次进程环境变量；不删除档案、不修改全局环境。下一次用完全相同档案验证重启持久化。不要通过`SendKeys`、UIA脚本或Computer Use代替本文件人工操作。

## 操作、预期结果与留证

每一行结果均待用户填写。失败时停止该链路，不用后续成功覆盖失败；只保存自有测试文字的截图/状态、时间、版本、错误类型/阶段和非敏感资源身份。不要录制其他窗口、密钥、私人词条、聊天或完整输入历史。候选显示本身不能独立证明entry来源；来源由同版本headless诊断及候选路径证据交叉核验，不能靠文字相同反猜。

| 步骤 | 人工操作 | 应观察到的结果 | 当前结果 |
|---|---|---|---|
| 1 | 用上述显式参数启动，点击宿主空白处 | 标题为隔离中文输入宿主（非TSF）；出现当前方案、状态、自有文档；无系统注册/激活 | not-run/user-paused |
| 2 | 点击“用户词库 / 本地学习” | 独立管理窗、正常中文EDIT；新档案学习关、generation0、0词；加载失败必须显示错误，不能静默空库覆盖 | not-run/user-paused |
| 3 | 填文字“云笺”、拼音“yun jian”，点击“添加” | 列表出现独立user ID、revision1、启用、选择0；generation递增，操作结果明确；原基础词条未改 | not-run/user-paused |
| 4 | 点击“保存”；等待任务完成，再“重新加载” | 保存/加载成功，内容和ID仍在；后台忙时明确拒绝多次任务，不阻塞主宿主按键 | not-run/user-paused |
| 5 | 回到宿主空白区，逐键输入yunjian | 全拼候选出现云笺；输入原文和候选保留；未选词时计数不变 | not-run/user-paused |
| 6 | Space/数字/鼠标分别选择，每次先输入并确认焦点 | 只有本窗口自有文档上屏一次；学习关闭时选择计数不增加；不得向其他应用写字。遇焦点切换先停止，检查原文恢复区 | not-run/user-paused |
| 7 | F6切到全拼/简拼混合，输入yj | 同一搜索/候选窗口可找到用户词；切换方案保留或明确恢复未提交原文，不接受旧候选 | not-run/user-paused |
| 8 | 取消组合，打开管理窗，明确点击“开启 / 关闭学习”开启 | 明确显示学习开、generation变化；这一步是用户实际选择，不能把默认状态记为开启 | not-run/user-paused |
| 9 | 回主窗输入并选择云笺两次，每次等待后台完成再检查列表 | 自有文档真实上屏，选择计数各增加1；首次计数1无学习优势，计数2才有有界学习单位。后台忙拒绝须记录为未保存，不能为补计数重放文档 | not-run/user-paused |
| 10 | 同音候选存在时显式选择非首项，观察后续排序并保留全路径诊断 | 评分规则在同解释成本下生效；不能保证指定词永远首位，不把个人偏好说成公开词频或语言质量 | not-run/user-paused |
| 11 | 关闭主宿主，确认程序结束；再用同一档案启动并打开管理窗 | 管理窗/worker随主宿主退出；同一档案能重新获得文件锁；词条ID、计数、学习状态从磁盘恢复，无重复学习 | not-run/user-paused |
| 12 | 关闭学习，再选择同一候选 | 状态明确关；文档仍正常提交一次，计数不增加；手工词条不删除 | not-run/user-paused |
| 13 | 点击“清空学习统计”，再重启同档案 | 计数归零，手工词条/ID保留；排序回到同手工词库、无学习时规则；旧备份不复活统计 | not-run/user-paused |
| 14 | 填原ID，改文字/拼音，点击“修改”；随后启停 | ID保留、revision提高；新候选按新内容更新，停用后不参与新候选；修改前的旧revision/candidate必须拒绝 | not-run/user-paused |
| 15 | 填原ID，点击“删除词条”，等待并重启 | 新用户层不再引用该user ID/旧偏好。基础词典本来包含同文字时仍可出现基础候选，不能仅凭“云笺还可见”断言删除失败 | not-run/user-paused |
| 16 | 另建词条，在“文件路径”填自己选定的全新绝对导出文件，点击“导出” | create_new成功；再次导出同路径明确拒绝、不覆盖。自行导出文件是明文私有数据，不上传 | not-run/user-paused |
| 17 | 用自己导出的同版本文件执行“导入”；重复及冲突分开验证 | 相同记录可无变化；同ID异内容/同读音对异ID冲突拒绝、不半导入；导入不替用户开启学习 | not-run/user-paused |
| 18 | 仅在另一个一次性测试档案准备坏文件/旧有效代，重新加载后选明确“恢复版本” | 最新坏代报RecoveryRequired并列候选；不当空库覆盖；显式恢复写更高代且保留证据，不能覆盖更高有效代。不要破坏主人工档案做此测试 | not-run/user-paused |
| 19 | 输入未提交拼音，切到外部自有测试空白窗再返回 | 当前组合取消/恢复区保留未确认原文，候选关闭；不自动向其他应用上屏。只留本宿主状态，不采集外部窗口内容 | not-run/user-paused |
| 20 | 回宿主输入候选；点击“下一次提交注入Unknown”，再选词 | 注入发生在本窗口写后，文档可能已经有字；状态隔离，不继续新输入或自动重放；此次不学习 | not-run/user-paused |
| 21 | 人工核对文档与原文后，点击“核对后开始新会话” | 接受当前自有文档开始新epoch；旧Unknown仅失效，不重试、不补学；旧候选点击/迟到确认不得污染新输入 | not-run/user-paused |
| 22 | 关闭主宿主；确认用户管理窗随之结束，重启同档案 | worker真实结束、文件锁释放；不会遗留第二写实例Busy。仅关闭管理窗可能隐藏窗口但worker仍由主宿主持有，不能误记为进程退出 | not-run/user-paused |

**NotApplied没有现成窗口注入按钮。** 不编造“点击NotApplied”。它当前由无HWND控制器回归`not_applied_keeps_original_and_a_later_explicit_selection_can_commit_once`与`selection_proof_is_returned_only_after_applied_and_retry_has_new_identity`覆盖；构造返回值不能算真实窗口NotApplied验收。未来如增加人工故障入口，须先设计并给出明确显示/三态检查，不能借故修改系统安全设置。Unknown按钮也只是隔离窗口注入，不是TSF宿主写入不确定性的系统验收。

## 退出与判定

按同一EXE/数据/档案版本保存每行实际结果、失败原文和重试的新记录；严禁将未执行填passed。R19只证明过添加/保存/学习开关与全拼/简拼候选部分观察，不能预填本表通过。只有真实运行后才能判断窗口操作链路；本轮headless测试或GitHub CI成功不改变本表not-run/user-paused。

用户词库的逻辑测试、物理断电、8h/24h、其他Windows应用、TSF注册/安装/默认输入法及翻译面板均是不同验收范围。本表完成也不自动放行那些范围或CL-010。
