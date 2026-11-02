# 本地 Marian 整句推理

独立CPU推理适配层，复用`cloudleaf-translation`的请求、取消、任务和结果契约，不接入实时拼音输入。没有网络、下载或SecretStore接口。仅接受已审查的两个固定OPUS方向及其配套tokenizer，使用Candle0.9.2和SentencePiece-rs0.2.2。

## 准备模型

权重不入Git、不随应用分发。实际本地下载需要用户明确授权。固定原件URL/SHA见`config/offline-models/opus-source-lock.json`。两原件是旧Torch格式，不能直接交给Candle的ZIP PTH reader。

在隔离开发环境使用固定CPU Torch2.10.0+cpu、Safetensors0.7.0、NumPy2.4.2和packaging26.0转换；运行时不调用Python。工具先核原件SHA，只使用`weights_only=True`，保留全部非别名张量的dtype/shape/bitwise值，验证移除的共享嵌入别名，重复序列化哈希一致后输出报告。转换不能证明不同解码器或后端具有相同译文。

```text
python tools/offline_models/convert_opus.py --originals ORIGINALS --output CONVERTED
python tools/offline_models/prepare_opus.py --originals ORIGINALS --source-files SOURCE_FILES --converted CONVERTED --output BUNDLE
cargo run -p cloudleaf-offline --example offline_models --release --locked --offline -- check BUNDLE/catalog.json
cargo run -p cloudleaf-offline --example offline_models --release --locked --offline -- import BUNDLE/catalog.json MANAGED_ROOT
```

`SOURCE_FILES`包含固定config/source.spm/target.spm/vocab.json和CC-BY4原文，按源锁的相对路径放置。准备工具不联网、不覆盖输出；导入写独立内容版本目录，完整校验后重命名。更新导入新版本，切回旧catalog即可回退，不删除原件或旧版本。显式`offline_models uninstall CATALOG MANAGED_ROOT`仅删除本工具创建且仍完整的版本，存在额外文件、损坏或不符标记时拒绝，保留用户资料。没有物理断电恢复验收。

中断导入可能保留以点开头的staging目录，但不会登记为Ready；下一进程使用独立staging。卸载遇到中途I/O失败可能留下不完整目录，之后自动卸载会拒绝，需人工核对处理。固定内容更新测试只覆盖清单/署名版本并存与回退，未实现任意新权重架构升级。

模型内容固定为研发用途：上游模型声明CC-BY4，en→zh镜像另标Apache2，训练来源完整权利链仍unknown。文件许可、署名和哈希校验不等于生产分发或语言质量批准。输出`Translation + Generated`，不会伪装成已审读的Approved映射。

## 使用与预算

在Windows面板选择“离线模型（本地OPUS-MT）”，填入导入后的catalog路径并应用；从真实能力中选择单词/短语/整句，手动选择中→英或英→中。没有Auto识别。后台显示Checking/Ready/Loading/Loaded，失败保留原文并显示对应错误。加载和推理都在后台，设置切换沿用旧service实际退出规则。

最多两个常驻模型、一个推理、四个张量计算线程；面板离线service一个worker、八项排队，原有retained/cache预算保留。输入最多256token（含标签/EOS）、生成最多256步，超限明确报错，不把截断内容当完整结果。采用greedy及PAD抑制，未比较证明优于Beam；没有量化或GPU实现。SPM piece需经Marian词表映射，en→zh独立添加`>>cmn_Hans<<`，不混用SPM内部ID。

取消在encoder前后及各decoder步骤检查，单个张量运算不可抢占；逻辑取消不代表内核已停止。worker真实返回前持续占预算，不生成替补线程；每次前后清KV，provider版本绑定catalog SHA。模型资源释放依赖真实任务结束，不承诺OS内存立即回落。

```text
cargo run -p cloudleaf-offline --example offline_translate --release --locked --offline -- CATALOG
cargo run -p cloudleaf-offline --example offline_lifecycle --release --locked --offline -- CATALOG
```

第一个工具从stdin读取`{"direction":"zh-en","text":"你好。"}`这样的JSONL，显式输出研发原文/实际译文/token/耗时；不是应用默认日志。第二个执行真实模型重复推理、取消、排队预算、超时和worker退出验证。完整样本来源在`tests/fixtures/translation-offline-smoke.jsonl`；用户给定句子和AI补充均是研发数据，没有独立gold，正式指标仍null。

## 依赖修复

九个固定Candle/GEMM/Pulp源包位于`vendor/offline`。严格源审计验证212文件，只有已登记的宏依赖替换与ZIP文件名返回值适配；运算源码不改。`scripts/rebuild_offline_vendor.py`按固定原件离线重建，`scripts/check_offline_vendor.py`核逐文件及限定补丁；不忽略RustSec。原136个依赖版本保持，新增依赖单独列于`config/audits/offline-runtime.json`，许可原件归入原有translation许可清单。

断网环境验收、正式语言质量、生产模型分发和全链路TSF均需分别成立，不能从本地推理或CI成功推断。完整实际结果见Round13报告。
