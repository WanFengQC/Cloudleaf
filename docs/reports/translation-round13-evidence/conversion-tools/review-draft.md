# Round 13 实装开发转换环境许可证归档

2026-11-02。只读检查 `.local/round13-conversion-env`，不导入Torch、不下载、不安装、不修改项目工作区。所有原文按字节复制，未改编码或换行。采集脚本在归档目录的上一层 `collect_conversion_licenses.py`；执行入口是该环境的 `Scripts/python.exe -X utf8`。

## 实际结果

- 15个实装distribution；62份实际许可证、NOTICE和作者文件，共879,689B。
- 每包同时归档实际METADATA、WHEEL、INSTALLER、RECORD及存在时的direct_url.json；两个实际pip安装报告单独保留。
- 所有metadata声明的License-File都在归档原件中找到；归档时对带RECORD指纹的许可文件核验，失败数0。这不等于对整个环境所有二进制进行完整RECORD验收。
- `manifest.json` SHA-256：`8ba82875c8c944c46d077dd32e061e6032d0eb3468211de671c792042ff3a2ff`。逐文件来源相对路径、字节数、SHA、RECORD校验和每包版本见此清单。
- 14个distribution有实际pip报告或前次固定wheel来源；pip25.0.1来自环境建立阶段，没有本任务可核验的原始wheel取得记录，不能虚构其下载SHA。

## 实装版本与许可边界

| 实装版本 | 原文结论与边界 |
|---|---|
| torch2.10.0+cpu | 主许可BSD-3-Clause；实际wheel的544,783B聚合LICENSE和24,088B NOTICE均保留。聚合文本含MIT、BSD-2/3、Apache-2及例外、BSL-1.0、Public Domain等项目，不能将整个wheel概括为BSD。清单出现GPU项目名称不代表该CPU环境启用了CUDA。 |
| safetensors0.7.0 | Apache-2.0；实装原文已保存。 |
| numpy2.4.2 | 源码表达式为BSD-3-Clause AND 0BSD AND MIT AND Zlib AND CC0-1.0；Windows二进制的LICENSE另列OpenBLAS BSD-3-Clause、LAPACK BSD-3-Clause-Open-MPI、GCC runtime GPL-3.0-or-later WITH GCC-exception-3.1并附完整原文。20份相关原件包含各随机数/数学实现，不能只写BSD。 |
| packaging26.0 | Apache-2.0 OR BSD-2-Clause；两种许可及选择说明均保留。 |
| sentencepiece0.2.1 | 该实装官方wheel没有独立LICENSE文件、License-File声明或License表达式；不伪称wheel已附许可。补充此前缓存的官方固定commit `31646a467d2051eb904e0b45de3a73e91fe1c1e3` Apache-2.0 LICENSE，SHA `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`，位于`supplemental-upstream`。其来源身份独立于wheel；随产品分发该wheel仍需重新审核实际内容及相应归属。 |
| filelock4.2.4 | 实装声明MIT；不能沿用早期候选filelock3.20.0的Unlicense结论。 |
| fsspec2026.9.0 | BSD-3-Clause；未安装http/s3等extras。 |
| networkx3.7 | BSD-3-Clause。 |
| Jinja2 3.1.6 / MarkupSafe3.0.3 | BSD-3-Clause；Pallets原文保留。 |
| sympy1.14.0 / mpmath1.3.0 | BSD主许可；SymPy实际原文包含附带软件条款和作者，另有latex MIT文本，不能用单个metadata字段替代原文。 |
| typing_extensions4.16.0 | PSF-2.0，保留相关历史条款。 |
| setuptools78.1.0 | MIT主许可；24份实际原件包括vendored Apache/BSD/PSF等及config NOTICE，不等同全部MIT。 |
| pip25.0.1 | MIT主许可与作者清单原件；本任务不声称已重新审核其全部内嵌代码，亦不把它当产品运行时依赖。 |

## 使用与交付限制

这些工具仅用于Git外开发转换和官方tokenizer差分；最终Rust面板与离线推理不调用Python，不随产品包携带Torch/NumPy/SPM官方wheel。原始模型权重、转换产物、Rust运行时和开发软件的授权彼此独立；软件许可归档不解决模型训练来源、正式译文质量或权重再分发门禁。

本任务没有运行Python生态漏洞扫描。Cargo RustSec成功仅覆盖其报告中的Rust依赖，不能据此宣称这个Python环境通过安全审计。若未来需要重发开发环境、wheel或工具安装包，必须按实际完整随包内容重新检查全部第三方许可与通知；当前归档不是 blanket redistribution approval。
