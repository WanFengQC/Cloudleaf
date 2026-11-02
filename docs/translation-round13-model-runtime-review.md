# Round 13：模型、运行时和开发工具审查结论

2026-11-02。此文结合固定上游原件与实际下载、转换、CPU推理，不把候选网页声明当作完整产品授权。29份初始来源记录见[原件清单](reports/translation-round13-evidence/model-source-review/sources.json)，实际权重取得见[下载记录](reports/translation-round13-evidence/models/acquisition.json)。

## 模型选择与权利

| 候选 | 固定版本/体积 | 实际决定与未知 |
|---|---|---|
| OPUS-MT zh→en | `cf109095479db38d6df799875e34039d4938aaa6`；312087009B | 已获用户授权下载，实际SHA通过，Windows CPU已生成译文 |
| OPUS-MT en→zh | `408d9bc410a388e1d9aef112a2daba955b945255`；312087009B | 同上；镜像Apache2与专门模型声明CC-BY4差异保留 |
| M2M100 418M | `55c2e61bbf05dfb8d7abccdc3fae6fc8512fd636`；1935796948B | 固定模型卡MIT；训练原件授权链未闭合，本轮未下载、未运行，不推测内存/速度/质量 |
| NLLB600M | 既有Round12审查 | 非商业/用途约束，不选为正式默认模型；未追加下载 |

选择两个OPUS是因为方向/结构明确、原件较小且有具体预训练模型许可声明，不能据此认定质量优于其他模型。当前使用FP32、CPU与greedy；未量化、未验证GPU，也未将上游量化支持当成本地已实现能力。

[OPUS-MT-train固定README](https://github.com/Helsinki-NLP/OPUS-MT-train/blob/d5e932c96d77596378010aa7e7f1caeb8ccaa9f8/README.md)专门声明预训练模型（含Tatoeba模型）为CC-BY-4.0；根训练代码MIT不能替代模型许可。zh→en固定HF卡也为CC-BY4，en→zh固定卡标Apache2，不能假定差异已获得重新许可解释。固定Tatoeba仓库根材料为CC-BY-NC-SA4，不能把整库材料或训练语料都归为模型CC-BY4。

权重、同revision的source/target SPM、config与vocab按同一模型发行物的具体声明开展本地研发；没有另行证明每个资源的独立版权链。模型训练来自OPUS集合，完整训练材料清单及权利链仍unknown。保留作者Language Technology Research Group/University of Helsinki、原模型/revision/URL、CC-BY4原文与转换修改说明；这不解决训练来源或镜像差异。**生产权重再分发仍blocked**，Git和CI包均无权重。

CC-BY4原件SHA `9ba9550ad48438d0836ddab3da480b3b69ffa0aac7b7878b5a0039e7ab429411`。实际manifest为research_only，不接受改标production或换许可ID。SPM程序Apache许可也不能给SPM数据资源重新授权。

## 运行时决定

| 方案 | 实际适配与部署决定 |
|---|---|
| Candle0.9.2 + sentencepiece-rs0.2.2 | 已实际支持两个固定Marian encoder/decoder、KV cache、自回归EOS/PAD处理和本地SPM；Rust CPU，无运行期Python或额外模型DLL |
| Candle0.9.1初选 | 真实RustSec失败，ZIP漏洞及paste停止维护，已放弃；原失败保留 |
| Candle0.11.1 probe | Rust1.90 MSVC实际CPUID编译失败，未通过更换工具链绕过项目基线 |
| ONNX Runtime / Rust绑定 | 未取得并验证这两个模型的完整固定encoder/decoder/KV/转换路径，不因能加载ONNX便宣称可翻译；没有引入依赖或发布DLL |
| CTranslate2 | 官方具备OPUS/Marian及CPU/量化路径，当前工程未建立并验证Rust/C++桥接，不作为实际后端 |

Candle源码自带Marian模型和示例，但Cloudleaf没有复制示例中的hf-hub下载或第三方tokenizer JSON。实际使用同revision的本地SPM，piece经Marian词表映射；en→zh增加独立`>>cmn_Hans<<`，末尾EOS0，PAD/start65000。官方SP0.2.1与Rust分词器60组研发编码/ID/解码对照一致，不证明任意SPM或所有Unicode完全等价。

两个原权重是旧Torch格式，实际Candle直接读取失败；开发转换使用固定CPU Torch2.10.0+cpu `weights_only=True`，Safetensors0.7.0。每方向256个保留张量逐dtype/shape/bitwise核验，共享嵌入别名先核一致再移除；重复序列化SHA一致。没有量化或dtype转换，仍不能推断不同运行时/解码器逐token译文等价。运行期仅读取Safetensors，不调用Python或动态模型源码。

最终213个Rust外部包、382份实际许可原件由Cargo.lock、精确政策、许可manifest和严格RustSec绑定；旧136包版本未升级。九个固定vendor包只修ZIP API和paste宏依赖，212文件可离线重建且能反向核上游，运算源码不改。不忽略advisory或warning。Candle/GEMM/Pulp/pastey的MIT/Apache及SentencePiece-rs Apache原件随开发程序保留，不重新标为项目MIT。详见源补丁锁与[最终审计](reports/translation-round13-evidence/local/round13-final-acceptance/dependency-audit.json)。

## 转换开发环境

[实装许可证归档](reports/translation-round13-evidence/conversion-tools/review-draft.md)记录15个distribution、62份原件/NOTICE，共879689B，所采许可文件RECORD hash mismatch为0。Torch聚合许可证和NOTICE、NumPy Windows OpenBLAS/LAPACK及GCC runtime GPL3+GCC-exception3.1均原样保留，不能概括为全部BSD。官方SentencePiece wheel没有独立许可文件，另存固定官方源Apache2，未伪称随wheel附带。

实际安装报告保存下载URL和wheel SHA；pip25.0.1原始wheel取得记录缺失保持unknown。开发环境不随应用分发，Python生态漏洞扫描未执行，RustSec结果不覆盖Python或模型文件。后续若重新分发开发环境，须按其完整随包内容重新审查。
