# CL-009 Round 6：人工任务工作流、源断言与统计准入

2026-10-28。分支`codex/cl009-data`；固定起点`c21db7ca70e5f51f84ca8bd30944d6722473c32c`，被测实现**`cdc73515ad5356844f1df4fc3ee30194c5a15ab6`**。后续方向提交只整理报告，不将测量改绑到报告head。完成了可执行离线工具及已捕获缺陷修复；正式词频、真人gold和正式语义发布仍阻塞，不合main、不进入CL-010。

## 实际审阅与修改范围

已读取AGENTS、roadmap、product-scope、Round 5共享/方向报告及实际数据代码。审阅旧`linguistic_sidecar`的token/DF/Zipf、正数下溢/unknown、母观测身份，以及`pipeline.verify_release`、发布/回退、`recovery`的guard/PID/start与未知状态拒绝边界，未发现本轮可独立复现的新P0。没有因计划任务数而改动这些既有生产代码或预算。

先行设计`050abe0`，初始缺模块测试`02176c0`，批准范围补充`51f0fe6`，协议/真实证据反例`6b7b02b`、来源绑定/heldout副本反例`d17b487`先于最终生产工具`cdc7351`。全部旧回归、资源及R1–R5报告保留。

|文件|目的|
|---|---|
|`tools/data_pipeline/annotation_workflow.py`|实际待填队列、意图/拼音/答案/键轨迹、来源授权绑定、双审/仲裁、全局split隔离和不可覆盖冻结|
|`tools/data_pipeline/source_assertions.py`|固定批准研究片段的源POS/义项/有向翻译断言与双向lookup，不自动补词频或反向语义|
|`tools/data_pipeline/corpus_gate.py`|核固定官方revision完整正文身份及实际权利/作者历史/提取/分词证据文件，未齐时拒绝统计|
|`tests/test_annotation_workflow.py`、`test_source_assertions.py`、`test_corpus_gate.py`|14项实际工具回归，覆盖声明变更、泄漏、重写、源定位/反查和证据假批准|
|`data/plans/ime-annotation-round6.json`|工具生成的12项实际待填任务，无虚构答案、许可或人员|
|`docs/cl009-round6-design.md`、`cl009-round6-annotation-handoff.md`、管线README和本报告|设计先行、显式B映射、命令/身份和未完成边界|

默认输入核心、123,544条runtime、1,468条隔离、生产音节/导入/发布/恢复格式均未改；独立研究图不替换正式包。已有source锁的批准范围不能因新增工具而扩大。

## 已捕获缺陷、失败原文与回归

[原始文件无损封装](cl009/round6/raw-artifacts.json)逐项保存UTF-8原文、字节数和SHA，按`utf8.encode('utf-8')`恢复原字节；发布展示件另有SHA，规范LF不冒充原字节。每个red的命令/返回码、测试提交及已保存草稿完整字节位于`red/`。

- **初始缺功能**：`02176c0`的三个测试模块在旧树真实缺模块失败，各exit1，不能把这些错误冒充下面具体断言的复现。
- **P1，审核协议可置换**：`6b7b02b`测试在当时工具草稿上实际捕获：替换包级protocol仍沿用旧approve。现review/仲裁逐项核同protocol SHA；旧审核不再有效。
- **P1，统计门禁可由虚构摘要放行**：同测试提交捕获仅填四个64位字符串就可使`statistical_use_ready=true`。现必须读取实际证据文件并核SHA、固定revision/content权利声明、作者历史、提取/分词实现/配置/单位；合规真实性仍需独立审阅，程序不替真人证明授权。
- **P1，来源原件/授权可置换**：`d17b487`真实行为red中，同source_id更换artifact仍ready。现task的`source_binding_sha256`固定完整source记录（含artifact/version/authorization），纳入annotation摘要，旧审核不能沿用。
- **P1，公共packet复制heldout答案**：同一真实red捕获`packet.json`无意保存第二份heldout答案。现公共metadata只含来源/协议/task IDs，答案只在独立split文件。回归检查全部其他文件均不包含heldout夹具答案；分文件仍不是加密/访问控制。

