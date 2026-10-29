# Cloudleaf Round 5：故障隔离、数据质量与独立评估审阅

2026-10-28。按[先行范围](../cl007-009-round5-plan.md)从用户指定的四个Round 4精确提交继续，分别读取实际源码、设计、报告、工作区和远程状态；没有重新初始化或合并方向实现。`main`保持`a564fd47b0f054ff360592af3ccb8d744021160f`。四个PR继续开放Draft，不注册、激活或发布系统输入法，不进入CL-010。

本轮交付了生产平台资源修复、独立算法观测和数据审计工具，但三方向仍未分别达到全部退出条件。设计、机制测试、真实应用、语言质量与产品完成分别判断；不会以词条数量、候选非空率或测试数量替代正式验收。

## 精确交付与检查

|方向|最终交付HEAD|实际被测实现|本地Rust / Python|新精确push CI|
|---|---|---|---|---|
|CL-007 / PR #4|`7f581dfca43515105fca918ffd79e55f315f7729`|`519ea587fe5a0fe56cb2a5ebc239832a118f692a`|157 / 16|[40689982781](https://github.com/WanFengQC/Cloudleaf/actions/runs/40689982781)|
|CL-008 / PR #2|`08b09277d3beeb90e284145a3423b9f00eca8d6c`|`292cbd06614c376dbd1b8cc0a8172e099590e4db`|77 / 65|[40690727932](https://github.com/WanFengQC/Cloudleaf/actions/runs/40690727932)|
|CL-009 / PR #3|`c21db7ca70e5f51f84ca8bd30944d6722473c32c`|`6d3d4e1ac717ad96de8c839ee9857c3629ebecbc`|60 / 69|[40692652095](https://github.com/WanFengQC/Cloudleaf/actions/runs/40692652095)|

最终方向head只整理报告与入口，被测源码/数据/二进制身份没有改绑。完整CI原始响应见[方向CI快照](cl007-009-round5-ci.json)：各自核对push事件、完整head SHA、Windows/Linux两个job及全部步骤、两个未过期且绑定相同head的产物digest。CI覆盖工程检查，不代替真实应用、长时间运行或正式语言评测。PR事件默认检出merge快照，不能冒充被测分支head。

共享分支本地被测提交为`cde605f0f91fff9870f2287d940b9f603220582b`，60 Rust、14 Python及12条README命令全部返回0；[命令与返回码](cl007-009-round5-shared/validation.json)、[逐字日志](cl007-009-round5-shared/raw-logs.json)、[原始结构化产物](cl007-009-round5-shared/raw-artifacts.json)与[双SHA](cl007-009-round5-shared/provenance.json)保留。最终共享提交的CI在推送后单独记录到PR #1和交付信息，避免伪造包含自身未来SHA的报告。

## A：CL-007 v5资源所有权与恢复

