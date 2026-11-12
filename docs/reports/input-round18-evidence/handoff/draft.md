# Round18 只读最终证据复核摘录

本文件供主代理整合，未修改实际工作区、未执行UI、未提交、未运行新增测试。

性能实现提交 `b57f61b22a47f818146c13c4a3d909477e38c616`；当前13项完整回归绑定 `cd16d0d9a8b8827ebcfd465440b370113fe4c927`。性能记录的17个源码/锁文件哈希逐项仍与当前字节相同；Windows按钮变更不在这组核心性能输入内。

## 完整回归与差分

- 最终本地13条命令均rc=0；Rust 254 passed、0 failed、2 ignored；Python 71 passed。原始命令、返回码、时间、源码/日志哈希位于`.local/round18-runs/verified-01..13/result.json`及相邻stdout/stderr。
- 新方案：1395输入×4方案×6ranker，33,480个完整Reference查询、83,700个完整Candidate比较，最终0差异/0不完整/0Beam剪枝；PositionDag上下文不支持16,740次为明确拒绝，不能计为完成比较。
- 默认字符模型旧入口：22,320个完整Reference查询、44,640个完整Candidate比较，最终0差异/0不完整/0Beam剪枝。
- 原始592个差异全部保留，不能报告“首次即通过”。初轮另有49图预算、294Reference不完整、735优化比较不完整。后续修复的是解释候选完整edge provenance确定性及纠错可达性，而非删断言。
- `r18-final-audit/preservation.json`：2793个既有报告/数据/测试/许可路径保留，230个旧Rust测试函数未删除；必要旧测试只加Budget穷尽分支。

## 相同默认事件配对

每次171个事件×5个新进程=每模型每版本855个样本；15组输出比较全部一致。只移除严格零值/None的新字段，正文、ID、路径、评分、统计不归一化。无warmup丢弃。

|模型|旧P95 ms|新P95 ms|变化|旧P99 ms|新P99 ms|
|---|---:|---:|---:|---:|---:|
|lexical|4.2114|3.8933|-7.55%|5.9277|5.2491|
|addone|4.9480|5.3363|+7.85%|11.1612|12.7359|
|witten|5.5452|5.9186|+6.73%|11.8701|12.1073|

普通桌面研发测量，可能与轻量自有窗口操作并发；不是专用低延迟验收。字符模型P95约+7.85%/+6.73%，不得声称零开销、全面提速或严格5ms门禁通过。

## 新方案逐键延迟

|方案/模型|n|P50 ms|P95 ms|P99 ms|max ms|
|---|---:|---:|---:|---:|---:|
|full/lexical|420|0.1621|2.1245|3.2879|4.0325|
|full/addone|420|0.2088|1.8573|4.3684|10.2307|
|full/witten|420|0.1731|1.9958|5.3246|6.5835|
|abbreviated/lexical|420|6.5386|25.2546|31.5030|33.7253|
|abbreviated/addone|420|15.3052|22.9495|26.3408|34.9643|
|abbreviated/witten|420|14.3999|18.8248|21.5855|24.1881|
|xiaohe/lexical|126|0.5123|3.4017|4.3235|4.3241|
|xiaohe/addone|126|0.4232|6.0592|13.6949|14.1855|
|xiaohe/witten|126|0.4584|5.8857|12.6405|13.5241|
|fuzzy-nl/lexical|420|0.4641|2.6189|3.5425|4.6748|
|fuzzy-nl/addone|420|0.4760|7.1153|14.0540|16.7864|
|fuzzy-nl/witten|420|0.4318|6.8815|11.9228|14.6079|
|correction/lexical|420|1.0639|5.8981|12.5943|18.1460|
|correction/addone|420|1.3496|14.9875|21.3384|54.1398|
|correction/witten|420|1.1191|13.1466|17.0993|19.0732|

全部120个latency/profile进程rc0不等于全部查询成功：837个最终profile查询中90次预算失败，30个唯一方案/模型/案例×3轮；均为简拼+addone/WittenBell，分别15/20案例，Expansions=200000。InputEngine遇预算不提供局部选择，原文保留。中间EVENT只打印error，预算分母仅指最终profile输入。

