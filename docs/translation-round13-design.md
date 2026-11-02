# Round 13：真实离线 Marian 推理设计

2026-11-02。基于实际检查点 `89d9cf287652776823db151e59a6e3d6d90e94ca`；工作区干净，main仍`a564fd47b0f054ff360592af3ccb8d744021160f`，PR5开放Draft。沿用现有service、取消、缓存、面板和设置worker，不增加翻译调度器。

## 路径与依赖决定

新增平台无关 `crates/cloudleaf-offline`，使用精确Candle core/nn/transformers **0.9.1**（MIT OR Apache-2.0）和 `sentencepiece-rs` **0.2.2**（Apache-2.0）。CPU默认，不引入CUDA/MKL、C++运行时或运行期Python。Candle官方`models::marian`及`examples/marian-mt`已有真实encoder/decoder/KV cache路径；使用本地PTH权重读取，不运行pickle任意Python代码，不自行实现神经网络或伪造输出。

候选为两个固定OPUS-MT模型：en→zh `408d9bc410a388e1d9aef112a2daba955b945255`，zh→en `cf109095479db38d6df799875e34039d4938aaa6`；每个权重312,087,009B。M2M100约1.94GB且部署开销更大，暂不下载；NLLB非商业/用途限制不作为产品默认模型。具体权重、tokenizer、config和训练来源分别记录，不将可下载或镜像许可证标签当成生产发布批准。两模型原件未取得并校验前，不能写Ready或推理成功；下载另行遵守用户授权。

Tokenizer使用配套source.spm/target.spm及vocab.json：SentencePiece piece经原Marian vocab映射，不把SPM内部ID当成网络词表ID；输出反向映射到piece再decode。en→zh按照模型资源核定目标语言前缀，zh→en不加虚构标签。必须与官方SentencePiece实现做研发样本差分；不采用未经溯源的第三方tokenizer JSON。

## 文件、安装与状态

模型目录外置Git。schema1 catalog引用两个版本化manifest（相对路径和SHA）；manifest记录模型ID、方向、固定来源URL/revision、架构/runtime/tokenizer类型、许可原件/署名、研发准入状态及每个文件的字节数/SHA-256。严格schema；入口拒绝越界路径、缺文件、混合tokenizer、损坏文件、不支持架构/版本。所有校验和加载在设置后台或独立管理工具执行。

本地导入复制到应用管理目录的独立ID/内容版本目录，不覆盖已有版本；完整验证后才生成catalog。失败或半写目录不能成为Ready。卸载仅删除由管理器创建并校验的版本，不删除导入原件；更新建立新版本，旧配置可以回退。无自动联网下载或启动时下载。模型路径及安装记录不进入普通日志。

状态区分NotInstalled、Checking、Ready、Loading、Loaded、Invalid、Incompatible、Failed。Checking/Loading是实际操作阶段，Ready表示文件校验通过，Loaded要求真实构造会话；状态不能代表正式语言质量或商业分发准入。

## 推理和资源

`MarianProvider::load(catalog)`在后台校验并加载本地模型，复用`TranslationProvider`。方向明确，不伪造Auto识别。支持word/phrase/sentence；输出`OutputKind::Translation`、`QualityStatus::Generated`，来源包含固定模型身份。

一份provider最多两个resident模型（每方向一个），整个推理并发最多1，通过有界现有service排队；锁竞争返回Busy，不增加替补线程。默认输入最多256token（含标签/EOS）、输出最多256生成步，且不超过模型position上限；超限明确失败，不把截断内容当完整译文。生成采用明确greedy/argmax研发基线，不宣称最优或与默认Beam相等。禁止PAD生成，按真实EOS结束。每个任务前后清KV cache，cache identity绑定catalog内容及config版本。

取消/超时在编码前后和每个decoder step检查；单个张量运算不是可抢占的。逻辑取消先失效UI，worker持续占预算直到当前计算真正返回；不声称取消立即释放所有内存。设置替换继续等旧worker真实退出，窗口关闭沿用2秒有界流程。输入线程/UI不等待模型、不读模型文件。观测分别记录加载、输入/输出token、生成步、冷/热时间、进程资源，整进程峰值不称为单模型内存。