[方向状态及可重放命令](https://github.com/WanFengQC/Cloudleaf/blob/7f581dfca43515105fca918ffd79e55f315f7729/docs/reports/cl007/round5-isolation-status.md)记录设计`263875e`、测试`c03e3c6`、生产修复`088bf9c`的顺序。旧实现确实在32个异常owner后丢失停止worker的可达JoinHandle，使第33个健康会话因32个monitor slot耗尽而失败；当时固定计费仍低于总上限，不能靠增加预算解决。

生产修改只涉及`adapters/windows/watchdog.rs`及`recovery.rs`固定成本接入：在线程创建前全局登记纯Send任务，构造结束、退休请求、独占状态和真实OS线程HANDLE退出证明齐备后才join并归还slot/Lease。部分创建失败同样退休。构造中、锁不可得、等待失败不授权回收；不在TLS/DllMain回收，不跨线程释放Host/Rc/COM。预算仍为64Host/64批/64MiB文本/128MiB固定、32monitor slot。新增任务开销纳入计费，不能证明安全的资源继续保活。

`render_tests.rs`新增自有Win32线程写后ExitThread、真实COM强引用、迟到/重复回调和健康FIFO回归。持续占用档先登记设计`6f85efb`，再提交测试/采样工具`ad45bfe`，`scripts/stress_windows_faults.py`只操作自己的子进程。完整157 Rust/16 Python、9项release专项、依赖/许可证/RustSec快照审计通过。

同一自建COM测试宿主持有32个异常Host/epoch原额度，健康会话持续180.000092秒完成1,017,364个FIFO周期、6,104,184次生产路径SetText调用。旧Unknown各保留一次，未自动重放；明确放弃材料后未析构Host/epoch仍各32，保持S_FALSE。[压力原件](https://github.com/WanFengQC/Cloudleaf/blob/7f581dfca43515105fca918ffd79e55f315f7729/docs/reports/cl007/round5-isolation/pinned-owner-stress/summary.json)保存622个时间戳样本、OS峰值和取样限制；后两个稳态区间句柄144、线程4、working set15,687,680B、private commit5,681,152B。取样峰不是精确峰，commit不是RSS，180秒不是8小时/24小时或真实系统输入吞吐量。

外部NativeDrain及clone使旧Host保持计费，旧epoch失效后新context完成6写，三个迟到/重复旧调用不写旧文档、不污染新上下文；合法最后释放后四维预算归零。release真实DLL599,552B，SHA`63028284a49dbbbe612edbbbaa93e24e4abf306a558c3cc42f0c4ca62c398af5`；测试exe SHA`0c10c7a15b2aabed93d6d4b180624e96d39e87e7b64ca8dfe61733ce74bf388e`。真实LoadLibrary/类工厂配假manager的20轮句柄113→113，仍不是真实TSF宿主验收。

两次旧测试失败没有删除或放宽断言：零等待清理必须有界等待真实失败尝试；容量测试显式维持有效scope并逐步核对实际入队长度。第二次失败没有捕获中间队列长度，TTL解释只是源码推断，不能写成已捕获根因；真正满队列的生产拒绝仍先于scope回退。三项失败、精确已知SHA与原文见[失败索引](https://github.com/WanFengQC/Cloudleaf/blob/7f581dfca43515105fca918ffd79e55f315f7729/docs/reports/cl007/round5-isolation/failures.json)。

## B：CL-008阶段观测与质量前置

[方向报告及78项证据](https://github.com/WanFengQC/Cloudleaf/blob/08b09277d3beeb90e284145a3423b9f00eca8d6c/docs/reports/cl008-round5/README.md)保留先设计/测试、真实红测及完整矩阵。`cloudleaf-search`新增可选Observer，共用同一算法，默认Noop不读钟。修改范围为研究crate的`profiling/lattice/search/lib`、`resource_phase_probe`、离线分析/准备脚本及回归；共享类型、默认输入核心和候选算法未替换。

固定R4的123,544条runtime、24探针×12配置×30配对、5次预热和原预算，8,640对共17,280次被测查询完整图/结果相等，288组历史已观测字段无差异。Reference仍8完整/14预算/2拒绝，标准Exact16完整/6预算/2拒绝，PositionDag标准及resident128均22完整/2拒绝，Beam1/8/64分别剪枝16/16/15例。[175组失败/预算/剪枝索引](https://github.com/WanFengQC/Cloudleaf/blob/08b09277d3beeb90e284145a3423b9f00eca8d6c/docs/reports/cl008-round5/failure-and-budget-index.json)及12配置原始stdout/stderr、逐样本耗时全部保留。参考未完成时完整算法等价仍未知，不把同谱系一致当独立语言质量。

阶段区分normalize、lattice、validate、traversal、桶/最终排序、上下文重排、物化及总时，未进入为null。桶排序嵌套在遍历，阶段分位数分母不一定相同，不能相加或相减伪造净CPU成本。标准Position的配对宿主plain总体P95为5.8861ms，Reference/Exact仍有大量预算失败；没有解除资源门禁。宿主峰值约185.5MiB包含预载词库、双图、双结果和累积JSON，不是单算法内存，也不构成优化收益证据。Structural矩阵没有运行正式上下文模型。

新`check_quality_preparation.py`在原R4门禁之外检查逐来源artifact/lineage、共同预处理、protocol/review的全来源覆盖和规范lineage/unit跨split泄漏，保留全局near组与来源内unit规则。实际缺合法独立gold、人工双审/仲裁、家族语义审核、封存heldout和独立保管者，两个实际门禁均exit2，Top-1/Top-5/MRR保持null。字段/SHA核验不能证明人工身份、权限真实性或语义去重；不读取heldout调参。未交付纠错、模糊音、简拼、双拼、学习、增量或默认核心接入。

## C：CL-009完整隔离审计与数据模型

[方向报告](https://github.com/WanFengQC/Cloudleaf/blob/c21db7ca70e5f51f84ca8bd30944d6722473c32c/docs/reports/cl009-round5.md)列出设计`aca20b3`、测试`cbf0f3c`等先行记录及实现`6d3d4e1`。新增只读`audit_isolations.py`和独立`linguistic_sidecar.py`、14项Python回归、来源决定和人工IME评估计划。既有生产解析器、音节表、schema2/3、TSV v2、导入/发布/恢复代码和B冻结词库均未更换。

最终新包逐行重审确认125,012=123,544接收+1,468隔离，原763/704/1类拒绝数量不变。逐条保存原行、原读音token、行号、SHA和未裁决状态：包括儿化后缀、标点/无调表达、连接声调串、成音鼻音、表中缺项、未指定读音和运行时预算等。分类说明运行时表达差距，不裁决普通话合法性，不静默丢弃或自动扩表。派生原文审计按CC BY-SA4保留署名/许可/修改说明，工具另为MIT。

频率侧包区分token比例、DF文档计数、每十亿token的Zipf；未观测/截断保持null，真实0需真实0计数，正计数下溢不能伪装零。派生Zipf必须同词/源/语料/时期/分词协议的正token母观测。POS/tagset、稳定义项、有向多对多翻译、短语组件及双审/仲裁只是可验证模型；当前只有自编fixture，没有真实生产注释侧包，123,544条频率/POS仍unknown/null。

实际元信息核验9成功、2失败，原时间、URL、状态和字节SHA全部保留。THUOCL只保留DF候选；wordfreq中文组成来源未闭合；SUBTLEX独立包再分发范围未解决；Wikipedia固定20261001仅选为后续自产token统计的条件候选，还需完整原件/逐页许可和例外/分词与代表性。没有下载这些候选语料/频率/heldout包，没有新批准频率来源锁。Tatoeba和语法页检索失败未伪装为成功。

`6d3d4e1`精确git archive、新无pip隔离环境、禁止构建网络的双build/verify九文件逐字节相同；新manifest SHA`5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5`。相对R4只manifest因工具指纹改变，不声称跨实现九文件全同。新包七点实际子进程发布/回滚中断恢复、原型1000次及83种子结果见方向原始诊断。七个中断均按预期exit73，恢复后再次激活/回滚/校验通过，用户词典探针SHA不变；整批380.658秒。它们只模拟进程中断，没有做物理断电。原索引诊断工具只保存10,000次查询的分位数与计数，没有逐查询数组；原始工具摘要完整保留，不能据此重建全样本。发布许可末尾空行引发的实际空白检查失败另存原文，改用可无损恢复的许可/署名封装，不修改真实包或豁免检查。

初始缺模块红测有精确提交；两个后续分类/下溢真实失败发生于未提交草稿，未保存完整旧源码字节，只能作历史诊断，不能事后编造精确旧版本。最终实现、69项Python及全部数据重放则绑定6d3d4e1精确归档。C侧包及人工计划没有自动兼容B preparation，未来仍需逐源artifact/lineage/preprocessing/protocol/review映射及真实独立人工材料。

## 身份、保全与退出条件

|共同身份|SHA-256|
|---|---|
|固定CC-CEDICT ZIP|`fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`|
|B旧冻结包/C新包共同runtime|`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`|
|A被测source-v3|`8151e6aa5ff2ac10fc46b373d44d8b92f44e2c4864444965c4e5058fa139795a`|
|B被测source-v2|`b62f6f8bcb6420bbb4a0ee8ba0cab64d413b8b0e8176325c5d075a8ec3ccb1ef`|
|B阶段宿主exe|`67100ac583e788acd8442cdf089dde98592ee1bf2593ee500d7752c43c72e743`|

不同指纹schema/范围不直接比较。更多数据、源码归档、工具/协议、产物身份和完整具体修改列表分别在方向报告与[收尾产物复核](cl007-009-round5-shared/direction-artifacts.json)、[方向文件清单](cl007-009-round5-shared/direction-changes.json)中。共享只新增本轮计划、状态、原文证据/CI和路线入口，不合并方向代码或修改共享接口。

[历史保全](cl007-009-round5-shared/preservation.json)核对起点共享47/A116/B118/C56份报告和64条词典/41事件/83种子的Git clean差异；没有覆盖Round1–4，没有删除旧测试或核心文件。Windows检出CRLF与Git LF分别处理。独立复核A124/B72/C23发布件的Git blob SHA及可重建原始UTF-8文件；原件/规范展示件各自有SHA，不能混为同一字节身份。

|方向|完成与仍需的退出证据|
|---|---|
|A 部分完成|纯监测资源回收、指定异常占用下的恢复/COM隔离通过；任意异常退出、永久卡住/外部引用与未析构Host仍可能耗尽额度，部署级隔离策略、8h/24h机制及至少8h真实应用验收未完成。准入仍拒绝。|
|B 部分完成|观测/差分/资源原文与前置契约完成；独立人工gold、真实train/dev/heldout语义审计、现代频率/正式模型与质量资源门禁未过，不选生产算法，不替换默认核心。|
|C 部分完成|隔离审计、模型验证、固定包复现及子进程恢复完成；现代频率权利和原件、真实语义/翻译/短语、独立人工评测、用户词典迁移/签名更新/物理断电持久性未完成。|

先审PR #1共享契约；它合并后仍须重新核对三方向基底、实际差异、冲突及新精确提交CI。保持四个Draft、不合main。只有各方向分别满足退出条件，才评估CL-010；本轮不放行。
