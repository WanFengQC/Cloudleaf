# Cloudleaf Round 9：单人流程收敛与外部验收准备

2026-10-30。用户本人是唯一项目负责人；四worktree隔离代码，不代表四名真人。Codex子任务是技术执行与核对，不能充当双审、签署者或外部验收人。先提交[计划](../cl007-009-round9-plan.md)`547c308`，按A→C→B→共享顺序完成实现和各工作区检查；同一工作区同时仅一项写任务。CI只读核验可并行。main仍`a564fd47b0f054ff360592af3ccb8d744021160f`，无合并、强推、Ready切换、TSF操作或CL-010。

## 精确身份与新CI

|方向/分支|R8实际起点|本轮被测代码|R9最终方向HEAD / 新push CI|
|---|---|---|---|
|共享 `codex/cl007-009-contracts`|`607c2c11227cb9a9e5507b0fa0bc0aae1cf503ea`|`48b122d69f1086fbc42365427e71080775db3a26`；生产源码未改|本报告提交后记录完整head与CI，不制造自引用SHA|
|A `codex/cl007-windows`|`333dc90e0985a13f75629e29599f3df1d7d951ab`|`eb353ae981ea891d4f84888fd7551e95e8998ca4`|`1a05633c27c133bc0558b0fb955b0f916edb6b72` / [40922169369](https://github.com/WanFengQC/Cloudleaf/actions/runs/40922169369)|
|C `codex/cl009-data`|`ef6cf16c558f5521cd776e4805068a07f15aedd2`|`72985c50904a41018058f1d05e3ce44902b58aa8`|`1c9a047c74aea01e504ed355601c0fe021b53f4b` / [40924257753](https://github.com/WanFengQC/Cloudleaf/actions/runs/40924257753)|
|B `codex/cl008-search`|`f991a8128cd07d4af57eb773cc2e598cf7e5809a`|`a1063f3c9053091a6b66b6d34a32a4b0f567ed83`|`2751b1512394e09c92bca2b4329456c3cd3c30de` / [40924998999](https://github.com/WanFengQC/Cloudleaf/actions/runs/40924998999)|

A/C/B新最终head已逐项核验push事件、完整SHA、两平台全部步骤及各两个未过期产物的head/digest。实际A各19、C各17、B各18步骤，原始响应在[方向CI](cl007-009-round9-shared/direction-ci.json)。共享最终报告提交推送后同样单独核验，结果绑定`.local/round9-final-receipt.json`和PR #1，不继承R8成绩。起点四个R8成功与真实分支/PR快照另存[基线](../reviews/pr1-round9-evidence/baseline.json)，均不改绑为新提交。

当前gh不在PATH，以官方GitHub API完成同等精确head核验；匿名额度降低后使用既有Git凭据仅在进程内认证只读请求，未打印或保存token。缺少CLI不是CI成功或失败的证据。

## 实际检查、失败和字节

|方向|本轮本地完整回归|专项结果与限制|
|---|---|---|
|共享|11命令均0；64 Rust、14 Python；fmt/Clippy/all-target/doc/release/workspace审计/diff/status|源码/原资源未变，未新增核心测试、基准或算法质量结论|
|A|157 Rust passed，保留原2 ignored；38 Python passed；Clippy/offline/doc/release/依赖审计通过|pending材料exit2、10缺件，child_started=false；无独立native/loader/长测/TSF|
|C|60 Rust、95 Python；fmt/Clippy/offline/doc/release/审计通过|固定全扫描0，覆盖拒绝2、真实pending标注audit2；不重下载、不计算正式频率|
|B|81 Rust、84 Python；15命令符合预期|实际export/status0/0，三正式gate2/2/2；内部进度成功不是gold批准|

各方向原文、命令和返回码见下列精确报告。没有删除测试/失败案例或降低预算。代码/数据/工具/产物身份分列；build日志可能复用编译缓存，二进制SHA不等于新性能测量。所有核心输入路径、Rust候选算法和共享接口均未改。

本轮失败完整保留：A旧字符串批准反例10测1fail；C新API未实现红、原始字段分母缺失6测1error；B旧团队占位1fail和新API缺失7errors。C新工具/新API缺失不冒充原有生产算法损坏。C归档另有README EOF和日志展示EOF的diff2；首次暂存检查2后链式命令仍提交`0ee401e`是归档错误，后续保留旧提交、复现失败、原始字节，只修展示并另提交，最终47项Git清单及暂存diff0；没有掩盖或强推重写。

共享检查首次在Windows裸命令rustc启动处WinError2，前2命令0、其余未执行；保留初始目录与修正前运行器，显式解析既有工具路径后11命令实际全跑通过。只读准备中的错文件名/Windows rg通配符错误亦[单列](../reviews/pr1-round9-evidence/harness-preparation.json)，未另存原始进程字节的项明确标为工具转录。没有把链式命令最终0当各项成功。

## A：材料绑定的只读准入

[精确A报告](https://github.com/WanFengQC/Cloudleaf/blob/1a05633c27c133bc0558b0fb955b0f916edb6b72/docs/reports/cl007-round9-status.md)。设计`ed02a81`、旧真实反例`062f619`先于实现。新增`scripts/check_windows_soak_admission.py`及6项测试，原`stress_windows_faults.py`长期预检复用同一核验入口；旧测试增加反例并保留原断言，新增5个pending环境/回滚/执行授权/runner批准/人工应用记录模板和操作入口。

旧预检仅接受approved/isolated及五个字符串，实际材料不存在仍可通过。现在核VM ID、Windows版本/架构、快照ID/SHA、实际回滚及授权文件path/SHA、环境/快照/operator/owner/时间/允许档位绑定，并与runner完整源码/数据/工具/二进制/配置/系统identity相符。材料或绑定缺失在创建segment/child前拒绝，旧批准不能自动升级。允许同一用户兼owner/operator，不虚构外部观察者。

工具只证明文件与声明一致，`declarations_independently_verified=false`、检查本身不授权执行/TSF。真实环境和授权未准备，新旧请求仍pending。R8 reporter仍逐segment，原中断/续跑回归随38测执行，不拼连续时长；本轮没有新长段或性能采样。用户必须实际准备/恢复验证环境并决定授权；之后才可重做准入，长期与TSF仍需各自原门禁。

## C：固定Ngrams全分析，统计采用仍未批准

[精确C报告](https://github.com/WanFengQC/Cloudleaf/blob/1c9a047c74aea01e504ed355601c0fe021b53f4b/docs/reports/cl009-round9-status.md)。新增`tools/data_pipeline/ngram_analysis.py`、6项真实实现测试、设计/操作说明；只读取R8同版两个固定原件，生产来源锁/登记/包/解析规则不变。原始triplet_fields先计数，坏字段不从分母消失；完整扫描保存所有异常计数和有限行号/SHA反例，partial不称CRC成功，异常时缺项值null。

固定gz60,833,224B/SHA`6ab29862a8c904c261bb9d5a02a342285c4dbabf79cee5c4f8818076fd44fb86`，totalcounts7,479B/SHA`f4a136b63419c0f34dfbfcf76f501ac80a745ad535f5cc9bda2666189a35e9f3`。解压195,878,750B/SHA`f5535ee7d0c0a5ba18bff3b4019d150da0e63265b8e2abdebb2d442db45b95f0`、严格UTF-8及EOF CRC一致。508,674行/唯一词头、19,074,163原始且有效三元组、464年1480–2019、现登记诊断0。形态215,764 other/292,899 tagged suffix/11 underscore，不生成POS事实。216,950,573缺失token-year组合不是观测0或覆盖率；只基于有totalcounts的年。全扫描tool49.183秒/process49.423秒，单次、O(unique tokens)内存；4,718,833词头UTF-8字节只是payload，未测分配量或进程峰值。

v3字段仍保留原名field2/field3；根据旧版说明的match/volume解释标inferred，中文具体分词模型/版本、v3截断、分母/窗口和现代代表性未闭合。token方案需同版同范围合法全token分母；DF不得跨词/跨年把book-count相加去重；Zipf仅能由已批准正token比例导出。正式token/DF/Zipf继续null，未导入生产。报告含Google固定编纂来源/CC BY3署名、许可链接和只读派生说明，不授权原书全文；source仍candidate-not-approved，研究用途，Wiktionary research_only、CC-CEDICT/Unihan既有范围不变。

实际`pipeline.py::verify_release`只接受旧固定词条schema2；`linguistic_sidecar.py::validate_sidecar`仅类型/引用。语义bundle reader/verifier、内容双审和失效绑定、可信签署根及manifest、兼容升级/回退/中断测试仍待实施。没有虚构POS、义项对应、反向翻译或许可。

## B：个人草稿与正式gold分开

[精确B报告](https://github.com/WanFengQC/Cloudleaf/blob/2751b1512394e09c92bca2b4329456c3cd3c30de/docs/reports/cl008-round9-status.md)。设计/红测`5588459`先于实现。新增`scripts/manage_ime_annotation_queue.py`、8项测试、作者操作README；将`config/evaluation/language-readiness-v1.json`的虚构团队占位改null，不代用户填实际评估者。C的13字段/三协议摘要、三个B正式工具和全部搜索算法未改。

显式SHA导出仅split=null的私有`internal-development-only`草稿，保留原槽ID、允许用户添加原创；不填身份/答案/批准。status不输出正文，分列个人字段完整、来源/授权文件与范围一致、当前内容及三份非空协议下的不同且非作者reviewer声明、必要独立仲裁和保管缺件。两个ID只说明声明不同，所有真人/实际签署证明false；formal_ready始终false，正式Top-1/Top-5/MRR始终null，不打开其他heldout文件。

实际C packet SHA`f83310f21567d47ff34bd6753951253dacfaf53d51bfd9193112ce1241a68bff`，采样C final`1c9a047...`。导出12原槽、个人完整0、当前审核0；三正式gate均2。用户本人可原创/标注/修正并得到内部进度，不能把同一人多次审核或AI审核当独立gold。真实授权、独立双审/仲裁、全局语义近重复/家族和独立heldout封存、70/15/15及至少2000家族的门禁仍缺，默认引擎不替换。

## 共享PR #1与人工决定

[R9人工准备](../reviews/pr1-round9-human-review.md)引用原十四项清单，不把R8工作重复包装；[新决定模板](../reviews/pr1-round9-decision-template.json)所有人/时间/head/CI/批准字段仍pending/null/false。用户可独自决定自己的PR，Codex只技术核对。完整main→`48b122d`当前审查范围patch3,400,917B、逐文件blob/SHA在[技术证据](../reviews/pr1-round9-evidence/review.json)；包含最终报告自身的完整差异另存最终凭据。

共享生产代码/公开测试/资源/配置相对R8未变；三方向input/types blob一致，merge-base仍`9af9bc1b2ebacc3ad76124cfa39d09de8eb77bd1`。这不是已经整合。只有用户实际合并PR #1后才fetch真实新main，审合并方式/冲突，逐方向普通merge并重做新head回归和两平台CI；不rebase、强推或自动合方向代码。人工清单明确Unknown取消不证明原文档未写入、session/epoch不可省略、UTF-16不是字素编辑；mergeable/CI不代批准。

## SHA、保全与最终门禁

|工具|本轮SHA-256|
|---|---|
|A准入checker|`6abcc4e5851314803eba67a21596daee18d2a476e5d431e5979f2e6671eca6e2`|
|A runner|`8ac58e1bb51f133b2022e887a32c0c994cccc13bd434736861fda4164278e1cc`|
|C原字段分析器|`fb78438ca3e4f79cb245ecb89219038bda615aa2d7866c305c131598904f0849`|
|B作者队列|`3c7c02eb3dca82c697b1004f74ed98a5b1f50ffc097f78960bb0731708a737c3`|

源码指纹范围、原数据/固定包、实际release二进制SHA与原路径在各方向identity；C旧source-v2不包含tools/data_pipeline，已另绑定新工具SHA和实现Git，不用不覆盖该目录的指纹冒充完整分析器身份。共享本轮release CLI身份见共享原检查，未运行新基准。[Git/原件核对](cl007-009-round9-shared/direction-artifacts.json)分别核A32/B44/C47项清单、原字节与历史，均为Codex技术核对，不代表不同真人审核。起点报告按工作区分别共享119/A529/B420/C210份保持，不相加冒充独立成果；旧测试/64/41/83原型资源和正式9文件SHA保留。

|状态|单人可做与必须落实的条件|
|---|---|
|done|A真实文件绑定预检、C固定分析/分母、B私有草稿进度/占位移除、全套回归/原失败和数据工具身份、人审准备|
|partial|工程工具可用，文件/声明一致不是真实授权；C结构通过不代表统计/语言质量；个人标注不是正式gold|
|blocked|用户实际环境/快照恢复与执行授权及合并决定；真实独立reviewer/必要仲裁/heldout保管和授权；统计采用、人工语义审读与生产发布器|
|not-run|8h/24h、TSF注册/激活/安装/卸载与真实应用、物理断电、正式gold/语言评分、生产频率/语义发布、默认核心替换、CL-010|

已复现的A准入、C新诊断分母和B占位问题已修；已审增量没有新的未修可复现P0/P1。生产不可回收Host/COM预算占用后的真实文字恢复、长期资源/真实交互、独立算法质量及正式数据语义仍是原P0/P1级放行阻断，不因本轮工具或CI通过而关闭。PR #1只达到可提交本人审阅的技术准备，未获批准。A/B/C各自退出条件均未满足，**不允许进入CL-010**。
