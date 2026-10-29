# CL-008 第四轮：前置门禁、独立受限参考与完整词条资源

2026-10-23。**部分完成；没有正式语言准确率、默认引擎接入或CL-010放行。** 本轮实现前置门禁和独立Python受限穷举，另消费C新交付的固定完整授权镜像词条包。原搜索算法、默认引擎、共享接口、原型64/41/83和旧报告均未改。

设计先行 `db6fa75`/`f2a04d8`，镜像消费设计 `5468d9b`；最终被测源码 `7d1a0fcaaa1302a8c88e60b0f1aafbf6228e69d1`，源码SHA `d8bb7af84e4e42b86f3cf4f6c617f713042406f286b4532eb160df8b021d1397`。完整命令/真实返回码见[validation](validation.json)，原文日志及文件、二进制、协议、探针和数据SHA见[provenance](provenance.json)。

## 正式评估门禁：明确拒绝

[当前实际门禁](language-readiness.json)返回 **2**，未被写作成功评估。既有协议/70/15/15/heldout≥2000家族门限核对通过，来源、独立人工gold、家族清单、封存/保管者和参数冻结仍缺前置材料。70/15/15及2000是既有预登记要求，不是当前已经满足或统计最优。

工具核对来源批准文件/许可/产物SHA、人工标注来源声明、家族/原文段落/近重复/规范输入/任务摘要隔离、旧41/83已见输入重叠、封存与参数/源码/二进制/词库绑定；可选预测必须覆盖heldout全部案例恰好一次，预算/拒绝/未支持不得被删除。协议文档以Git UTF-8/LF字节身份绑定，数据和二进制按原始字节核验。

**没有打开封存答案或计算Top-1/5/MRR，所有语言指标为null。** 来源批准、人工独立性、近重复语义及保管者/加密真实性仍需人工审核；工具不会把自填声明变成独立证明。当前登记计划尚未建立正式实验来源，不表示已有授权字符/镜像词条被否定；完整词条资源实验与人工gold前置条件分别记录。

7项新门禁测试使用明确标为fixture的临时声明与13,340条机械摘要验证通过/篡改/泄漏/分母语义；这些数据从未写入正式计划，不是语言gold。另4项独立参考语义测试覆盖分段/打平/前缀/Unicode/等待/拒绝及耗尽。

## 真正独立的受限结构参考

依先提交规则，从原冻结Unihan全资源选固定13读音各2条，原ID升序、共26条，未读取Rust候选作选择。独立Python直接逐词匹配并枚举全部路径，未调用Rust Lattice/Reference；14个固定探针全部保留，13个合法探针完整、1个非法拒绝保留，累计枚举148条完整路径（包含同文多路径），空串/等待/未命中均在分母。

对Reference/ExactDag/PositionDag逐rank比较text、整数score、完整IDs、features、records和状态/等待/回退，完整参照下0差异；非法拒绝的完整等价结论为null。Beam8/64在该小图没有剪枝，不能据此选择生产宽度。

Beam1在9例发生可观察候选丢失：`ana / anana / xian / xi'an / shishi / zhongguo / beijing / xuexi / zhongg`。扩展1和驻留1的四个精确策略压力配置各保留10例预算失败，完整等价为null；未放大预算或删除失败取得通过。详见[受限完整结果](restricted/results.json)，原子词库/探针/[来源证据](restricted/source-evidence.json)和Unicode许可均在同目录。

**该实验是dictionary-derived resource subset，不是人工语言gold。** 它解决这些受限图的跨实现核对，不解除R3完整44,431条资源上15例Reference耗尽的边界；不把同谱系Rust参考叫独立证明。R3固定13例P95>5ms、短例退化和Beam1的4/4/5/5损失原样保留，见[上轮报告](../cl008-round3/README.md)。

| 受限配置 | 完整/14 | 预算失败 | Beam剪枝例 | 最大单例P95/P99 ms | 进程峰值MiB |
|---|---:|---:|---:|---:|---:|
| independent-python | 13 | 0 | 0 | 0.36710/0.37830 | 20.84 |
| reference | 13 | 0 | 0 | 0.07360/0.10150 | 4.83 |
| exact | 13 | 0 | 0 | 0.06500/0.08560 | 4.94 |
| position | 13 | 0 | 0 | 0.04770/0.05120 | 4.80 |
| beam1 | 4 | 0 | 9 | 0.02140/0.36160 | 4.82 |
| beam8 | 13 | 0 | 0 | 0.07300/0.09860 | 4.93 |
| beam64 | 13 | 0 | 0 | 0.10110/0.11170 | 4.97 |
| exact-expansions1 | 3 | 10 | 0 | 0.00270/0.01650 | 4.76 |
| position-expansions1 | 3 | 10 | 0 | 0.00200/0.00900 | 4.75 |
| exact-resident1 | 3 | 10 | 0 | 0.00200/0.02700 | 4.77 |
| position-resident1 | 3 | 10 | 0 | 0.00660/0.01750 | 4.77 |

计时均5预热/30样本。Python与Rust进程峰值含不同宿主运行时，不能用二者比值声称产品收益。没有剖析规范化/建图/搜索阶段占比，后台负载未隔离。

## 固定完整CC-CEDICT镜像资源

