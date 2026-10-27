# Round 5 正式实施约定

2026-10-27。目标仍为完整正式输入法；Rust自研核心、Windows优先和独立平台扩展边界不变。不合并main，不注册/激活/发布输入法，不进入CL-010。四个PR维持开放Draft。

## 实际起点与证据

已读取实际AGENTS、roadmap、Round4共享/方向报告和对应设计，四个本地工作区均干净，远程refs逐一相同。main为`a564fd47b0f054ff360592af3ccb8d744021160f`。

| 方向 | 工作分支 | 起始精确HEAD | 本轮重核的基线push CI |
|---|---|---|---|
| 共享 | codex/cl007-009-contracts | e4ac2e35e5c47d8d661aadc211a7de806d1a61f4 | [40363579414](https://github.com/WanFengQC/Cloudleaf/actions/runs/40363579414) |
| CL-007 | codex/cl007-windows | 573911ae07e0f4bce02f86aad16058e0e4c564e5 | [40362061101](https://github.com/WanFengQC/Cloudleaf/actions/runs/40362061101) |
| CL-008 | codex/cl008-search | 96949ddc32945e2e9dd9694f9ab7e6e1c4b0f65a | [39984226936](https://github.com/WanFengQC/Cloudleaf/actions/runs/39984226936) |
| CL-009 | codex/cl009-data | 81d85ad414b3ecae6af9573b881acbd9dfa56037 | [39983095185](https://github.com/WanFengQC/Cloudleaf/actions/runs/39983095185) |

每次基线运行均核对push事件、精确SHA、Windows/Linux两个job/全部步骤及两个未过期产物digest；这些不证明后续Round5提交。工作流的PR默认merge快照不能冒充实际检出head；新提交分别以push验收。Rust/MSRV1.90.0、Edition2024、resolver3不变；A存在独立审计的Windows依赖，B存在独立研究crate，C/共享仍为六核心crate；不能把根workspace清单当作所有方向实际依赖。

## 设计、测试与实施范围

各方向先提交设计，再提交可重放测试与真实红测记录，再实现生产或离线工具代码；代码冻结后运行完整回归。测试使用的临时机械/自编格式数据不当语言gold。根只写本轮执行约定/审阅汇总/证据和路线入口，不在共享分支合入方向代码。

| 方向 | 修改范围 | 必须取得的证据 |
|---|---|---|
| A | `adapters/windows/watchdog.rs`及必要的恢复接入、真实COM/Win32测试、新v5故障/准入设计和报告 | 启动即全局登记纯Send监测资源；真实worker结束后才join/归还slot与Lease。32个异常owner后健康新会话、外部COM强引用、迟到回调、FIFO及Unknown隔离。部分启动/登记失败、并发回收与长期占用分别覆盖。 |
| B | 研究crate内部共用算法的可选profile入口、新工程host/脚本/语义测试、正式前置契约和报告 | 默认Noop不读时钟，既有API/默认核心不变；归一化、建图、搜索和排序/物化的包含/嵌套关系明确。每例profile与原路径全字段等价；超预算/非法/等待/空图不删，记录实际阶段、开销和原始样本。 |
| C | 独立隔离审计工具、typed observation/语言侧包验证器与测试、来源决策/独立人工gold计划和报告 | 1468条隔离原行/读音token/原因与固定原件逐条对应；合法未支持不能误称源坏数据。token/DF/Zipf单位、分母/公式/许可分别校验；POS/义项/翻译/短语只接受可追溯事实，不自动把斜线段变人工语义。 |

A只调整平台私有资源所有权，不改变总预算/监测slot上限以掩盖占用。已未析构Host/Rc/COM/未知锁仍不得跨线程释放或退费；只回收真实退出且安全的纯线程/类资源。S_FALSE是保活边界，不是可用性已完成。任意异常退出和永久宿主阻塞仍需明确退出条件，不能假设全可回收。

B的新增公开观测入口只属于实验crate内部API加法，不修改共享types/默认input，也不替换Reference/ExactDag/PositionDag/Beam。计时必须注明嵌套不可相加，观测开销与未仪表化耗时分开；同谱系一致不充当语言正确性。使用原冻结CC-CEDICT全量123544条，runtime SHA `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`，原24探针/12配置/30测量/5预热和预算不调。正式计划缺gold时继续exit2、语言指标null。

C保持schema2/3、TSVv2、生产音节表和现有未知词频不变；新模型为独立离线侧包，不进入输入路径。固定源ZIP SHA `fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`、125012源条目的123544接收/1468隔离为核对起点。来源元信息许可审查不等于取得数据许可；不自动访问受限MDBG接口、不联系第三方、不造词频/人工标注。C新增代码需新干净源码归档/无pip环境双构建和七点恢复，旧交B的2401包不更换或改绑。

## 依赖、检查与交付

三方向分别在已有工作区工作，不同时修改共享接口。C的新侧包不作为B本轮数据切换；B消费旧冻结runtime，C质量审计独立报告。正式语言评估的来源/家族/全局近重复/独立标注/双审仲裁/封存和参数冻结需要B/C互相审阅约定，缺真实材料必须拒绝，不能靠fixture放行。

每方向保留全部旧测试与Round1–4报告；新失败原文/实际返回码、未提交状态与已提交SHA分开记录。运行README中的locked fetch/offline构建、fmt、Clippy全部target拒绝warnings、Rust all-target/doc、Python、资源/许可证和对应漏洞审计；再运行方向专项、基准/资源记录。新目录发布原文或无损UTF-8封装，展示件若规范换行/空白须保留双SHA，不用日志格式问题改变断言。

最终分别交付：设计和测试先行提交、具体文件清单、实现冻结SHA/数据及二进制SHA、全命令与原始报告、性能/预算/失败分母、新最终head的Windows/Linux push CI、完成/部分/未实现/阻塞与退出条件。共享汇总自身的CI推送后另记PR1及最终交付，避免伪造自含SHA报告。三个方向全部退出条件满足后才评估CL-010，本轮不放行。