后四项来自**未提交草稿**，不是某个旧已提交生产版本；草稿`.py`完整字节及SHA已保存于`round6-red-bound`和`round6-red-source-behavior`，测试commit分别可追溯。第一次source-binding测试因辅助API尚缺失的准备失败另留`round6-red-source`，不混作行为缺陷。最终所有回归绑定精确实现归档。

## 人工IME任务与正式质量限制

实际队列12项场景覆盖日常短句、长句、分音、编辑、退格、取消、OOV、专名、混输、标点、术语及上下文。场景是待办分类，**没有填入人造意图、拼音、正确答案或审核人**。用户尚未提供真实作者/双审/仲裁/保管者及授权，因此实际`audit`为exit2，`freeze`为exit1且不创建冻结目录。分母没有删除，formal语言指标仍null。

完成任务时要求明确input_intent、pinyin、acceptable_outputs、key_trace、scenario、源单元/家族/near组与source授权，双审须不同且不是作者；拒绝/分歧需独立仲裁，所有审核绑定内容与协议。跨镜像相同artifact不能变lineage；全局family/near、lineage+unit、规范拼音及意图近似键不得跨split。机器规则不能证明人员真实性、语义近重复或源授权真实。

[C→B显式映射](../cl009-round6-annotation-handoff.md)明确：family-index的8个字段及输入规范化可对应旧B结构，task摘要的语义仍独立审核；四份实际preprocessing/lineage_audit/annotation_protocol/annotation_review文件、逐源artifact/协议/review覆盖、70/15/15与heldout至少2000家族、独立保管者/加密seal/参数源码二进制词库冻结仍须旧B门禁通过。没有自动转换为可放行B的配置，也没有从待测词典生成gold。

工具的结构冻结不代表正式材料齐全：`formal_language_ready=false`始终保留。仅synthetic fixture在临时目录验证正常流程；其heldout是明文，明确`heldout_encrypted=false`，不能充当B封存。

## 可追溯研究源图与双向查询

使用既有批准的固定Wiktionary片段：46,788,375B、66,593完整行，SHA`ca4df3453261f9f12f1223deb11a26d979078ea6e1af870b44da2d3dcf4e2998`。完整gzip SHA/CRC和所有页面例外未闭合；研究范围、顺序偏差与许可限制继续传递，绝不称完整正式双语库。

继续按既有锁限定2–16字Han且有明确Mandarin/Pinyin。实际结果：**591条源词条、887个源义项、241条明确英语翻译声明**；18条权利标记隔离、111条缺明确拼音、65,873条范围外，合计66,593。原件全部1328中文行并非本图接收数。POS与gloss保留原字段；原翻译sense标签未自动对齐，`sense_id=null`，无词频推断。英语→中文反查引用同一有向翻译ID，不新增反向equivalent断言。

[完整图manifest](cl009/round6/graph-manifest.json)及[源位置/反查核对](cl009/round6/graph-audit-resource.json)保存身份和分母。独立只读核对全部591行的原行SHA、887义项pointer、241翻译pointer与双索引，未发现字段差异；这证明字段追溯，不证明语言事实正确。完整派生图留在`.local/round6-diagnostics-cdc7351/graph-a`与`graph-b`，均附原许可/署名的无损封装，不把页面例外未全审的正文再次随本PR分发。原许可证的MIT/CC BY-SA边界不变。

|产物|字节 / SHA-256|
|---|---|
|assertions.jsonl|626,590 / `1979ad15a351339edfccd492c154b41007f15f8fc590d4ec8ca53b199795430b`|
|lookup.json|33,723 / `5403311f125de0e8468576519bcced614d2785084cbfb09f856310558806e04b`|
|图manifest|1,402 / `ac19db545f667de918b61a63b92fd3a0747f892ebb092eb7b7dcd33b9b567340`|

精确源码git archive、新独立模块路径和`python -I -X utf8`的两个子进程构建，**五个产物逐字节相同**；没有构建联网，实际命令保留。两次整体命令耗时1.2771/1.1013秒。额外资源诊断的build为0.8948秒，OS进程峰working set130,834,432B，包含完整原件/行数组/图/序列化，不是单算法live成本或TSF资源成绩。

