# CL-009：正式频率与语义侧包的发布前契约

2026-10-29。延续R5独立`cloudleaf-linguistic-sidecar-v1`结构，不改变生产schema2/3、TSV v2、默认输入或现有正式词条包。此文登记正式发布尚缺的材料，不交付伪造的production数据。

## 频率、分母和冻结协议

|指标|值与分母|身份与缺失规则|
|---|---|---|
|token_frequency|该词token计数/总tokens，分子为非负整数，分母正整数，单位tokens|固定corpus id/version/SHA/period和tokenizer id/version/config；没有观测为unobserved/null，不能补0|
|document_frequency|包含该词的文档数；分母总documents，值仍为文档计数而非比例|必须固定文档边界/去重和同一处理协议；不能当token计数|
|zipf|`log10(token_frequency)+9`；母观测必须为同词/源/语料/时期/分词的正token比例|零token没有有限派生Zipf；原生Zipf另附定义SHA，不能凭DF/词序/单字频率构造|

除既有语料字段外，正式统计发布还须冻结原件和许可/作者/例外审查、提取/分词实现SHA及配置、token定义、文档边界、重复文本处理、计数工具和运行环境。`corpus_gate`当前核固定Wikipedia revision和证据声明，不自动取得许可或证明代表性。没有通过的真实统计语料，当前三指标均无新观测。

## 正式语义材料清单

已有v1结构可表达下表，但结构校验不是正式发布器。

|对象|必须保留的身份和证据|
|---|---|
|词条|事实层稳定entry_id及固定lexicon SHA；不能把释义修订当成新词头来源|
|POS|tag、tagset及版本，source_id、源unit/SHA、人工双审|
|义项|稳定sense id、entry_id、language、源definition/POS、源断言位置及SHA；源义项改版需明确迁移，不能猜目标义项|
|有向翻译|relation id、source_sense_id、target_sense_id、explicit relation、源证据与双审；反查索引引用同一断言，不自动新增逆向equivalent|
|短语/固定搭配|stable phrase id、entry_id、kind、components、源证据；分词结果不自动证明固定搭配|
|权限和审核|来源版本/原件/许可文本/audit SHA、适用scope、署名/修改/同许可义务与页面例外；作者/两独立reviewer/有分歧时独立arbiter的可核验材料|

正式发布前需新增经过审阅的内容审核绑定格式：每条review绑定完整注释/关系内容、固定来源授权和处理协议摘要；任一变更使审核失效。**现有v1只验证双审声明和引用/公式，不含生产内容签署协议，不能据其`verified=true`发布。**本轮不静默修改已有v1侧包以冒充已完成迁移；研究图的原始POS/义项/翻译声明不自动转成正式侧包。

R7人工IME工作流已修复包级处理协议绑定，与语义侧包独立。IME的`review_binding_sha256`绑定注释协议、预处理、lineage审核；旧R6审核缺字段必须真实重新审核，工具不得代填。真实12任务继续pending，无人员、许可、答案或封存。B正式70/15/15、至少2000heldout家族及加密独立保管的既有门禁不变。

## 发布与恢复

正式侧包必须先有合法材料、内容审核、签名/可信版本清单、与词条包的精确外键和升级迁移契约；再设计发布器及独立验证，不复用词条数量作为语言验收。当前没有这些生产材料或侧包发布器。

现有词条原子发布/损坏拒绝/回滚/显式恢复保持。测试运行真实临时文件和子进程，用户词库不写入这些目录；不能由夹具恢复推断物理断电、签名升级、用户词库跨版本迁移或新侧包兼容已验收。
