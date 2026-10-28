# CL-009 Round 5：逐行隔离审计、频率语义和独立人工评测前置

2026-10-28；分支 `codex/cl009-data`，方向起点 `81d85ad414b3ecae6af9573b881acbd9dfa56037`，被测实现 `6d3d4e1ac717ad96de8c839ee9857c3629ebecbc`。本轮交付只读隔离审计器、独立语言侧包验证器、实际来源决策和新的重放证据。现代词频、真实语义注释、独立人工 gold 和生产语言质量仍然阻塞，CL-009 不关闭，不进入 CL-010。

## 设计与修改范围

[先行设计](../cl009-round5-design.md)提交于 `aca20b3`；其后 `cbf0f3c` 添加尚无实现时真实失败的测试，`329114f` 补充同词同源同语料 Zipf 约束，`c9576d2` 补充分离源语法/义项错误的反例，`ab897f8` 在实现前补齐无空格声调串及零冲突审计范围，`67365c0` 增加正计数下溢不得伪装观测零的反例。最终生产代码仅在 `6d3d4e1` 提交；后续方向提交只整理报告与文档。

|文件|本轮作用|
|---|---|
|`docs/cl009-round5-design.md`|先行范围、审计/频率/语义与恢复门禁|
|`tools/data_pipeline/audit_isolations.py`|只读固定原件逐行独立重放；分类不修改生产读音表|
|`tools/data_pipeline/linguistic_sidecar.py`|独立频率/POS/义项/有向翻译/短语模型的结构与数学一致性验证|
|`tests/test_isolation_audit.py`、`test_linguistic_sidecar.py`|新增 6+8 项测试及篡改、身份、未知值、引用和审核子例|
|`docs/cl009-round5-frequency-and-annotation.md`、`data/plans/ime-human-gold-v1.json`|实际来源决定、模型限制、尚未交付的人工评测计划|
|`tools/data_pipeline/README.md`、本报告和 `docs/reports/cl009/round5-*`|命令、原始证据、固定输入/工具/输出身份及阻塞|

共享 schema2/3、九列 TSV v2、默认 64 条原型、A/B 接口、核心音节表、既有导入/发布/恢复实现都没有修改；新侧包不接入默认引擎。旧测试、Round 1–4 报告及给 B 的冻结词库保留。

## 全部 1,468 条隔离记录

[逐条审计](cl009/round5-isolations/records.jsonl)保留繁简原文、原读音、原行及 UTF-8 原行 SHA、成员/1-based 行号、原拒绝原因、每个 token 的类别、未实施的处置和语言规范未裁决状态。[摘要](cl009/round5-isolations/summary.json)来自最终新包，而非沿用旧 manifest 的统计。完整源仍为 125,012 条，123,544 条接收、1,468 条隔离；原拒绝数量 763 `unsupported_cedict_reading`、704 `unsupported_reading`、1 条运行时预算未改变。

|逐记录主类别|条数|当前处理|
|---|---:|---|
|儿化后缀 `r5`|680|保留隔离；尚未设计并验收儿化输入/候选语义|
|逗号 / 间隔号|444 / 161|保留源表达，不自动删标点或改读音|
|无调 ASCII / 字面数字|137 / 3|不从词头猜拼音|
|无空格连接的数字声调串 / 冒号分隔|13 / 1|不自动切分或改写|
|成音鼻音 / 带调但表中缺项音串|9 / 11|不直接扩充生产合法音节表|
|`xx5` 未指定读音 / 多种表达缺口|4 / 4|保留未知，不补读音|
|运行时预算|1|保持既有 16 音节等边界，不因词源合法就放宽|

主类别互斥；token 计数可在同一记录内重复，不与主类别简单相加。分类描述固定源表达与运行时差距，**不是合法普通话或源错误的裁决**。源中零冲突已明确，通用冲突合并不在本审计范围；遇冲突失败。源语法和坏义项另有自编反例，不误报成多个 token 缺口；完整原件没有这两类坏记录。

本审计的数据派生部分保留 [CC BY-SA 4.0 许可及原署名的无损原文](cl009/round5-isolations/upstream-documents.json)和[本轮修改说明](cl009/round5-isolations/README.md)。工具代码为 MIT，不将带原文的审计记录重标 MIT。没有删除、改写或自动接收任何隔离记录。

## 来源选择、统计与语义模型

2026-10-27 的实际 primary 元信息检索有 9 项成功、2 项失败，URL、时间、HTTP 状态、完整元信息字节 SHA 和 Git blob/commit 见[原始记录](cl009/round5-source-review.json)。Tatoeba ToU 超时、CC-CEDICT 语法页不可读均保留，未伪装新许可确认；本轮没有下载候选语料、频率、句子或 heldout 数据包。