阶段观测必须单列：plain query和profile query结果相同，但profile包含计时开销；scoring/bucket统计嵌套在traversal内，不能把所有阶段相加。简拼主要耗时在遍历/评分，交集其次，音节解释通常更小。具体全表见`r18-perf-final/phases.md`。

## 进程资源

下表是独立15个新进程、50ms采样及OS峰值。包含词典/模型原件读取缓冲、不可变索引、搜索、帧和诊断。不是单算法分配，不是Host/COM，不是128MiB内部预算的RSS上界。

|方案/模型|采样数|采样working最大MiB|采样private最大MiB|OS working峰值MiB|
|---|---:|---:|---:|---:|
|full/lexical|15|184.69|185.26|188.45|
|full/addone|21|211.38|212.89|211.45|
|full/witten|24|208.24|212.58|211.12|
|abbreviated/lexical|40|187.27|194.02|188.57|
|abbreviated/addone|54|208.81|212.67|211.19|
|abbreviated/witten|52|207.35|208.89|211.19|
|xiaohe/lexical|14|184.75|185.29|188.59|
|xiaohe/addone|20|205.62|208.55|211.09|
|xiaohe/witten|22|207.37|208.89|211.19|
|fuzzy-nl/lexical|18|184.73|185.29|188.43|
|fuzzy-nl/addone|23|205.56|208.17|211.23|
|fuzzy-nl/witten|27|207.29|208.92|211.26|
|correction/lexical|20|184.75|186.20|188.39|
|correction/addone|31|205.78|208.90|211.19|
|correction/witten|32|206.30|208.88|211.19|

## 真实候选观察和未放行项

- 简拼lexical：zgr首选撰稿人、中国人第3；bjdx首选北京大学；混合zhonggr首选中国人；nh首选内涵、你好不在Top10。不得把非空率或这些例子当中文准确率。
- 小鹤nihc、vsgorf、bwjkdaxt分别形成你好、中国人、北京大学；lo碰撞保留lo/luo音节路径。完整Top10、Features、ID、原始证据保留在profile stdout。
- 正式Top1/Top5/MRR全null，独立gold未交付。新方案仍opt-in，不替换默认核心。简拼字符组合预算失败、P95超过实时目标；当前仅研发partial。
- 微软/自然码未核实完整原始规则而未实现；小鹤+纠错拒绝；模糊只从完整精确弧扩展；物理键盘当前ASCII/v，纯API的ü能力不能冒充实际Unicode键盘验收。
- 无TSF、真实跨应用输入法、长期压力、学习持久化或正式语言质量验收。主代理需把实际窗口按钮和Unknown测试另行绑定各自二进制SHA。

## 原始证据和身份

`r18-perf-final/index.json`列全部文件SHA；120个run的command/result/stdout/stderr保留；stdout原件和gzip均在，`compression.json`验证CRC和解压后逐字节SHA。资源日志另在resources。差分3组压缩日志均有CRC及原始字节SHA。

- 词典runtime SHA `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`。
- 旧input_trace二进制 `fba227f97a8dfe6df65e4a97e11f356c6691af40563f74332ba0ed8f29197a53`；新 `add07d004c458aa811141920a1fa5589e142ed6a7ca418abb432a784c0971830`；scheme_trace `b163b35e6b29fe01d2f881b6d18ede8ef398e34a83b8a4d53de7310a5915f723`。
- 性能工具run.py `e51d81569ab8bc4dbdc5084f80c6171fdeba22169b2d65adf646a98c690d3cff`；summarize.py `08b15233323e2512b281bf3a041eb2723f06d4dd5e036eae476babc3d3c1cea0`；resources.py `72de6b183f5299a01e36b3538613a154802eeb84d34f34abb276f95e028cc25f`。

所有完整绝对路径、工具/数据/二进制SHA和原始指标见相邻handoff.json。本摘录未核对最终push CI；需要主代理单独完成精确新HEAD两平台CI。
