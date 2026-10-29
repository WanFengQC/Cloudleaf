# Round 9：单人作者队列与正式 gold 分离

2026-10-30。单人维护者可以在私有目录原创、标注和改错；四个worktree和Codex子任务都不代表独立真人。R8已经交付正式三门禁和静态交接表，本轮只补实际可操作的作者队列，不重做算法实验。

## 文件及边界

新增`scripts/manage_ime_annotation_queue.py`、`tests/test_annotation_queue.py`、操作README及R9方向报告。`config/evaluation/language-readiness-v1.json`中历史`cloudleaf-development-team`只是占位，改为null，由用户实际登记评估身份；不改变正式协议、70/15/15、heldout至少2000家族、双审/仲裁或独立保管要求。C的13字段摘要和三协议摘要、正式B工具和Rust引擎均不改。

`export`显式接收C作者源packet路径和原字节SHA，只接受schema1/原format、非合成/非词典派生、所有任务split=null的未分配作者材料。不得传入已冻结或含heldout正文的包。输出新的B私有`internal-development-only`外层，保留原packet和原任务ID清单、来源SHA，附空`authorization_files`映射；不填人名、答案、审核、批准或split。不覆盖已有文件，不从词典或AI生成内容。

维护者在导出的私有文件中填写自己的真实作者声明/原创任务和实际来源；新任务可以增加，原任务槽丢失在入口拒绝，尚未填写者继续pending。`authorization_files`只给packet已有source ID添加`{path,sha256}`，相对路径以队列文件为准；核实际文件字节、C source.authorization原SHA及evaluation/modification/redistribution范围声明一致性，不把读到文件当真实签署或来源批准。

## 三层进度

1. 个人标注字段完整：意图、拼音、可接受输出、按键轨迹、场景及来源/作者等字段具备；只代表内部草稿完整，不依赖“completed”自报状态放行。
2. 审核声明与当前内容匹配：复用B既有annotation_digest/review_binding_digest；三份协议SHA必须非空且格式有效，两个reviewer ID不同且均非作者，绑定当前任务/来源及三协议；有reject则还要独立仲裁声明。来源或协议变更使旧声明失效。不同字符串和文件一致性不证明实际是不同真人或已经签署。
3. 正式gold：本工具始终`formal_ready=false`、Top-1/Top-5/MRR=null，不创建或读取heldout文件，不执行正式划分/冻结/搜索/打分。真实授权、独立双审/仲裁、保管/加密及正式三个门禁仍由原流程审查。

`status`报告只输出ID、摘要、计数与缺件，不输出意图、拼音、答案或完整任务。有效输入成功生成报告exit0，**exit0不是正式就绪**；格式错误、源SHA不符、split已分配或输出存在则exit2。export同样只表示生成草稿成功。

## 必要反例与验收

先提交设计及失败用例：旧团队占位不为null、新API尚不存在。实现后测试真实导出/报告入口，覆盖空12槽、个人字段完整仍非gold、重复/自审声明、空协议、来源授权文件缺失或SHA变动、变更后旧审核失效、分歧仲裁、同人保管、已分配/heldout拒绝、输出不覆盖和报告不泄漏答案。机械fixture只验证代码，不代表真实许可/人员/gold。

完成后运行用户要求全部Rust locked/offline检查、release、workspace审计和全部Python测试，保留失败原文和原始字节。R9报告绑定精确代码/工具/输入/二进制身份；对应Draft普通推送并另核新head Windows/Linux push CI。人员和授权未到位继续blocked，默认引擎不替换，不进入CL-010。
