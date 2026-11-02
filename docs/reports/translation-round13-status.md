# Round 13：真实离线中英整句翻译

2026-11-02。**工程路径已实际运行，本轮总验收partial**：两个真实固定OPUS模型经本地Rust CPU推理，在既有Windows面板完成双向整句展示、取消和复制。完全断网、正式独立语言质量及生产权重分发未通过，不能宣称正式离线整句翻译已完成。不进入CL-010。

## 版本与修改

- 基线`89d9cf287652776823db151e59a6e3d6d90e94ca`；设计`debbe3307bd09e148238e2190b8f6f3272524be2`，先有管理/设置失败测试`5e00097`，依赖和转换设计`c8284f1`/`4b3a788`，实现`18a5c0a`，源锁字节修复`1dc9fb80157afa2bfbb643ff598cffe60815dcd8`。
- 最终完整本地回归实现 **`0b41b2578a84c09b2170f34ce38c328cda6978a3`**；token提示失败测试`fd1a777`先于修复。GUI二进制为`1dc9fb8`，后续只改token超限提示、交付说明与CI包，神经推理实现未变。
- 本报告和原始证据另作文档提交。避免自引用SHA循环，**最终完整HEAD、其自己的Windows/Linux push CI和远程复核写入`.local/round13-delivery-receipt.json`及[Draft PR5正文](https://github.com/WanFengQC/Cloudleaf/pull/5)**；不能用实现提交或旧CI替代最终文档HEAD。
- main保持`a564fd47b0f054ff360592af3ccb8d744021160f`，PR1–4的HEAD/状态在交付回执复核。五PR均保持Draft，无合并、强推、rebase、Ready、TSF、安装或长测。

| 文件范围 | 实际作用 |
|---|---|
| `crates/cloudleaf-offline/src/{manager,marian,lib}.rs` | 固定本地模型校验/导入/卸载、实际SPM和Marian CPU推理、状态及资源约束 |
| offline tests、4个examples与README | 入口反例，真实模型管理/推理/生命周期与旧PTH失败诊断 |
| panel `settings/setup/controller/windows_ui.rs`及测试 | schema3独立catalog、后台加载进度、实际能力/整句模式、Generated显示和超限提示 |
| `tools/offline_models` | 固定原件开发转换、离线bundle准备、官方SP差分；运行期不调用 |
| Cargo、9个vendor包、源生成/验证工具和审计政策 | 明确新增依赖、限定源补丁、许可证和安全审计 |
| 模型锁、THIRD_PARTY_NOTICES、CI与本轮文档 | 精确模型/工具身份、保留归属、构建研发工具，无权重打包 |

完整路径见[修改清单](translation-round13-evidence/changed-files.tsv)，全部归档字节见[证据索引](translation-round13-evidence/index.json)。实时输入的input/types/pinyin/candidate/dictionary五crate与R12基线无差异，没有新建翻译调度器或联网客户端。该清单截至实现提交；本轮新增报告和证据另列于Git提交。

## 模型、运行时与实际安装

完整[来源及软件审查](../translation-round13-model-runtime-review.md)区分代码、权重、tokenizer、训练材料和实际分发权利。选择OPUS两个单向模型，M2M100约1.94GB未下载/实测，NLLB非商业约束不作正式默认；不能从体积认定速度/质量最优。

| 方向 | 固定revision | 原件SHA-256 | 转换Safetensors SHA-256 |
|---|---|---|---|
| en→zh | `408d9bc410a388e1d9aef112a2daba955b945255` | `69a1d6ec829cee349360b3b677ac0aa99a7d88822d1a6578370029efffdca3f5` | `e5f4423b70cb0f5a40d38d1a2733a77cec2aa23cc0bae647985fe17f8f5fe616` |
| zh→en | `cf109095479db38d6df799875e34039d4938aaa6` | `9d8ceb91d103ef89400c9d9d62328b4858743cf8924878aee3b8afc594242ce0` | `a7c75c919608ea1af3bb6d4aca052af43f35e6967a4f133a865bfe2bde13f21e` |

原件各312087009B，转换各312062436B。SPM/config/vocab独立SHA见`config/offline-models/opus-source-lock.json`及归档manifest。Catalog SHA **`f88a5f57513a0aaae3bbd659950adf042c8c8dde07736084cb951f61f22c7f0f`**。实际模型留Git外`.local/round13-managed-models/catalog-该SHA/catalog.json`；没有模型进入Git或CI包。

实际运行Candle0.9.2、SentencePiece-rs0.2.2、Rayon1.11 CPU。固定源SPM→piece→Marian vocab→encoder→逐步decoder/KV→EOS→target SPM；PAD抑制、明确greedy，不把逐词替换或模拟输出当作翻译。模型加载、哈希、转换/文件读取与推理在后台/管理工具，不在窗口消息循环或实时输入路径。

权重按具体上游CC-BY4声明本地研发，en→zh镜像另标Apache2、训练来源完整权利链unknown保留。许可原文/作者/修改说明已保存，模型状态仍research_only，输出为Translation + Generated。正式语义sidecar或词频准入没有因此改变。

## 实际输出与已见质量问题

20条研发烟雾输入包括用户给定10句和AI补充10句，来源明确，缓存关闭。实际输出原文、token/步骤/时间/版本全部在[JSONL](translation-round13-evidence/inference/extended-resource-smoke/stdout.jsonl)。20次均生成不代表20次正确。

| 输入 | 实际输出 |
|---|---|
| 你好。 | Hello. |
| 我正在开发一款中文输入法。 | I'm developing a Chinese input method. |
| I am developing a Chinese input method. | 我正在开发一种中国输入法 |
| The result should preserve the original meaning. | 其结果应保留原意。 |
| Didn't you receive the message? I do not think that the plan is impossible. | 你没有收到消息吗? |

保留明显失真：上例漏掉后半句否定；Cloudleaf被写作Claudelaf，“123.45元”变成“$123.45”；emoji和混合中文可丢失/替代。没有编辑模型输出或用非空率宣称准确率。正式Top1/Top5/MRR均null，无独立gold、未给出BLEU/COMET等质量结论。

## 性能与资源

完整[逐点资源](translation-round13-evidence/inference/extended-resource-smoke/result.json)属于独立CLI进程，含两个模型、tokenizer、运行时与程序，**不称为单模型内存**。

- 新进程完整校验/加载1333ms；OS文件缓存未清空，不声称物理冷启动。随后20次推理73–647ms，样本少且输入不同，P95保持null。
- 墙钟7.919秒、内核+用户CPU9.859秒；31个实际采样点（工具目标间隔100ms，枚举开销使实际间隔更长）。采样working set峰949501952B、private commit峰952029184B、线程峰8、句柄峰51。
- 早期独立运行记录不同峰值1002934272B等，原文保留，不取较小者冒充唯一稳定内存。没有长期趋势、OOM注入或OS级资源硬隔离验收。
- provider最多两个resident模型、一次推理、4个计算池线程；4不是进程总线程数。面板离线service为1worker、队列8；输入256token/16KiB、生成256步，超限明确失败。没有无限替补worker。
- [真实生命周期运行](translation-round13-evidence/runs/lifecycle-real/stdout.txt)含10次无缓存重复推理、锁竞争Busy、排队满、实际取消、旧context StaleResult、1ms逻辑超时和worker真实结束后的恢复。14次实际provider调用，最大同时1，最终0；超时后底层另用28ms返回Cancelled，未冒称立即抢占内核。没有长期测试结论。

8条额外真实边界：空/空白InvalidInput、16385B和小于16KiB但超过256token的InputTooLong、预取消Cancelled、0秒Timeout，以及之后两次成功。原文见[边界日志](translation-round13-evidence/inference/resources-and-boundaries/stdout.jsonl)。不支持方向由provider能力及原有service校验；固定模型不提供Auto识别。

## 管理、版本和恢复

[真实模型管理记录](translation-round13-evidence/management/summary.json)覆盖21条命令/事件：完整校验、重复导入、附加用户文件拒卸载、单字节损坏拒用/拒卸载、缺tokenizer、错误固定hash、原件恢复、署名元数据新版本并存与旧版回退、只删受管版本且保留源文件、真正终止导入子进程再重新导入。它不是物理断电测试，也不是任意新模型架构升级。

staging不成为Ready，新的进程建立独立segment式暂存目录；已中断材料保留。卸载中途I/O失败可能留下不完整目录，后续自动卸载会拒绝，需要人工核对。没有把拒绝自动删损坏目录宣称为完整自动恢复。

## Windows面板

用户已授权仅离线窗口自动化。真实GUI绑定`1dc9fb80157afa2bfbb643ff598cffe60815dcd8`，EXE SHA `bdefb663cc2fe7c9a4e768cf2f31c8fbe11efb30b8c77ff20a0d2be267ff7066`。在离线提供者、真实受管catalog、缓存/联网未启用时，实际键入双向句子，观察Running→Completed、Generated和固定模型来源；中→英251ms、英→中204ms均含UI观察间隔。

[中译英截图](translation-round13-evidence/gui/zh-en.png)、[英译中截图](translation-round13-evidence/gui/en-zh.png)、[复制回贴](translation-round13-evidence/gui/clipboard-roundtrip.png)、[取消](translation-round13-evidence/gui/cancel.png)及[步骤原文](translation-round13-evidence/gui/accepted-steps.json)。回贴追加了刚复制的译文，未读取旧剪贴板。取消后原文保留、迟到观察仍Cancelled无译文；正常Alt+F4退出码0。

最终提示修复后的本地release EXE为6580224B，SHA **`df92ebe73123af33fbca8196b8c66997303291d1c109c5f0e8a425746c15f78a`**，路径`.local/round13-build/release/cloudleaf-translation-panel.exe`。它已完整回归，未把前一版GUI记录转绑到这个SHA；神经/原生控件实现相同，仅错误文案不同。源码、产物及命令分别记录。

## 原始失败与修复

所有原始stdout/stderr、返回码及源码/工具指纹位于[运行目录](translation-round13-evidence/runs)，没有删除旧测试或失败分母。

1. 初选Candle0.9.1严格RustSec失败；0.11.1 MSVC CPUID失败；最终0.9.2限定ZIP/paste修补并完整审计，不忽略告警。
2. 两真实PTH原件不是ZIP，直接读取失败`Could not find EOCD`；先修设计，再进行固定工具、安全受限开发转换和逐张量校验。
3. 管理API和schema3设置失败测试先行；固定管理路径、缺件、损坏和混版本被拒绝。
4. 实际GUI发现旧逻辑硬编码禁用离线整句；保留原生控件失败测试，以真实capabilities修复。
5. 完整回归曾失败于“未安装”旧状态约定，修复提示后原断言保留并通过。
6. 最终审阅发现token限制误写16KiB，失败测试101后修正为字节或提供者token限制；限额未放宽。
7. Git源锁CRLF规范化验证失败后脚本仍继续提交，是工程执行错误。保留`18a5c0a`历史并由`1dc9fb8`修正属性和源锁；反向核全部212文件，不改写提交。
8. 多行Edit的UIA SetValue不发用户编辑通知，曾导致可见原文但请求为空；真实键入正常，不据此修改模型逻辑。桌面恢复后历史窗口失去绑定，旧进程退出状态未捕获保持unknown；重新专用启动的正常退出记录独立保留。一次误用Hidden启动由测试工具明确终止，不列为产品崩溃或正常退出。

## 完整回归与CI

[最终14条命令](translation-round13-evidence/local/round13-final-acceptance/results.json)逐条返回0，绑定实现`0b41b2578a84c09b2170f34ce38c328cda6978a3`。包含diff/fmt、Clippy `-D warnings`、locked/offline全目标测试、doc-test、release、工作区/资源审计、25项Python、严格RustSec/许可、旧基准/消融及离线examples release。Rust134通过、0失败/忽略；当前doc-test实际0，不能编造成有文档用例。旧微基准仍是原型核心，不代表正式词库或TSF性能。

Cargo.lock SHA `6c82b7aaf5ddf7a64e82230fd3466fe2d53365d2371375561e1fb9bfde9e3106`。最终213包/382许可原件、RustSec数据库/工具SHA和原始JSON在审计文件；开发转换工具许可另存，不以RustSec代替Python扫描。Windows/Linux最终push CI须核最终报告提交的两个job/步骤/原始日志和三个artifact，交付回执生成前不预填成功。

## 状态和退出条件

| 状态 | 内容 |
|---|---|
| done | 固定真实模型取得/转换/校验；本地Rust双向推理；既有Windows面板整句/取消/复制；模型管理与资源预算；适用本地回归 |
| partial | 单次资源/重复调用观测、有限SP差分、模型元数据更新回退；不是长期泄漏、OOM或全部模型兼容验收 |
| blocked / not-run | 完全断网环境与真实无网络请求审计；独立人工语言gold、正式质量目标；模型生产分发权利；Python生态漏洞扫描；物理断电 |
| not-implemented | Auto识别、量化/GPU、任意模型下载器/通用模型格式、TSF集成及应用上屏；原R12密钥/联网安全按钮人工验收仍待用户 |

离线provider不持有SecretStore/HTTP/下载接口，单独CLI未注册在线provider，源码检查没有网络调用；**本机网络未物理断开，也未抓取全量网络轨迹**，不得以这些结构证据证明“完全断网通过”或“零网络请求已验收”。未改变网卡/防火墙或安全设置。

没有发现已能复现且未修的新增P0/P1代码缺陷；有限测试和AI审阅不能证明不存在。已知译文漏译/数字货币失真使正式质量门禁继续失败。下一步由真实受控断网Windows环境验收双向窗口路径，再用独立授权gold评价/改善模型与解码；权重分发与训练权利链单独审查。上述门禁未齐，不放行正式离线翻译或CL-010。