|来源|本轮决定与未满足条件|
|---|---|
|THUOCL 固定 `a30ce79…`|保留 DF 研究候选；旧年代和组成词表来源未闭合，不能当现代 token 频率|
|wordfreq 固定 `912caf6…`|中文组成来源仍不明，`blocked-provenance`；代码/数据许可不同，默认缺词零不是观测|
|SUBTLEX-CH 2010|独立 Cloudleaf 包的修改/商业再分发条款范围仍待核，`blocked-license-scope`；论文许可不自动覆盖附录原件|
|中文 Wikipedia `20261001`|选择为后续自产 token 统计的**条件候选**；须取得完整原件摘要、逐页修订/作者/许可及例外过滤、分词器和领域代表性证据；当前没有批准新频率来源锁|
|自有明确授权人工任务|优先的人评取得路径；标注许可、人员、仲裁者、独立保管者、任务和封存答案均未交付|

[频率/注释设计](../cl009-round5-frequency-and-annotation.md)明确 token 比例、DF 文档计数和每十亿 token 的 Zipf；不混用分母、时期、分词或来源。未收录、低于截断、未观测保持 null，真实零须有明确观测和实际零计数。派生 Zipf 只能引用同词、同源、同语料全部身份的正 token 观测，不能由 DF、缺词默认零或跨词记录生成；正计数 float 下溢不能通过误差容忍变成观测零。

独立 `cloudleaf-linguistic-sidecar-v1` 可表示来源及审核 SHA、POS/tagset、稳定义项、逐向多对多翻译、短语组件和双审/仲裁声明。本轮只用自编 fixture 测试，**没有生产频率、POS、义项对齐、翻译或短语注释侧包**。字段和数学关系通过不证明许可真实、reviewer 真实独立或语言事实正确。现有 123,544 条频率/POS 仍全部 unknown/null，45,146 条旧斜线分段不自动升级成稳定义项。

人工 IME [实际计划](../../data/plans/ime-human-gold-v1.json)状态为 blocked，正式 Top-1/Top-5/MRR 均 null。C 侧包和该计划不自动兼容 B 的 `quality-preparation`，更不等于已交 gold；未来须逐源显式转换并核验 artifact、lineage、preprocessing、annotation protocol、review hashes，另交独立人工意图/可接受输出/完整按键任务、权利、近重复审计、人员与封存材料。词典派生答案、自填 reviewer 名称、机械样例和 41/83 条旧种子均不能放行。

## 固定身份、回归与恢复

[干净重放](cl009/round5-clean-replay.json)从实现 `6d3d4e1ac717ad96de8c839ee9857c3629ebecbc` 导出 458,773 字节 git archive，SHA `88800d4be792a239b5ca84fe13637c94907f0bfce39d9b7b4fa9ab83cd05ddcf`。全新 checkout、无 pip venv、Python 3.12.10、`-I -X utf8`、无第三方 distributions，两个独立目录分别 build/verify，九产物逐字节相同；构建入口拒绝 urllib 网络调用（没有采用操作系统网络沙箱），归档中全部 69 项 Python 测试通过。

|身份|SHA-256|
|---|---|
|固定完整 ZIP|`fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`|
|新 manifest|`5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5`|
|共同 runtime.tsv|`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`|
|共同事实 records.jsonl|`168f4579a7d72c87c41a9b2bde0d5f65f1537c65b64e107ea60bbf409f7fd072`|
|共同 rejected.jsonl|`6c8b786912f4495b87064655b4f9f18369b1d7c94cea699e58f50c96afc8d0e8`|

全部九文件及工具文件摘要见[只读收尾核对](cl009/round5-evidence-index.json)。相对 Round 4 的 `2401b16`/`0dee2e2` 包，只有 manifest 因工具指纹变化而不同；不得写成跨实现九文件相同。B 的旧冻结输入不变。包内 `verify` 仍明确 `source_only_statistics_rebuilt=false`；完整原件重建和本轮独立逐行审计分别提供源级证据。

[本地九条检查](cl009/round5-local-checks.json)全部 exit0：release/debug 构建、locked fetch、fmt、全部 target Clippy `-D warnings`、60 项 Rust、doc tests、资源审计和 69 项 Python。69=旧 55+新增 14，未删除旧测试。其他诊断命令、七点实际包恢复和原型基准原文见[诊断记录](cl009/round5-diagnostics.json)；本轮性能/恢复摘要另列其实际输出，不将旧成功当新验收。

