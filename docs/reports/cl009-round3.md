# CL-009 第三轮：固定来源复现、恢复重测与正式数据审查

2026-10-23。被测实现为 `ed9516bf74e36649b0dc415850c65792732c10d4`；本轮不修改生产代码、来源锁、运行时格式、原型词典或A/B接口。新增[正式来源候选矩阵与质量门禁](../cl009-formal-data-review.md)先提交于 `3d1adf8`，其后只追加测试与报告。正式词库、频率、双语及独立语言质量仍未验收，CL-009不关闭。

## 固定输入与独立复现

[完整证据](cl009/round3-replay.json)保留原件/产物摘要、解释器身份、导出、每条命令、stdout/stderr和时间。沿用已有缓存，先按可信来源锁核对原件及许可，没有重新下载或把残片标为完整文件。

|输入|字节数|SHA-256|
|---|---:|---|
|完整Unihan17.0.0 ZIP|8,518,517|`f7a48b2b545acfaa77b2d607ae28747404ce02baefee16396c5d2d7a8ef34b5e`|
|固定Wiktionary压缩前缀|8,388,608|`fdf3ae6918edb0e6af91cad20dfb51a2ecad8ce59ab026b4ab4e1e7f33581518`|
|Unicode许可原文|锁定原字节|`e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96`|
|CC BY-SA4许可原文|20,138|`28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5`|

从被测提交 `git archive` 导出全新 `.local/round3-ed9516b/checkout`，归档344,533字节，SHA `0721cdfe46cb53fa3035101e8751e102ee90dbded9f0068bc30d4332bbd49182`。导出不包含`.git`或旧`.local`。新建无pip venv，Python3.12.10；每条命令独立使用`-X utf8 -I`，移除PYTHONPATH/PYTHONHOME、明确只加入导出副本，检查pipeline路径及第三方distribution为空。数据命令禁止urllib网络访问。

完整Unihan在两个新目录独立build/verify；固定压缩前缀先各自extract-prefix到不同本地文件名，再分别build/verify。提取均得到66,593条完整LF行、46,788,375字节，SHA `ca4df3453261f9f12f1223deb11a26d979078ea6e1af870b44da2d3dcf4e2998`，末尾139字节保持丢弃。原gzip全件仍未完成：全件SHA=null、全件CRC=false，研究范围不变。

|构建|接受/隔离|两次构建时间|九文件一致|manifest SHA-256|
|---|---|---|---|---|
|Unihan字符|44,431 / 18合法但运行时不支持|7.906 / 8.687秒|是|`6012638a09d1a2e8d2d7347ad10c78a357c3b953bba68cff610795fb609770b0`|
|Wiktionary研究词条|701 / 142隔离（94不支持、30歧义、18权利待审）|1.495 / 1.461秒|是|`df137d60c7299d84cdaa262386556bafa4e56103fde2ae592738a31bbe292d6f`|

比较包括records、runtime、index、quality、rejected、source-lock、NOTICE、LICENSE、manifest全部九个文件，不只比较数量或runtime。Unihan runtime SHA为`1b13b199440c0ca9b59074de9100f58c6a0b847b12a7b986e1b752084ff60f8e`；词条runtime仍为`b47287d0fd405c225409653cd8d08f34f9fcb31ef8bb8c9b9e54dbcf5fd7741c`，没有替换B历史冻结元数据。每份verify声明`source_only_statistics_rebuilt=false`；本轮原件重建才证明原始统计可重放，自包含verify不等于重新取得原件。

Unihan缺频率44,431条，研究词条缺频率701条，全部保持null/unknown。研究词条仍521个不同词头、532拼音键，仅216条显式英译；485条缺英语，未推测填补。两来源源坏记录/冲突均0，隔离项仍不冒充源坏行。

## 18条审计与重放脚本中断

同一归档重新计算18条合法源读音，7条成音鼻音/叹词、11条当前音节表缺口、17字符无运行时读音、嗯另有en。逐条证据与[第二轮完整审计](cl009-reading-audit.md)解析为JSON后完全相同；没有扩表或改近似音节。字段语法符合UAX38不替代正式规范/键编码政策，后续决策列入来源质量计划。

