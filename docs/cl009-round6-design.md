# CL-009 Round 6：可执行标注、源断言图与统计语料准入

2026-10-28。起点`c21db7ca70e5f51f84ca8bd30944d6722473c32c`。不修改schema2/3、TSV v2、生产音节/导入/发布/恢复接口，不接默认引擎；R1–R5、123544正式固定词条包及B冻结输入不变。先提交本设计，再提交真实红测，最后实现独立离线工具。

## 实际问题及范围

R5人工gold只有blocked计划，无法录入任务、绑定双审结果、检查全局泄漏或冻结数据；已有Wiktionary固定研究原件保留POS/义项/翻译却没有独立源断言图；Wikipedia只有dump元信息，不能跳过页级例外和分词直接补频率。本轮分别实现`annotation_workflow.py`、`source_assertions.py`、`corpus_gate.py`及真实回归、实际待填任务包、准入和源图报告。代码、注释和文件为UTF-8，不新增第三方依赖。

## 实际人工任务协议

任务必须含`input_intent/pinyin/acceptable_outputs/key_trace/scenario`，来源原件/授权、作者、全局`lineage_id/family_id/near_duplicate_group`和规范`source_unit_id`。待填任务明确为`pending`，文本/答案/审核人/授权空，不自动生成拼音或正确答案，不从待测词典构造gold。工具可生成实际工作队列和说明，但空队列或虚构fixture不能成为正式评估材料。

完成任务需作者授权SHA、两名不同且不是作者的reviewer，审核结果绑定规范化annotation SHA；修改意图、拼音、答案或按键后原审核立即失效。有分歧必须独立于作者/reviewer的仲裁者明确决议。来源/授权/审核信息只是文件声明，不能自动证明真人身份或权限真实性；实际人员由用户指定，不联系外部人员。

审核/仲裁还须逐项绑定同一annotation protocol SHA；替换包级协议不能继续使用旧审核。许可/提取/分词准入的证据必须是实际文件path/SHA及被核对的结构化内容，不能只填写四个64位字符串就批准统计。提取协议固定实现/配置，分词协议固定实现/配置/单位；权利声明必须绑定具体revision/正文且包含作者历史与页面例外审阅。仍不自动证明声明真实。

冻结前核验所有完成记录和来源、全局family/near组、lineage+unit及规范化输入/输出近似键不得跨train/dev/heldout；不同镜像ID的相同artifact必须归入同lineage。源单元/分组的语义真实性仍需人工审计，不能靠程序相似键证明。split由独立保管者明确分配，不静默随机重分；一旦输出目录存在拒绝覆盖。冻结manifest绑定全部输入、协议、工具和输出SHA，公开索引不含heldout答案，heldout答案独立文件由保管者在受控目录运行保存；文件分开不是加密或访问控制。

本协议不降低B原70/15/15家族与heldout>=2000家族等正式前置门禁。C冻结文件只是显式交付材料，是否满足B由旧门禁及新增preparation继续判断；正式质量指标始终由独立评估计算，本工具不计算Top-1/Top-5/MRR。实际12个待填任务保持blocked。测试fixture显式synthetic，不能混入正式任务。

## 真正源事实的研究图

只消费已批准研究范围的固定原件`.local/data-sources/zhwiktionary-prefix-8388608.jsonl`：46788375字节，SHA`ca4df3453261f9f12f1223deb11a26d979078ea6e1af870b44da2d3dcf4e2998`，66593完整行；锁`data/sources/locks/wiktionary-zh-20261001-prefix8388608.json`。CC BY-SA4、研究片段、源顺序偏差、全gzip未完成及逐页例外未全审的限制全部传递，不升级为正式完整双语库。

保留中文词头原POS、义项gloss字段和源显式英语翻译，每条断言绑定source_id、version、原行SHA、1-based行号及原字段JSON pointer；稳定ID只表示该原件位置，不声称跨修订语义稳定。媒体/例句/引文不输出，额外权利标记隔离。翻译的原`sense`标签不自动解析为已对齐的义项ID，保留unresolved；不将斜线或“词义二”猜成语言事实。

实际范围继续遵守既有锁的transformations：2–16字Han词头且含明确Mandarin/Pinyin字段，非此范围只计跳过，不因研究图而扩展到所有中文行。已知1328中文行只是原件统计，不是新图接收数。构建同时核许可原文SHA并无损封装；报告列实际接收、隔离、范围外与缺读音分母。

中文→英语和英语→中文查询索引引用同一条有向源翻译断言，不新造reverse-equivalent断言；英语反查只是检索入口。所有频率unknown/null。输出保留许可/署名/修改说明、原件及工具SHA；双构建逐字节相同。该源图不能直接满足R5需要真人双审的生产sidecar或人工gold。

## Wikipedia条件候选的实际准入

只对预选官方API页面取得原件，不下载3.4GBdump或换未经审核镜像。已取得`自然数`page342/revision94816134，timestamp2026-10-07T07:31:16Z，API revision SHA1`a8aa38891980a843b77d0715fc69d90d23e520e2`。固定原件需核响应SHA256、完整revision content SHA1/内容模型/命名空间/时间、attribution与history URL，条款版本另绑定SHA；revision末编辑者不等于全部作者，history链接是署名链。

原文含模板、ISO/教材引用、媒体和quote参数；未完成页例外审核、可复现纯文本提取与分词协议，因此原文仅留`.local`，Git只保存不含正文的身份/门禁报告。`corpus_gate`必须分别拒绝缺rights review、未审例外、缺提取/分词版本/配置；不能把“没扫到版权关键词”判为已授权。未满足门禁，token/DF/Zipf均null，不对源码wikitext伪计词频。即使准入结构通过，也不证明百科语料代表日常输入。

## 检查与退出

新增失败测试覆盖：审核后改答案、作者充当审核、分歧缺仲裁、缺授权、同artifact换ID跨split、全局near/family/lineage单元泄漏、空/未完成任务冻结、覆盖冻结输出；源断言定位/未对齐翻译/反查不造反向事实/权利隔离/哈希改变；Wiki revision/hash不一致及rights/tokenization缺口。测试调用真实工具，不复制业务算法。

冻结实现后运行README全Rust/Python、fmt/Clippy/doc/资源审计和本轮专项。源图两次完整构建/查询与任务生成/拒绝/fixture冻结的全部原文和身份保留；原正式包及历史Git对象只读核对。大型构建/恢复/性能测量与A/B错开。新结论记录精确commit、原件/工具/二进制/输出SHA；未冻结草稿不能伪造精确旧实现。本轮不宣称正式语言质量、不合main、不进入CL-010。
