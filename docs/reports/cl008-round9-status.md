# CL-008 Round 9：单人作者队列及明确的正式质量门禁

2026-10-30。分支`codex/cl008-search`，R8实际起点`f991a8128cd07d4af57eb773cc2e598cf7e5809a`且clean。设计/失败测试先提交`5588459`，被测实现 **`a1063f3c9053091a6b66b6d34a32a4b0f567ed83`**；最终交付提交只归档本报告/原始证据，不把其SHA冒充新的算法或质量测量。

用户本人是唯一维护负责人；worktree、Codex子任务、角色名称及两个不同字符串不代表实际独立人员。本轮未创建任何真实作者、答案、授权、审核、heldout或正式gold，未改变Rust/默认引擎、C字段与三协议摘要或B三个正式门禁。

## 实际文件与行为

|文件|修改目的|
|---|---|
|`docs/cl008-round9-authoring-queue.md`|先行设计，分开个人草稿、当前摘要下审核声明、正式gold三个层次|
|`scripts/manage_ime_annotation_queue.py`|显式SHA导出未划分作者草稿；核授权文件及声明、缺件与审核进度；报告不包含意图/拼音/答案|
|`tests/test_annotation_queue.py`|8个真实实现测试，机械fixture明确不是许可/人员/gold|
|`config/evaluation/language-readiness-v1.json`|历史虚构团队占位`cloudleaf-development-team`改null，实际评估人由用户明确登记|
|`docs/acceptance/cl008-round9/README.md`及README入口|实际作者导出/标注/进度操作、独立双审/保管边界和正式流程入口|
|本报告及`cl008-round9/`|原始红/完整检查、固定C作者源、原字节/展示件身份及历史保全|

export只接受C schema1、非合成/非词典派生且全部split=null的作者源，固定输入原字节SHA，生成新的B私有`internal-development-only`外层；不填作者/答案/批准/split。原槽ID不能丢失，可增加本人原创任务。不要把冻结或含heldout正文的包交给工具；发现任务已划分即拒绝，无导出。输出存在也拒绝，保留个人改动与历史。

status把个人字段完整、来源绑定、实际授权文件SHA/范围声明、当前内容和三份**非空**协议SHA下的两个不同且非作者reviewer声明分列；有reject还需不同于作者/两reviewer的当前仲裁ID声明。内容/来源/协议变化使原审核声明失效。实际文件存在或ID不同不证明真人独立性、签署事实或权利；所有这些证明标志保持false。

有效报告exit0仅表示工具成功生成内部进度，`formal_ready`始终false、正式Top-1/Top-5/MRR始终null。三正式门禁仍分别核真实来源、任务/上下文/家族、双审仲裁、封存和参数冻结；70/15/15、heldout至少2000家族及独立保管要求未改。本工具不冻结/划分/搜索或额外打开heldout文件，不从待测词典生成答案。

## 原始失败与完整回归

[原始红测](cl008-round9/red/result.json)在`5588459`实际运行8测，**1 failure + 7 errors、exit1**。failure为旧`evaluator_id`虚构团队占位不为null；7个error为本轮新增export/status API尚不存在。后者是新能力缺失的先行测试，不冒充已存在算法实现错误。完整stderr/stdout保留，没有删除或跳过断言。

实现后初次定向8测通过（工具转录，未另存独立进程字节）；追加“把空三协议重新散列仍不得计审核”子例后，最终完整Python回归覆盖同一8测及所有旧测试。[15条实际命令](cl008-round9/checks/validation.json)均符合预期：

|检查|实际结果|
|---|---|
|rustc版本、git diff/status、fmt|0；Rust1.90.0，材料实现提交clean|
|Clippy `--workspace --all-targets --locked --offline -- -D warnings`|0|
|Rust all-targets / doc tests|0 / 0；all-targets **81 passed**|
|release locked/offline构建|0，记录实际现存CLI路径/字节SHA，不作新性能宣称|
|workspace依赖/许可/64/41/83资源审计|0|
|完整Python `discover -s tests -p test_*.py -v`|0，**84 passed**，含新增8测|
|真实C作者源export / status|0 / 0；只生成待填私有草稿及报告，12原槽、个人完整0、当前审核声明0|
|现三个正式gate|2 / 2 / 2；全部仍阻断，指标null|