C从干净归档 `2401b1640d37b17ea4112aeb51862fb0ddba41d1` 构建/verify交付，B核九文件SHA/bytes后独立冻结副本：**123,544条记录，其中110,045条多字记录，频率全部unknown，POS缺失仍null**；完整英文义项保留records sidecar，241条runtime注释省略未冒充完整双语释义。runtime SHA `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`。

输入为CC-CEDICT/MDBG上游数据的CC BY-SA4获授权固定公开镜像，提交`7019e1c3ec3de3a44b165b15177ce2bb39a026d3`、来源日期2026-06-04；**非最新官方版本、非发布者签名**。保留[NOTICE](cccedict-source/NOTICE.txt)、[许可正文](cccedict-source/LICENSE.txt)、[来源锁](cccedict-source/source-lock.json)、[上游审查记录](cccedict-source/upstream-documents.json)、[构建清单](cccedict-source/manifest.json)、[质量报告](cccedict-source/quality.json)和[接收清单](cccedict-source/received-from-c.json)。候选/派生文本继续适用CC BY-SA4，工具代码另为MIT，未包含镜像解析器或CC-Canto数据。

逐配置完整载入全部123,544条，使用原24无gold探针、原12配置、30测量/5预热和原所有限额；unknown不补0，Structural不训练语言模型。[原始结果](cccedict-full.json)与[逐rank/分组核对](cccedict-analysis.json)保留全部候选、原始样本、来源、预算、剪枝及完整性。

| 完整资源配置 | 完整/24 | 非空/24 | 预算例 | Beam剪枝例 | 混合P95 ms | 最大单例P95/P99 ms | 峰值MiB |
|---|---:|---:|---:|---:|---:|---:|---:|
| reference | 8 | 19 | 14 | 0 | 179.31380 | 225.80850/257.43830 | 185.49 |
| exact | 16 | 13 | 6 | 0 | 176.80290 | 217.30440/232.07820 | 185.50 |
| beam1 | 6 | 19 | 0 | 16 | 1.44880 | 3.08350/3.14660 | 185.34 |
| beam8 | 6 | 19 | 0 | 16 | 12.74870 | 20.96230/25.75590 | 185.36 |
| beam64 | 7 | 19 | 0 | 15 | 72.48820 | 127.26040/139.62430 | 185.48 |
| exact-edges128 | 4 | 11 | 18 | 0 | 3.24100 | 11.62740/12.08820 | 185.39 |
| exact-expansions500 | 6 | 3 | 16 | 0 | 0.52520 | 0.64270/0.74150 | 185.34 |
| exact-resident128 | 6 | 3 | 16 | 0 | 0.14320 | 0.22290/0.67820 | 185.32 |
| position | 22 | 19 | 0 | 0 | 7.85910 | 16.15100/16.88930 | 185.47 |
| position-edges128 | 4 | 11 | 18 | 0 | 0.36070 | 0.72930/0.73080 | 185.46 |
| position-expansions500 | 6 | 3 | 16 | 0 | 0.67090 | 0.84850/1.36080 | 185.43 |
| position-resident128 | 22 | 19 | 0 | 0 | 8.97670 | 16.79100/16.95830 | 185.42 |

exact 对完整同谱系Reference：8例一致、0例差异、16例null。

position 对完整同谱系Reference：8例一致、0例差异、16例null。

8例完整Reference为empty、short-shi、short-zhong、short-lv、word-zhongguo、prefix-zhongg、waiting-zhong、unknown-zzzz，包含空/等待/未命中；不当作8条中文语言正确答案。另8例Position对完整旧Exact一致，仅为同谱系补充核对，不增加独立穷举证据。

标准Position仍有7例P95>5ms：word-beijing、word-xuexi、word-jisuanji、word-ruanjian、sentence-beijing、long-xian、long-shi；3例P99>10ms：word-ruanjian、sentence-beijing、long-xian。Reference的14例及标准Exact的6例预算失败全部保留，不能凭Position有输出就宣布产品响应或算法质量达标。

混合样本分位数与最大单例分位数分开。拒绝和预算仍在分母，快速失败不解释为完整搜索更快；进程峰值为每配置整体，不能派生成每例内存。不同词库与R3的差异不是算法单因素因果对照。当前没有人工答案，非空候选或整词命中不叫中文准确率。

## 检查与交付边界

74 Rust all-targets、doc、50 Python（原39+11新测试）、locked fetch、离线debug/release构建、fmt、Clippy -D warnings与资源审计全部通过。当前门禁的退出2是保存的明确拒绝，并未伪装返回0。机械100/10实验重新运行，Beam1原4/4/5/5损失保留。

原始样本合计34060条，分项 {'mechanical': 20800, 'full_cccedict': 8640, 'restricted_rust': 4200, 'restricted_independent_python': 420}；完整配置、源码、二进制、数据、探针、协议及证据指纹分别保留。目录按UTF-8/LF发布，provenance记录原件→发布件双SHA；本地原件和R3均不覆盖。

- 完成：前置证据拒绝门禁、泄漏/冻结/完整分母测试、跨实现真实受限穷举、完整授权镜像资源矩阵与失败轨迹。
- 部分：完整词库搜索机制与工程资源边界；完整性为null的案例未取得穷举结论，同谱系参考不等于独立证明。
- 未完成/阻塞：合法独立人工gold、正式train/dev/封存heldout及保管者、现代词频/模型、正式双语对齐、纠错/方案/学习/增量、默认输入接入及A/C退出条件。保持Draft PR #2，不合并main，不进入CL-010。
