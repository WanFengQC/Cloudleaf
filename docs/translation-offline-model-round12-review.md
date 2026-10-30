# Round 12：离线模型候选审查

2026-10-31，只读来源审查，没有下载权重、转换模型或执行推理。现有 `OfflineModelProvider` 仍不可用，面板必须显示“未安装”。以下固定来源声明不能直接当作 Cloudleaf 生产准入。

| 候选 | 官方固定 revision | 权重文件元数据体积 | 权利边界 |
|---|---|---:|---|
| OPUS en→zh | `408d9bc410a388e1d9aef112a2daba955b945255` | 312,087,009 B | 固定 HF README 声明 Apache-2.0；需核具体上游权重原件范围 |
| OPUS zh→en | `cf109095479db38d6df799875e34039d4938aaa6` | 312,087,009 B | 固定 README 声明 CC-BY-4.0，须保留署名与修改记录 |
| M2M100 418M | `55c2e61bbf05dfb8d7abccdc3fae6fc8512fd636` | 1,935,796,948 B | 模型卡声明 MIT；多语单模型可作较大对照 |
| NLLB 600M | `f8d333a098d19b4fd9a8b18f94170487ad3f821d` | 2,460,457,927 B | CC-BY-NC-4.0，模型卡排除生产部署，当前不选为产品随包候选 |

来源：[OPUS en→zh 固定 README](https://huggingface.co/Helsinki-NLP/opus-mt-en-zh/raw/408d9bc410a388e1d9aef112a2daba955b945255/README.md)、[zh→en 固定 README](https://huggingface.co/Helsinki-NLP/opus-mt-zh-en/raw/cf109095479db38d6df799875e34039d4938aaa6/README.md)、[M2M100 固定卡](https://huggingface.co/facebook/m2m100_418M/raw/55c2e61bbf05dfb8d7abccdc3fae6fc8512fd636/README.md)、[NLLB 模型卡](https://huggingface.co/facebook/nllb-200-distilled-600M)。字节数来自文件 API 元数据，不是下载后验证。

元数据预期 LFS SHA-256：en→zh `69a1d6ec829cee349360b3b677ac0aa99a7d88822d1a6578370029efffdca3f5`；zh→en `9d8ceb91d103ef89400c9d9d62328b4858743cf8924878aee3b8afc594242ce0`；M2M100 `d907ea45e4e4b9db163382a6674f6218b3c59566fe06d77f4055c208b4e87ed1`。没有声称实际原件校验通过。

## 下一阶段的工程实验

优先研究两个 OPUS 方向模型，加 M2M100 对照。OPUS 是 Marian Transformer，配套各自 source.spm、target.spm、vocab.json；不能混用两个方向 tokenizer。英→中模型需核对支持的目标语言前缀，例如 `>>cmn_Hans<<`。M2M100 要设置源语言和目标语言 token。[OPUS 说明](https://huggingface.co/Helsinki-NLP/opus-mt-en-zh)、[M2M100 说明](https://huggingface.co/facebook/m2m100_418M)。

建议架构为现有 Rust Provider → 项目自有小型 C ABI shim → CTranslate2/SentencePiece C++；仍复用 TranslationService，不新增调度器，不在运行期调用 Python。Python 只可在独立离线转换阶段使用，转换后固定全部文件、量化配置与 tokenizer 哈希。[官方 OPUS 转换](https://opennmt.net/CTranslate2/guides/opus_mt.html)、[支持的 Transformer](https://opennmt.net/CTranslate2/guides/transformers.html)。

可审查版本：CTranslate2 v4.8.4，解引用 commit `5de572ef60b67a0276c5989ee73d0072808ff4c3`，MIT；SentencePiece v0.2.1，commit `31646a467d2051eb904e0b45de3a73e91fe1c1e3`，Apache-2.0。C++17/CMake/MSVC runtime及实际选取的 OpenMP/MKL/oneDNN 等依赖须单独准入，当前未安装。[构建要求](https://opennmt.net/CTranslate2/installation.html)、[CTranslate2 许可](https://github.com/OpenNMT/CTranslate2/blob/v4.8.4/LICENSE)、[SentencePiece 许可](https://github.com/google/sentencepiece/blob/v0.2.1/LICENSE)。

具体取消约束：官方 TranslationOptions callback 用于 greedy search，不能推断 Beam 也支持逐 token 中断；不能中断时仍让旧 worker 占预算直到真实退出，结果先失效。[固定头文件](https://github.com/OpenNMT/CTranslate2/blob/v4.8.4/include/ctranslate2/translation.h)。

OPUS 项目代码 MIT 不代表全部模型权重同一许可；en→zh 镜像声明与系列泛称存在差异，必须取得具体权重原件的许可/署名并闭合来源。模型卡不能证明训练数据授权已全审。[项目许可](https://github.com/Helsinki-NLP/Opus-MT/blob/master/LICENSE)、[上游权重说明](https://github.com/Helsinki-NLP/Tatoeba-Challenge/blob/master/models/eng-zho/README.md)。

下一阶段先完成权利与原件准入，再测试 CPU 冷加载/热推理/峰值内存、Windows 离线部署、长句/数字/专名/标点、取消与资源回收，使用独立人工 gold。延迟、内存、量化后体积及语言质量当前全部 null；官方支持 INT8 不等于这些模型已完成量化或质量验证。[量化文档](https://opennmt.net/CTranslate2/quantization.html)。
