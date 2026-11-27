# Round 28 自动中英混排

自动混排仍默认关闭。开启后，完整中文读音锚点与通用英文字形共同决定分界。词条覆盖完整的拼音片段不因新增英文证据被切开。中文段仍沿用原搜索方法、排序与预算，提交失败、修订过期和中文词条学习的状态机复用原有实现。

## 68 条冻结研发轨迹

释放构建使用相同的 Round 27 源码基线 `a5675b0e88fb4ea9f0e88061350f4a255beb98c5`、词典、lexical 研究模型、搜索配置、探针和事件序列。对照端只替换 `crates/cloudleaf-input/src/mixed.rs`。两端均采用 PositionDag、Lexical、Top10，输入和搜索字母各限定 64 字节。固定轨迹为原创工程例，独立人工语言指标仍为 `null`。

| 样本 | 数量 | 基线 Top1 | 对照 Top1 | 基线 Top10 | 对照 Top10 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Round 27 | 16 | 14 | 14 | 15 | 16 |
| Round 28 | 52 | 39 | 41 | 47 | 50 |
| 合计 | 68 | 53 | 55 | 62 | 66 |

- `check` 整句由 Top10 外进入第 2 位。原字面区间为 `[9,13)`，修复后的 `[6,11)` 正好覆盖英文。首选仍受右侧中文同音词排序影响。
- `meeting` 的完整英文区间 `[8,15)` 在两次都保留，目标仍在第 7。拆段复核表明右侧目标 `几点开始` 单独排名第 4，仍需中文上下文排序证据。
- 其余变化为两个专名从 Top10 外到第 1、另一个从 Top10 外到第 2。初版大小写字形规则把紧邻 SKU 的中文吞进字面段，日志 `round28-mixed-frozen-coda-replay` 保留了这次回归。将该字形规则限定为非缩写的大写首字母且右侧有完整中文锚点后，SKU 轨迹恢复到第 1，最终对照没有已观测的名次倒退。
- 两条仍在 Top10 外的轨迹均涉及数字后尾部句点，保留了全部原文字节。未在核心猜测数字与句点是否形成小数、版本号或中文句界。以本机宿主传递到核心的实际按键为准，领先 `/` 的路径首键可通行给宿主，未在本轮模拟原生应用的这一交互。
- 68/68 显式原文确认及失败后新回执重试保持文本一致，新增的 6/6 编辑轨迹拒绝旧修订。自动首选产生中文学习证据的次数从 38 次变为 37 次，显式原文确认均无学习证据。首选提交正确次数为 53 和 55。显式原文路径在候选页没有展示原文的情况下也经过验证。此结果覆盖核心事务与学习证据，Windows 上屏及长期学习持久化继续等待原有门禁。

扩展样本组包括独立英文 5、专名 4、混排 14、型号 4、SKU 2、数字 4、邮箱 1、URL 2、路径 3、代码 3、标点 2、拼音歧义 2、编辑 6。每条按键轨迹输出排名、候选来源、候选 ID、中文与原文字节区间、提交保护、修订及学习条目数，仅保留 Git 外结果。

| 单次 Release 回放 | 键数 | P50 / P95 / P99 / 最大值，毫秒 | 触及预算 |
| --- | ---: | --- | ---: |
| 基线 | 933 | 0.201 / 1.853 / 2.770 / 3.503 | 0 |
| 对照 | 933 | 0.108 / 0.813 / 1.309 / 2.601 | 0 |

墙钟数值是本次事件观测。不同运行时刻的构建及测试负荷不同，不能使用这两次回放推断加速幅度。分配、内存与真实窗口延迟未在该专项中测量。

## 重放与材料身份

| 材料 | SHA-256 |
| --- | --- |
| runtime 词典 | `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431` |
| lexical 研究模型 | `36dc4d9057bc70e3494fe397da09e8ab6f187c435bd735b66a7734f9d414fce5` |
| Round 27 固定轨迹 | `f69d2a303612b19ab20c9f8015c66bca561ddfa56540042dac9060a0d5119b4b` |
| Round 28 固定轨迹 | `1fbca98904b72fa820ca25c5900e714c054a67cf826fc490cd133d91b1849730` |
| 核心分界实现 | `2310d73b5bad6eb21afddaa8227101e8e5f877c5bd4a47bf792276b88609b82a` |
| 回放示例工具 | `0c882c4af8e8e2ec65a8af02ac186018b390e0bd25aa20ae1b71d9949dad0e7d` |
| 基线 Release 可执行文件 | `87e90096dfba710b9a8065cc7e90c57110ba06d0b220bb72f98fbe55f298ada1` |
| 对照 Release 可执行文件 | `38f73c4a6ad4aa3eefee37af3a5c75fbf989d4cb256723fdefefc15e9656521c` |
| 基线原始 stdout | `829af62f733c66920f5145be103c6ca90514cc495b04e3734dd5f80b1f8985a2` |
| 对照原始 stdout | `737f9d972d1690cfeaaf7c81290a553bae435a09ff1deb55703c050c1c351fda` |

`cargo build --locked --offline --release -p cloudleaf-input --example mixed_input_trace` 两端均返回 0。`mixed_input_trace.exe --round28 FIXTURE RUNTIME RUNTIME_SHA MODEL MODEL_SHA` 两端均返回 0。额外 52 条固定原件位于工作区 `.local/r28-mixed-dev/mixed-input-round28.tsv`，未随仓库发布。精确产品构建额外重放 68 条，除事件墙钟外 1,676 行回放相同。记录位于 .local/round21-runs/round28-mixed-exact。两份原始回放及命令回执位于 `.local/round21-runs/round28-mixed-frozen-base-replay` 和 `.local/round21-runs/round28-mixed-anchored-replay`。独立对照构建的源码身份及命令记录在 `.local/r28-mixed-dev/baseline-src/.local/round21-runs/`。

`cargo test --locked --offline -p cloudleaf-input --test mixed_input` 返回 0，9/9。新增两个机制回归覆盖英文韵尾、完整拼音负例、专名、缩写以及学习或回执隔离。旧实现的第一个回归返回 101，真实拆段不符。原始失败在 `.local/round21-runs/round28-mixed-red-test`，本轮定版专项在 `.local/round21-runs/round28-mixed-anchored-tests`。