本轮确实出现一次**证据脚本断言中断**：初始脚本把新CLI紧凑JSON与历史经过格式化的JSON要求字节相同，前16条数据命令均成功后在该断言停止。新报告SHA为`163864ae1e625ccb3be25825465d6af8c85ccc13748c3ea60fe8fd09342cf2dc`，历史为`b604c9f5fd2938b36f3a6ad630031585908477cdb766c3d5a3ff451ca1f411ce`。差异是JSON格式，解析事实相同，不能把它写成字节相同，也没有产品数据或代码失败。

确认原进程已结束后保留原脚本、16条日志和产物；新补全脚本只读复核各九文件摘要与审计事实，并完成隔离49项测试，未覆盖旧证据。完整报告分别记录semantic identical=true、byte identical=false和两个摘要。没有为制造改动而修改稳定CLI输出格式。

## 真实发布中断与回退

[七点实际包中断证据](cl009/round3-real-interruption.json)在七个全新存储中使用本轮44,431条字符包和701条研究包，保留原第二轮证据。仅对自身Popen创建并持有句柄的写者进程注入`os._exit(73)`，owner锁PID逐项等于实际子进程PID；没有终止其他程序。

覆盖拿锁后、复制中、完整缓存rename后、临时指针写后、current替换后、rollback替换前和替换后。七例均确认旧或新current指向完整包；替换前失败保持旧指针；写者确认已退出后显式recover，随后activate和rollback均成功。每例用户词典探针的SHA在中断/恢复/升级/回退前后不变。

完整49项Python测试同时包含真实Windows junction、硬链接拒绝、元数据/祖先路径门禁、活进程/未知状态拒绝和自建子进程terminate恢复。没有删除或弱化旧测试。上述证据是进程中断和当前工具契约，**没有测试物理断电、磁盘控制器持久性或正式安装器升级**；不声称能对抗不遵守guard的其他进程并发替换路径。

## 本地检查与资源观测

[九条完整本地命令](cl009/round3-local-checks.json)全部exit0：locked fetch、离线debug/release build、fmt、Clippy全部target且warnings拒绝、60项Rust测试、doc tests、工程/资源审计、49项Python测试。干净归档venv另执行全部49项Python测试成功，不复用原模块。资源审计仍为6crate、0外部Rust依赖、4资源（7条来源登记、41事件、64原型词条、83种子），历史基线未改变。

|范围|样本/载入|P50/P95/P99|进程峰值与解释|
|---|---|---|---|
|44,431条字符包Python后台精确索引|10,000次；载入52.8853ms|100/200/200ns|258,359,296字节，含前置完整包校验|
|701条研究包Python后台精确索引|10,000次；载入0.9861ms|100/200/200ns|32,858,112字节，含前置完整包校验|
|64条原型Rust输入核心|79,000事件|1,900/4,800/9,300ns|6,033,408字节；[原始指纹/计时](cl009/round3-core-performance.json)|

测量平台Windows11 build26300、i5-11400、12逻辑核，桌面背景负载未隔离。Python字典精确键查询不测Rust候选/TSF，64条原型不代表正式词库，单轮观测不宣称性能提升。原型源码SHA `a7a33957a0cb796762752310bbbea744bfe63357e4acd074f8109772050686b8`、二进制SHA `d53f54a25bd6b4fb8c114a06966010d4b98bf0597efb7e9c5e897e96c59e5b80`随报告保留。

## 完成和未完成边界

完成：固定两来源双构建全九文件一致、18条事实复核、七个真实包中断点与回退、本地全部检查及11来源候选/质量门禁审查。没有实质生产代码缺陷需要修复，因此本轮只提交可复查的设计和证据。

仍部分完成或阻塞：完整Wiktionary原件和逐页例外；生产词/短语读音与覆盖；具体可商业再分发的现代频率；独立人工IME评测；完整双语；扩展音节政策；签名安装器、跨版本用户词典迁移和断电验收。正式来源计划分别列出关闭条件，没有把候选或研究摘录变为正式批准，`language_accuracy_valid=false`保持。CL-009与CL-010均不因本轮结构/恢复通过而放行。

新提交的精确Windows/Linux CI须在推送后另核验；本报告中的本地通过不沿用历史CI作证明。未合并PR，未改main。