## 面板与设置变更范围

在现有设置schema增加可选本地catalog路径并迁移旧版本，联网授权继续默认关闭；保留独立在线model字段。原生面板新增离线模型catalog选择/路径及状态/来源显示，设置准备时原文可保留。模型/配置/方向变化沿用现有revision/session失效，不修改默认输入引擎或types。

文件范围：新offline crate及其管理/实际推理example、Cargo成员/锁/精确依赖政策和许可副本、panel settings/setup/windows_ui与相应测试、独立模型获取/校验工具、R13报告证据。R1–12历史证据不覆盖；旧stub兼容保留并明确不可用。

## 验收和边界

先提交设计；失败测试覆盖损坏/缺件/路径/版本和不可用错误，再实现。真实模型到位后执行双向给定研发句、长句/混合/Unicode、取消/超时、cold/warm、官方tokenizer对照；保存输出原文、模型/源码/二进制/工具身份和命令返回码。没有原件或许可时保留partial，不用mock代替真实推理。

完全断网是独立环境事实，不能仅用不设置在线provider代替。优先在无联网实现的离线推理进程观察网络行为；不擅自关闭用户网卡/安全设置。没有受控断网环境时明确保留该项未验收。

执行全套Rust/Python/fmt/Clippy/release/资源许可RustSec和方向测试，最终HEAD核Windows/Linux push CI。真实GUI验收依Computer Use规定，不触碰密钥/联网安全按钮。所有五PR继续Draft、原四PR/main不动，不执行TSF或8h/24h，不进入CL-010；正式Top1/Top5/MRR无独立gold仍null。

## 主要风险与退出条件

- 镜像en→zh Apache标签与上游模型CC-BY4声明不一致：保留具体原件与归属说明，研发加载不自动批准随包分发。
- Candle模型接口存在不等于本权重实际兼容；必须加载真实文件并核输出/形状，失败保存原文。
- 纯Rust SentencePiece不是官方C++实现：需与官方实现差分，差异保留而不静默修词表。
- CPU greedy可能较慢/质量不足：保留真实分母和资源占用，不用速度宣称正式质量。
- 两方向实际推理、模型身份、后台面板展示和适用回归都具证据后才称工程推理路径交付；完整断网、正式质量和分发门禁分别审查。

## 实测后的依赖决策修订（实现前固定）

0.9.1真实RustSec失败：ZIP1.3.1命中漏洞，paste停止维护。0.11.1在Rust1.90 MSVC又真实命中CPUID调用编译错误，且扩大停止维护依赖范围。最终采用 **Candle0.9.2**，其官方Marian接口一致；核心本地源码仅将ZIP7改为已修复的**ZIP9.2.1**并适配三处文件名Result返回值。GEMM七个0.19.0 crate及pulp0.22.3仅将paste依赖替换为**pastey0.2.3**，运算.rs不变。九个固定原件共212文件约3.53MB；不是以忽略审计或伪造包名消除告警。外部独立probe的Rust1.90 locked/offline check及严格RustSec均真实返回0，真实工作区仍须重跑。

新增 `vendor/offline` 及单独精确源补丁清单/生成工具/审计。允许的path依赖仅这些固定原件与限定补丁，逐文件哈希约束；不放宽原registry政策到任意path。保留上游LICENSE和原件身份，现有输入核心依赖不升级。维护成本是后续升级时重核这11个修改文件，不能默认沿用安全结论。

设置升至schema3并迁移schema1/2：新增独立offline_catalog，不覆盖在线model或词典路径，迁移/启动继续关闭联网。原测试仅更新当前版本期望并保留旧schema输入覆盖。模型manifest还须保留固定许可原文与署名/来源说明；目前两方向均按具体上游CC-BY4声明记录研发边界，并明确en→zh镜像标签差异。禁止管理器保留名与导入内容冲突，不允许改许可标识伪造准入。