测试真实导出/进度入口，覆盖个人字段完整仍非gold、同人/自审、空协议重散列、授权文件缺失/SHA/范围变化、旧内容摘要失效、分歧仲裁、同人保管、已分配包拒绝、原槽丢失、输出不覆盖和状态报告不泄漏正文。机械fixture只验证代码；完整声明一致也不会将`formal_ready`变为true。

计划的protocol阶段现在也因实际评估者null而拒绝，这是删除虚构身份后的真实缺件；没有降低原门禁。旧R8 protocol通过记录保留在原提交，不改写。本轮没有重复搜索性能矩阵、语言打分或产生独立质量成绩；常规Python回归中的既有机械Rust宿主测试不等于正式评测。

## 版本与数据身份

|身份|SHA-256 / 说明|
|---|---|
|本机实际源码字节，benchmark.source_sha256范围|`c83d5ee05d95b5d079955939fe0b7a14d4517f39d6033d475d2c34f95807e8eb`，包含Python脚本/测试，不代表Rust算法改变|
|新作者队列工具|`3c7c02eb3dca82c697b1004f74ed98a5b1f50ffc097f78960bb0731708a737c3`|
|未变B handoff摘要契约工具|`cd010b12f6f8e20fe51c5e857b7a2e8aa46a3dc39c2079d2239045ba6c6b5bc3`|
|真实C作者源packet|`f83310f21567d47ff34bd6753951253dacfaf53d51bfd9193112ce1241a68bff`，采样C完整HEAD`1c9a047c74aea01e504ed355601c0fe021b53f4b`，仍12项空作者/答案/审核|
|release CLI，347,136B|`a3f5652c86888dfea8288329af3cc08bbc14239c1a280b2111e1a6b0148435f6`，实际build执行，不代表新基准或原生TSF产物|
|语言计划（evaluator null）|`19565b1a2df832113596fcdb6ed2b167cbcff2791459c1835c727437f157501e`|

绝对本机原路径、其余配置与工具身份在[identity](cl008-round9/identity.json)。[无损原件](cl008-round9/raw-artifacts.json)包含39份原始进程/文件UTF-8字节，LF展示件另列[44项Git发布字节校验](cl008-round9/checksums.json)，[两个运行器](cl008-round9/runners.json)保存实际源码和命令。代码注释和新文档UTF-8；没有覆盖R1–8。起点420份报告、旧测试和C契约/三个B正式工具/其他准备配置、全部Rust及数据无差异，见[保全](cl008-round9/preservation.json)。

最终交付HEAD在报告归档后产生，自己的Windows/Linux push CI由共享R9最终汇总核对，不能继承R8或被测实现的旧成功。

## done / partial / blocked / not-run

|状态|结果及负责人边界|
|---|---|
|done|作者队列导出/缺件进度、分层声明和授权字节一致性、历史team占位移除、8新测试及81 Rust/84 Python全回归、12真实空槽输出和三gate阻断证据|
|partial|用户可以开始原创和个人标注，但现尚无实际作者/授权/答案；声明一致性不是已签署或独立人员证明|
|blocked|真实授权及独立双审、必要仲裁、全局语义/近重复审核、独立heldout保管/加密封存、正式规模家族与冻结；不能靠同一人重复审核或AI解除|
|not-run / 未实现|真实gold和正式Top-1/Top-5/MRR、正式打分/发布比较、默认核心替换；纠错/模糊/简拼/双拼/学习/增量等原产品缺口仍在|

本轮已修占位身份表述和补可独立完成的作者工具，已审增量未复现新的未修P0/P1；独立语言/资源与真实数据条件仍是发布阻断，不声明产品无风险。PR #2保持Draft，不合main、不进入CL-010。下一步由用户私有填写原创草稿、收集真实授权，再由实际独立参与者完成正式流程；Codex不代签或制造答案。
