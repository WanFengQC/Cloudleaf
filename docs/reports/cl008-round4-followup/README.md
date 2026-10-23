# CL-008 第四轮补充：关闭跨来源近重复隔离缺口

2026-10-24。**门禁缺陷已修复；正式语言评估仍阻塞。** 本次只修离线前置检查，不改变搜索算法、默认输入引擎、数据、评分或共享接口，不进入CL-010。

设计先行提交 `4c41ee03d1c2c0d30ff7b3a9573319473843de5f`，见[设计补充](../../cl008-round4-global-near-duplicate-groups.md)。被测修复提交为 `736c517ddb0903dd9dbf10bba5e80ac2ed1783d4`，源码指纹 `cea0d0681c01d7a522811e10c3a12d9abd2e1fb9c699b3dc025f073af7978a77`。本目录后续发布提交不改变该被测源码身份；最新精确提交的Windows/Linux CI另由PR #2及共享状态报告记录。

## 具体修改与红绿对照

- `scripts/check_language_readiness.py`：`near_duplicate_group` 改为整份家族清单的全局键。同一已声明近重复组跨来源仍不得跨train/dev/heldout；`source_unit_id` 保持按来源限定。
- `tests/test_language_readiness.py`：增加跨两个获批evaluation-corpus来源的三个split配对回归，并增加不同来源可使用相同局部段落编号的正例。既有50项Python测试均保留。
- 新增设计与本目录证据；项目README增加入口。没有修改旧轮报告。

在原脚本上应用新增回归，真实出现 **3项失败、退出1**：train/dev、train/heldout和dev/heldout的同近重复组均被错误接受，详见[修复前日志](regression-before-fix.log)及[身份/命令](regression-before-fix.json)。该次HEAD为设计提交 `4c41ee0`，门禁脚本仍与 `909a1b0` 相同；新增回归的精确字节SHA也单独保留。修复后9项门禁测试全部通过，见[修复后日志](regression-after-fix.log)。这些临时人工/许可声明仅为格式fixture，不是真实来源批准或语言gold。

## 完整本地检查与当前拒绝

[完整命令、返回码与原始日志SHA](checks/validation.json)绑定被测修复提交和新源码指纹：locked fetch、离线debug/release构建、fmt、Clippy `-D warnings`、74项Rust全目标测试、doc测试、52项Python测试（旧50+新增2）和资源审计均通过。所有检查期间源码指纹保持一致。[Rust日志](checks/command-05.log)、[全部Python日志](checks/command-08.log)可逐项复查。

当前正式登记计划仍真实返回 **2**，见[门禁报告](checks/language-readiness.json)与[命令日志](checks/command-10.log)；来源、独立人工gold、家族清单、封存与参数冻结材料仍未齐备。`prerequisites_ready=false`、`language_accuracy_valid=false`，Top-1/Top-5/MRR均为null。工具只核对提供的声明和文件一致性，不能证明人工独立性、近重复语义、加密或保管者身份真实。

发布辅助脚本第一次将33个证据文件误作全部历史文件数，遇计数断言退出1；实际另有README与.gitattributes，共35项。该辅助失败及修正说明保留于[发布失败记录](publication-first-attempt.json)，未修改产品源码或重新解释测试失败。

## 历史证据与剩余边界

`909a1b08642d63f206fb2a31bcbe178013cb4cae` 下原Round4目录的 **35项Git文件（33个证据文件及2个辅助文件）逐字节未变**，详见[原Git blob与SHA清单](historical-evidence-preserved.json)。34,060条搜索计时样本继续绑定 `7d1a0fcaaa1302a8c88e60b0f1aafbf6228e69d1` 与旧指纹，未重跑、覆盖或改绑到本次源码。完整镜像runtime仍为 `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`；本次没有新增数据或语言/性能测量。

发布文本统一UTF-8/LF，展示日志去除行尾空格；[provenance](provenance.json)保存本地原件与发布件双SHA。为完整保留实际Windows输出字节，[raw-logs.json](raw-logs.json)封装全部12份原始日志：逐条将`text`按UTF-8编码即可还原并核对其SHA，不依赖本地临时路径。

- 已完成：跨来源已声明近重复组隔离修复、旧版本真实失败证据、全部旧/新测试及当前材料不足的明确拒绝。
- 仍未完成：跨来源近重复语义聚类本身、合法独立人工gold、正式train/dev/封存heldout与保管者、现代词频/模型、正式语言质量及双语对齐、纠错/方案/学习/增量、默认输入接入和A/C退出条件。
- 下一步依赖：由数据与独立标注方先建立可审核的全局家族/近重复清单及合法语料，完成原正式评估协议的封存与冻结；不得用机制fixture补材料或据本次修复放行产品。PR #2保持Draft，main不合并。
