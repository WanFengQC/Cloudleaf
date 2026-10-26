# Cloudleaf Round 4：可靠性、正式数据与独立评估审阅

收尾日期2026-10-27。三个方向仍为独立 Draft；`main` 保持 `a564fd47b0f054ff360592af3ccb8d744021160f`。不注册、激活或发布系统输入法，不进入 CL-010。起始分支/HEAD、依赖、文件范围与风险先登记于[本轮执行约定](../cl007-009-round4-plan.md)，设计提交在相应实现之前。方向报告保留实际测量日期，本次只按最终提交收尾，不改写旧证据日期。

本轮关闭了若干机制缺陷并取得完整授权词条，但三个方向尚未分别满足全部退出条件。完成、部分完成与未完成分别记录，不以测试数量、模拟宿主或词典非空率替代产品验收。

## A / CL-007：owner 退出、材料与卸载

最终交付 `573911ae07e0f4bce02f86aad16058e0e4c564e5`，实现冻结 `5c0ba9d651038eca5b3fcb53dba3f6835cf13a0c`；[方向报告](https://github.com/WanFengQC/Cloudleaf/blob/573911ae07e0f4bce02f86aad16058e0e4c564e5/docs/reports/cl007/round4-owner-status.md)保留命令、原始输出、失败、源码/数据/二进制SHA与未完成条件。先行设计为 `dddb693`（纯值receipt）、`cc05623`/`9fcdc91`（真实线程证明及等待失败）、`9a3712b`（异常flight快照）、`a8a0753`（显式延长压力）及 `b4555cc`（队列满优先级）。

文件范围：`adapters/windows/owner_cleanup.rs`、`recovery.rs`、`session.rs`、`watchdog.rs`、`mod.rs`、原生测试和工程压力脚本。真实重复线程句柄由Arc/OwnedHandle保有，零等待仅明确signaled才证明退出；等待失败不授权owner。userdata由全局纯Send Arc receipt保有，不含Rc/COM；弱Host hook仅留原owner TLS，Drop只标记退休。另一线程仅在真实退出、watchdog完成及唯一无窗口类注销成功后释放纯值资源，不跨线程DestroyWindow或释放COM。

正常STA退出、Unknown批独立存活、新STA明确查看/放弃后四维预算归零、S_OK有真实COM/Win32机制证据。异常故障仅在自有隔离子进程内的自有线程SetText写后ExitThread：同步/异步各一次，原instance/epoch/sequence/VK/LPARAM及计划文本可读，Writing转Unknown、后续未执行事件NotApplied，无读取死锁或自动重放。明确放弃后目录空，但两个未析构Host与两个epoch凭证仍收费，S_FALSE继续阻止卸载；没有伪造异常零资源完成。

独立复核发现并修复两项真实回归：已退出reader仍持显示token，显式discard需先回收其receipt；满队列时scope过期曾先barrier排空再接管第33键，现先拒绝新ownership，再考虑scope回退，并新增确定性过期scope回归。原断言保留、失败全文保存。新故障mode编号与旧用例冲突、新测试误期待共享reservation已销毁等准备失败另列，未删旧测试或放宽生产scope。

153 Rust（含93原生；2项默认ignored诊断另显式运行）、16 Python、fmt、Clippy、doc、资源/许可证/RustSec、debug/release和8项专项通过。审计锁及数据库身份记录在方向证据，不将历史审计外推当前全部安全。新DLL593,408字节，SHA `d253221575cc1e14a88e8f4339e16e0645431e4b4f925d1a6eff8f86510df1ac`；自建loader20轮句柄113→113，假manager不代替真实应用。

同一子进程连续180.0034021秒完成1,368轮、5,472个真实COM owner，684轮注入类清理失败再恢复，每轮原始信封/Unknown、四维零和S_OK均断言。峰计费14,852,992字节。Win32实际取得602个采样、0采样错误，采样峰150句柄/17线程、清理末值124/2；冷启动4/1不作相同稳态基线。10–40、70–100、140–170秒三个活动区间句柄范围均124–150；瞬态采样及三分钟不能证明物理内存上界或生产零泄漏，也不替代长时间真实宿主压力。模拟COM及64条原型基准单独标注，不作为正式TSF延迟。

**A1–A4仍部分完成，A5未放行。** 不执行析构的宿主退出仍可能永久保有Host/COM/JoinHandle与预算，累计后拒绝服务；任意指令退出/锁不可得、永久宿主阻塞、持续User32失败和外部强引用没有完成回收。系统注册/激活、记事本/浏览器/办公软件/多行编辑器、密码/readonly全矩阵、跨架构、安装签名升级卸载和长期真实输入尚未验收。未接入B搜索或C正式词典，不以机制压力替代这些门禁。

## B / CL-008：前置门禁和独立参考

最终交付 `96949ddc32945e2e9dd9694f9ab7e6e1c4b0f65a`；首个交付 `909a1b08642d63f206fb2a31bcbe178013cb4cae` 的[原始报告](https://github.com/WanFengQC/Cloudleaf/blob/909a1b08642d63f206fb2a31bcbe178013cb4cae/docs/reports/cl008-round4/README.md)保留34,060条采集顺序样本、失败分母、命令和原件/LF发布双摘要，搜索测量实现仍为 `7d1a0fcaaa1302a8c88e60b0f1aafbf6228e69d1`。独立审查另外发现跨来源近重复键缺陷，先设计 `4c41ee0`，再修复 `736c517ddb0903dd9dbf10bba5e80ac2ed1783d4`，最后单列[修复证据](https://github.com/WanFengQC/Cloudleaf/blob/96949ddc32945e2e9dd9694f9ab7e6e1c4b0f65a/docs/reports/cl008-round4-followup/README.md)，没有把旧样本改绑新版本。

文件范围：`scripts/check_language_readiness.py`、`scripts/evaluate_restricted_oracle.py`、两组新Python测试、预登记计划与设计、来源/许可和新报告。Reference、ExactDag、PositionDag、Beam和默认输入核心未更换。设计 `db6fa75`/`f2a04d8` 在实现 `b4d7300` 之前，完整镜像消费另先登记 `5468d9b`。

正式评估检查来源/许可/产物SHA、人工gold声明、家族/段落/近重复隔离、旧种子泄漏、独立保管者、封存、参数/代码/数据冻结及预测完整分母。实际计划缺少合法独立人工gold、家族清单和保管者，明确返回2；Top-1/Top-5/MRR保持null。工具检查声明一致性，不能独自证明人工独立性、语义去重或加密/身份真实性。

从完整Unihan按固定13读音、稳定ID各取2条，共26条；独立Python逐词匹配/穷举未调用Rust图或候选。14探针含13合法完整、1非法拒绝，逐rank全字段核对三种精确Rust策略0差异。Beam1有9例候选丢失，四个精确策略压力配置各10例预算失败；不完整等价为null。该词典派生子图验证机制，既不是语言gold，也不解除完整资源独立穷举不足。

完整授权镜像词条采用C冻结的 `2401b16` 九文件包，runtime SHA为 `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`；123,544条全部频率unknown。原24探针×12配置×30测量/5预热不调预算：Reference14例、标准Exact6例预算失败；完整同谱系Reference只8例可比且一致，其余16例等价null，另8例同谱系Exact补充不当独立证据。Position无预算失败，但7例P95>5ms、3例P99>10ms，最大单例16.151/16.8893ms，整进程峰值约185.5MiB。Beam的15–16例剪枝不作为无损加速；不同词库的时间差不作算法单因素结论。

跨来源近重复组现采用全局键，来源内段落编号仍按来源限定。新增三个split配对在旧实现真实红测3项，修复后通过；另测跨来源相同局部段落编号合法。74 Rust、52 Python及完整工程检查通过，实际正式门禁仍exit2；原35个Git文件（33证据及README/.gitattributes）保持旧blob。发布辅助脚本第一次把33证据当全部文件数导致的断言失败也保留，没有修改产品结果。现代频率/正式模型、人工train/dev/封存heldout、完整独立质量、纠错/模糊音/简拼/双拼/学习/增量、默认核心接入均未完成。搜索没有因更复杂而被选为生产算法。

## C / CL-009：完整固定授权词条及恢复

最终交付 `81d85ad414b3ecae6af9573b881acbd9dfa56037`，实现冻结 `0dee2e27fa7749d952df24e44bfaefc9a2331c6d`；[第四轮报告](https://github.com/WanFengQC/Cloudleaf/blob/81d85ad414b3ecae6af9573b881acbd9dfa56037/docs/reports/cl009-round4.md)保存完整质量、来源选择、命令/原始输出和阻塞。设计先提交 `65ae777`/`fb2a743`，再实现 `7afd507`、无损授权原件封装 `2401b16` 和特殊归档成员拒绝 `0dee2e2`。

文件范围：`tools/data_pipeline/cedict_source.py` 与原管线的新格式分派/事实校验、新容器测试、来源锁/审核清单、上游许可证据、README及本轮报告。共享schema2/3、TSV v2、原型、音节表和A/B接口未变。

数据权利与取得方式分别核验。没有自动访问禁止脚本访问的MDBG查询/API/export；采用明确CC BY-SA4分发原始ZIP的固定公开镜像 `poliwhirl555/cc-canto-cedict-jsons@7019e1c3ec3de3a44b165b15177ce2bb39a026d3`。核对README/LICENSE原文Git blob、原件头部许可/125012条/日期、全件CRC和唯一普通成员。不包含镜像解析器、JSON或CC-Canto，不称最新官方版或出版者签名。数据保留署名、许可、修改说明和ShareAlike，工具代码另为MIT。

固定ZIP为3,975,327字节，SHA `fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`；唯一成员9,867,130字节，SHA `9a3a8627c39701ecbd10246647f759f737ccd560cdbd29ef50bed8244e9ebd03`。125,012源记录接收123,544条（110,045多字、13,499单字），隔离1,468条：763不符合明确数字声调格式、704当前音节表不支持、1运行时预算；源坏行/重复/冲突为0。事实层保留繁简、大小写原读音及原始英语段，241条超长runtime注释省略仍保留完整侧包。所有频率/POS仍null/unknown，不补零或推断。

最终固定源码git archive、新无pip venv、隔离模块路径、禁止网络的独立进程双构建/verify，全部九文件一致。最终manifest SHA为 `25b72932432b927316febdb691ff9edfad8380a08cc6de5ef38751853c037c9d`；B早期冻结manifest为 `85b6f837e0644716605f9458a59ebbc3044ed2b03a1c6d26f497ac2fa5993de2`，其余八文件一致，builder摘要变化不冒充跨版本九文件一致。后台Python完整构建28.38/31.42秒、进程峰值约1.008GB，不代表实时输入内存或Rust查询延迟。

旧字符包到新词条包的七个自建真实子进程发布/回滚中断点均按预期exit73，恢复后资源/指针完整、用户词典探针SHA不变。60 Rust、55 Python、README九命令、fmt/Clippy/doc/资源审计和原型基准通过。容器特殊类型、篡改重哈希、重复/冲突及坏记录有独立拒绝测试。保留上游尾随空白导致的diff检查失败回放、NTFS冒号缓存实际为ADS后离线复制普通文件的同SHA证据。

现代频率和双语来源本轮分别审查：THUOCL的DF不代替token频率；SUBTLEX独立包分发范围、wordfreq中文组成来源仍受阻。元信息文档有实际UTC、字节SHA和固定Git blob；Tatoeba本次超时不伪装重新核验或取得语料。尚无批准的现代token频率固定包、人工IME gold、结构化POS/义项对齐及完整双向翻译。旧18条Unihan隔离读音未扩表。正式语言质量、签名安装、跨版本用户词典迁移和物理断电验收未完成。

## 新提交CI、证据保全与下一步

三个最终方向SHA的push运行均已核对Windows/Linux两个job、全部步骤及两个绑定相同完整SHA的未过期产物digest，原始响应见[本轮CI快照](cl007-009-round4-ci.json)。旧成功和PR合并快照不代替新精确提交。共享汇总自身的后续提交另在推送后记录到PR #1及最终交付，不伪造自含SHA报告。

| 方向 | 最终提交 | 实际检出精确SHA的双平台push CI |
|---|---|---|
| CL-007 | `573911ae07e0f4bce02f86aad16058e0e4c564e5` | [40362061101](https://github.com/WanFengQC/Cloudleaf/actions/runs/40362061101) |
| CL-008 | `96949ddc32945e2e9dd9694f9ab7e6e1c4b0f65a` | [39984226936](https://github.com/WanFengQC/Cloudleaf/actions/runs/39984226936) |
| CL-009 | `81d85ad414b3ecae6af9573b881acbd9dfa56037` | [39983095185](https://github.com/WanFengQC/Cloudleaf/actions/runs/39983095185) |

共享分支只更新本轮计划/汇总/证据和路线入口，没有合并方向实现或修改共享接口。历史报告、64条原型词典、41事件语料和83诊断种子按各路径Git clean后的blob核对；Windows CRLF检出与Git LF分开处理，不能将换行比较器错误解释为历史报告被修改。共享22/A70/B61/C45份起始历史报告无变化，核心/测试/适配文件未删除。A44项、B33项发布证据的Git blob SHA及C九文件双构建/交B冻结包另外独立核对，见[保全记录](cl007-009-round4-shared/preservation.json)与[产物复核](cl007-009-round4-shared/direction-artifacts.json)。

共享被测源码保持 `ddf3ed6655216dee951d4d154665ae9a63763d1b`，60 Rust、14 Python及12条格式/构建/静态检查/文档/资源审计/基准命令全部通过；最终汇总只增文档证据。完整[检查](cl007-009-round4-shared/validation.json)、[原始日志无损封装](cl007-009-round4-shared/raw-logs.json)及[原件/展示件摘要](cl007-009-round4-shared/provenance.json)保留。首次比较器CRLF误比对、Windows包装器未解析新增PATH的启动失败均保留，修正比较方式/显式工具路径后完整重跑。首次发布原始日志的EOF空行触发仓库空白检查后，原文按UTF-8封装到JSON，展示件去行尾空白并单独登记双SHA；没有取消检查或丢弃失败。

下一依赖仍为A的异常引用回收/长期真实宿主与系统验收、C的正式频率/独立gold/结构化双语和生产更新持久性、B的独立完整语言质量与资源门禁。三个方向分别满足退出条件后才评估CL-010；当前不放行。
