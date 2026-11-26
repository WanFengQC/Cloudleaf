# Round 24 离线翻译模型选型

完成三种候选的权重元数据核对、许可证核对和接入评估。固定 Marian 仍是本地对照。新增权重获取、量化、实测内存与延迟、模型对照推理状态为 `blocked`，现有授权覆盖固定 Marian 的两份权重。本轮只读取公开模型卡、项目文档及文件元数据。

| 模型与固定版本 | 方向、分词与技术路径 | 权重原件及计算开销 |
| --- | --- | --- |
| Meta M2M-100 418M `55c2e61bbf05dfb8d7abccdc3fae6fc8512fd636` | `en` 与 `zh` 双向，使用发布的 SentencePiece BPE、词表、语言标识，解码强制目标语言标记。可研究 `rust-bert` 的 M2M100（LibTorch 依赖）或 CTranslate2 原生 worker 内服务。CTranslate2 文档列明该架构及 INT8 转换，支持 CPU 与可选 CUDA。 | `rust_model.ot` 1,935,781,288 字节，PyTorch 权重 1,935,796,948 字节，都是独立备选格式。名义 418M 参数的 INT8 张量约 418 MB，排除非量化权重、索引与缓存。端到端内存和延迟未测。 |
| Google MADLAD-400 3B MT `fa184c675da0b5c9e1c8694fccd4e12e2d422094` | 多语 T5 编码器解码器。按目标语言加 `<2zh>` 或 `<2en>`，使用随权重发布的 T5 词表与 SentencePiece。Candle 上游示例明确展示 MADLAD 量化 T5 可运行路径。可从 CPU 验证起步，CUDA 需独立检查匹配的算子及配置。现有锁定的 Candle 0.9.2 源码包含 `t5` 和 `quantized_t5` 模块。 | `model.safetensors` 11,761,587,872 字节。发布者仓库列出的 Q4_K 为 1,654,597,280 字节，Q3_K 为 1,264,101,024 字节，Q2_K 为 965,486,240 字节。量化质量、内存与延迟未测。 |
| Meta NLLB-200 distilled 600M `f8d333a098d19b4fd9a8b18f94170487ad3f821d` | 英文 `eng_Latn`、简体中文 `zho_Hans`，使用 SentencePiece 和目标语言 token。CTranslate2 列明该架构的转换路径，CPU、可选 CUDA 和 INT8 需对特定构建验证。 | `pytorch_model.bin` 2,460,457,927 字节。权重采用非商业条件，本轮排除出产品候选。无本地测量。 |

上述文件字节数来自固定版本的发布元数据。实际 working set、private commit、CPU 热态、GPU 显存、冷启动、分词和生成 P95 仍是实测项目。现有两份固定 Marian 权重各 312,087,009 字节，实测耗时见[真实翻译回放](integration-round24-translation-quality.md)。

### 来源、许可证与衍生资源

- M2M-100 的 Meta Fairseq 入口给出原始 418M checkpoint 和词表，固定 Hugging Face 权重标注 MIT。Fairseq 代码许可证单独核对。MIT 下的复制、修改、研究及再分发附带版权与许可声明保留义务。转换时还需固定基件、分词器和 Rust/C++ 运行库，保留所用各原件的通知、转换记录及哈希。`rust_model.ot` 上游声明的 SHA-256 为 `f170f6a277d00b20144fa6dac6ecd781c5a5e66844c022244437dd2da3a83655`。原始训练与派生权重的完整材料链仍待交付审批。[Meta 原始来源](https://github.com/facebookresearch/fairseq/blob/main/examples/m2m_100/README.md) · [固定权重目录](https://huggingface.co/facebook/m2m100_418M/tree/55c2e61bbf05dfb8d7abccdc3fae6fc8512fd636) · [Fairseq 代码许可证](https://github.com/facebookresearch/fairseq/blob/main/LICENSE) · [MIT 条款](https://opensource.org/license/mit) · [Rust M2M-100 API](https://docs.rs/rust-bert/0.23.0/rust_bert/models/m2m_100/index.html)
- MADLAD 固定发布标注 Apache-2.0，原始 3B 检查点由 Google Research 发布，公开模型卡注明发布格式的转换来源。Apache-2.0 下的研究、修改、商业分发和衍生分发需满足许可证、相关通知和修改说明等条件。权重的 Apache 标记和训练数据逐来源权利应分别审查。Q4_K 的上游 SHA-256 为 `ea6e5531a3e95213c7f0635988d119e078a655c09306e47851e15d4c0c3f9c37`，主权重为 `66ff5f8fcaf92291da486fdfbd4d5233cec90e1359348a56e3172c978b3a76d4`。若量化需选择可重放的来源和工具版本，保留与原始准确性和内存对照。[Google 原始检查点](https://github.com/google-research/google-research/blob/master/madlad_400/README.md) · [固定发布与量化文件](https://huggingface.co/google/madlad400-3b-mt/tree/fa184c675da0b5c9e1c8694fccd4e12e2d422094) · [Apache-2.0 条款](https://www.apache.org/licenses/LICENSE-2.0) · [Candle 量化 T5 示例](https://github.com/huggingface/candle/blob/main/candle-examples/examples/quantized-t5/README.md)
- NLLB 固定权重标注 CC BY-NC 4.0，模型卡将其定位在研究和单句翻译，并限定产品部署、专业领域及长文场景。研究权利仍受非商业条款约束，派生量化继续受所用原权重条件约束。仅登记为许可不符的技术备选。[NLLB 固定权重和模型卡](https://huggingface.co/facebook/nllb-200-distilled-600M/f8d333a098d19b4fd9a8b18f94170487ad3f821d) · [CC BY-NC 4.0 条款](https://creativecommons.org/licenses/by-nc/4.0/legalcode.en) · [CTranslate2 架构支持](https://opennmt.net/CTranslate2/guides/transformers.html) · [量化类型](https://opennmt.net/CTranslate2/quantization.html) · [CPU 与 GPU 支持](https://opennmt.net/CTranslate2/hardware_support.html)

### Worker 接入条件

先验证 M2M-100 量化候选的固定模型与分词文件，再判断是否承担另一套原生运行库。MADLAD Q4_K 适合作为保留现有 Candle 方向的对照，优先实测模型大小是否超出设备限制。模型下载、许可和签名登记各依现有管理方式。先在 Git 外证明请求身份、取消、超时、截断、方向切换、未知语言标记和退出的行为，再通过 `WorkerClient` 及既有 IPC 进入相同性能和质量对照。模型代码和运行库仅限 worker 的依赖闭包，普通输入、候选释义查询与提交确认沿用既有路径。

本轮实际使用 Marian，不引入新的运行库。备选模型元数据原文与文件 SHA-256 留在 `.local/round24-translation/metadata/`，发布包没有新增权重、备用模型或量化产物。