[七点实际包恢复](cl009/round5-real-interruption.json)全部完成：持锁、复制半途、缓存重命名后、临时指针写出后、指针替换后、回退替换前和回退替换后。每例只对自有子进程注入 `os._exit(73)`，持有真实解释器进程身份；全部 exit73、owner 已退出、恢复后再次 activate/rollback/verify 成功。旧包 manifest 为 `6012638a09d1a2e8d2d7347ad10c78a357c3b953bba68cff610795fb609770b0`，新包为上述 `597145…77d5`；用户词典探针 SHA 均为 `97de41d09a06013362436e2ace797b7499bf0c10e197c5a41e8c5ff4528aa2b1`。七例使用独立 store、最多三路并行，整批 380.658 秒；单例 107.895–176.137 秒包括初始化、校验、恢复和再激活，不等于故障子进程等待时间。子进程 180 秒上限未触发，不是物理断电验收。

完整新包两次 build 的原始计时为 30.140 / 30.887 秒，进程峰值 1,007,665,152 / 1,006,866,432 字节，含 Python 完整事实/索引结构。离线 [JSON 索引诊断](cl009/round5-index-performance.json)索引 13,028,916 字节、加载 273.0372ms，10,000 次精确键查询 P50/P95/P99 为 100/400/600ns，进程峰值 734,343,168 字节包含前置完整校验。现有工具仅输出这些分位数和计数，没有保存逐查询数组；不能从摘要重建全样本，也不能据此推断 Rust 候选、TSF 或生产词库性能。

[原型核心基准](cl009/round5-core-performance.json)仍只用 64 条手写词典：79,000 个事件 P50/P95/P99 为 2,000/5,300/13,200ns，最大 370,600ns，峰值 6,004,736 字节；release 二进制 SHA `d53f54a25bd6b4fb8c114a06966010d4b98bf0597efb7e9c5e897e96c59e5b80`，源码指纹 `8a6bc1038113fceca89c8a932d24b09a964581f2c7acdc30b2702772962d1ee2`。[83 条种子诊断](cl009/round5-algorithm-v1/report.md)保留逐例原始输出与对照；其 dev/heldout 名称只是旧自编诊断划分，不是正式独立语言评测。重型方向测量已错开，桌面后台负载仍未控制；没有跨轮性能优劣或生产语言准确率结论。

[红测原文](cl009/round5-red-evidence.json)保留精确 `cbf0f3c` 初始缺模块红测及两个后续真实失败。后续分类/下溢日志捕获时实现尚未提交，未归档当时完整实现字节，因此只作历史诊断，不能声称已精确重建旧源码。最终修复和所有测试则已绑定上面的精确实现归档。命令封装原文及 SHA 见[运行器证据](cl009/round5-runner-sources.json)，路径可在重放时显式调整。

证据首次暂存时，许可原文末尾空行使 `git diff --cached --check` 实际 exit2；[完整失败记录](cl009/round5-publication-check.json)保留原暂存树 `240c6e75559b6b1e68be602b2245e3fb305b0d58`、全部待交付文件的 Git blob/SHA、原命令和错误、修复前报告及运行器。按既有原文封装方式，将发布证据中的许可/署名改为 UTF-8 JSON 字符串，原文空白、字节 SHA 和 Git blob 均可复原；本地真实审计器输出及数据包的 LICENSE/NOTICE 不变，没有裁空白或豁免检查。`summary.json` 仍引用原始审计输出的原文 SHA，不能用封装 JSON 的文件 SHA 替代。

可在新目录提取并核对原文：

```python
import hashlib
import json
from pathlib import Path

source = Path("docs/reports/cl009/round5-isolations/upstream-documents.json")
document = json.loads(source.read_text(encoding="utf-8"))
output = Path(".local/round5-restored-notices")
output.mkdir(exist_ok=False)
for row in document["documents"]:
    raw = row["utf8_text"].encode("utf-8")
    assert len(raw) == row["bytes"]
    assert hashlib.sha256(raw).hexdigest() == row["sha256"]
    assert hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest() == row["git_blob_sha1"]
    (output / row["name"]).write_bytes(raw)
```

## 剩余阻塞与退出条件

|状态|边界与下一证据|
|---|---|
|完成|固定原件 1,468 条逐行审计、typed sidecar 工程模型/反例、实际元信息来源决策、冻结源码全回归和新包双构建|
|部分完成|完整授权词条/原读音/英文原始段可追溯；语言规范和英释义逐条审读、1,468 条缺口处置仍待人工/算法设计，不自动扩生产解析器|
|阻塞|现代 token 统计固定原件/许可及代表性，真实 POS/义项/翻译/短语数据，合法独立人工 gold 及全局家族/近重复泄漏审计|
|未完成|签名安装器、物理断电持久性、跨版本用户词典迁移、生产 Rust/TSF 资源与语言覆盖验收；真实系统输入法不在数据恢复证据范围|
|未开始|默认核心接入新数据、CL-010；必须 A/B/C 各自满足退出条件后再评估|

方向最终推送的完整 SHA、Windows/Linux push CI 及 PR 状态由共享 Round 5 审阅汇总逐项记录；本报告不把尚未核验的远程状态写成成功。PR #3 保持 Draft，不合 main。