## Wikipedia固定原件准入：仍未放行

只检查已选官方`自然数`page342/revision94816134，时间2026-10-07T07:31:16Z。API响应SHA`d0cbce6752c5971399e2176470992549bca95ee61800c5bd45f8ad507b99d70f`；正文10,501B，revision SHA1`a8aa38891980a843b77d0715fc69d90d23e520e2`、SHA256`56e173a1f64ee94c13e1061e3cc970ebe3153e75f8e8cffc50c8293073392f12`均实际匹配。

[取得记录](cl009/round6/wikipedia-acquisition.json)保存官方URL、UTC、HTTP及原件SHA；条款元信息响应也固定SHA，末编辑者不充当全部作者。源文含模板/引文/媒体等可能例外，作者历史和逐页例外审阅、可复现提取/分词实现与配置尚缺，[准入报告](cl009/round6/wikipedia-gate.json)实际exit2。原件/条款正文只留`.local`，没有下载全dump、新语料或分词依赖，没有token/DF/Zipf观测值，没有新批准生产来源锁。

## 全量回归、身份及未执行项

[完整10条检查](cl009/round6/checks.json)全部exit0：Rust1.90.0，locked fetch、offline debug/release、fmt、Clippy `-D warnings`、**60 Rust**、doc、资源审计及**83 Python**。83=旧69+本轮14，未删旧测试。精确归档中隔离重跑83 Python另通过。[专项命令](cl009/round6/diagnostics.json)保留正常exit0及预期拒绝exit2/exit1原文；拒绝不写成正式准入通过。

源码归档SHA **`d15e239ae3e30506612fdcf790d9a481977d042b94630c063b021364a67dbe47`**；工具文件SHA见[provenance](cl009/round6/provenance.json)，原始/发布字节分别登记。Python3.12.10解释器exe SHA`4d6f5f81a4bca11191c4c7c6b43632694d0a4ce74e068619d8fdc161d469859a`不代表整个标准库二进制集合。

旧正式包本轮完整verify exit0、九文件SHA只读复核：manifest仍`5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5`，runtime仍`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`。源码未改发布/回退/迁移模块。**本轮没有重跑全量包双构建及七点中断380秒测试**，R5原证据不改绑为R6新验收；旧回归中的实际fixture恢复测试仍完整执行。物理断电与跨版本用户词典迁移未测。

64条原型1000次基准仍只是工程回归：79,000事件P50/P95/P99为2,000/4,900/10,600ns，max119,600ns、峰值5,996,544B；release宿主SHA`d53f54a25bd6b4fb8c114a06966010d4b98bf0597efb7e9c5e897e96c59e5b80`，本轮source-v2指纹`8fb63c288fd61185bac9392d0236a9d2b5f68662978ab37469f5becca35c8dbf`。旧83种子逐例输出同样保留，不属于正式语言gold。桌面背景负载未控制，不作跨轮优劣结论。

[历史保全](cl009/round6/history-integrity.json)确认起点79份报告无变化，旧测试未删，核心及生产数据代码未改。全部发布件Git字节校验表见[checksums](cl009/round6/checksums.json)，原始runner源码与原路径见[runners](cl009/round6/runners.json)。最终新HEAD的Windows/Linux push CI由共享R6汇总独立核验，本报告不提前宣称成功。

|状态|本轮结论/后续依赖|
|---|---|
|已通过|可执行人工任务/双审绑定/结构冻结工具、4项草稿门禁缺陷修复、研究图5产物双构建与全字段源定位、完整工程回归和旧正式包只读验证|
|部分通过|有实际来源的POS/义项/双向查询研究图；原句法字段可追溯，未完成逐页权利、语义审读及义项对齐，不是生产sidecar|
|数据/人工阻断|现代token统计、授权/代表性、实际人工意图/答案/角色、语义near分组与独立封存；Wikipedia准入仍失败|
|未实现/未执行|真实生产词频及结构化完整双语发布、用户词典迁移、签名更新/物理断电持久性、B显式适配及正式质量评测；未将新图接入默认核心|

CL-009不满足正式词频/语义发布退出条件，仍不放行。保持PR #3 Draft，不合main、不进入CL-010。
