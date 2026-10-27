# Round 5：频率、语义侧包与人工IME评测的采用边界

2026-10-27。配合[先行设计](cl009-round5-design.md)与 `cloudleaf-linguistic-sidecar-v1` 离线验证器；不是现有runtime格式变更。没有真实统计、POS或语义标注的记录保持null/空集合；当前123,544条仍无正式频率/POS，不把类型验证器等同数据交付。

## 统计口径与验证

|metric|实际含义|分母/单位|拒绝的混用|
|---|---|---|---|
|token_frequency|某entry的出现token数除以全体token数|`tokens`；固定corpus/version/SHA、时期和tokenizer/config|DF计数、词典收录数、缺词默认0不可当token比例|
|document_frequency|含该entry的文档数|`documents`；明确文档单位/总数，value保留原计数|不能直接解释为token概率；不同文档单位不直接相加|
|zipf|每十亿token频率的log10|`per_billion_tokens`，原生定义SHA或同词同源同语料同预处理的正token母观测|不能由DF推导、不能对真实0取对数、未收录默认0不是观测|

`coverage=observed` 的计数0必须真为numerator0；正计数的float下溢不准变观测零。`unobserved/below_cutoff` 的value/numerator/denominator均null。原生Zipf=0可能是有效观测，但须明确observed、来源定义和基准；不能由库的缺失默认值生成。整数拒绝bool，数值拒绝NaN/Infinity；已知正比例不因绝对误差容忍为0。派生Zipf母/子须同entry、source、corpus全部身份（含时期/分词/配置），不得只改标签。

频率是领域与年代受限的观察，不自动等于候选排序权重。侧包尚未接入B研究评分或默认核心；正式采用需先审权利、统计代表性、开发集调参和封存集评估。

## 本轮实际来源决策

原始检索身份及失败见 `reports/cl009/round5-source-review.json`。以下仅审元信息，未下载这些候选语料/频率/句子/heldout包，也未新增批准来源锁。

|候选|本轮primary依据|决定与下一证据|
|---|---|---|
|THUOCL `a30ce79…`|固定README明确DF及2014–2016博客、2008–2016新闻和搜狗；固定MIT与研究/商业许可|保留独立DF候选；词表含其他输入法词库/标签，组成来源待核。不是现代token包，不填unknown。保留THUNLP许可/版权与科研引用要求。|
|wordfreq `912caf6…`|固定README定义token比例/Zipf及缺词minimum；中文Jieba附表provenance未知，统计约截至2021；NOTICE区分Apache代码与CC BY-SA4数据及各作者|中文组成来源未闭合，继续blocked-provenance，不安装或导出；不把缺词默认0当统计。|
|SUBTLEX-CH2010|作者论文区分词token计数/电影上下文多样性及ICTCLAS2009Share+PKU；论文CCBY与研究用途表述、wordfreq特定邮件许可并存|独立Cloudleaf数据包商业修改/再分发范围仍未确定，blocked-license-scope。论文许可不自动解决特定附录原件授权，未取数据。|
|中文Wikipedia `20261001`|官方固定dump索引列完整3.4GB多流文本；官方dump许可默认CC BY-SA4/GFDL但逐页例外/侵权风险与ToU优先|**选为后续自产token统计的条件候选**，非批准的现成频率。须先固定完整原件SHA、修订/作者/许可、逐页例外过滤、文本抽取与分词器；百科领域不能冒充日常输入分布。本轮只取索引/校验元信息，没有取3.4GB原件或部分包。|
|Tatoeba文本/句对|本轮ToU读取超时|保持旧条件候选，未取得新许可确认/固定原件/逐句作者，也不改作人工IME gold。|
|自有授权人工任务|项目未来直接授权/同意+人工意图与输入任务|优先的人评取得路径，当前没有受聘/授权标注者、许可或结果。未自动联系外部作者，不用词典派生答案代替。|

Wikimedia取用还须遵守User-Agent和连接上限；自动取得允许性与内容权利分别处理。dump索引中XML计划弃用提醒只影响未来采集设计，未导致本轮悄悄更换包。许可声明、语料完整性和语言代表性三个门禁均需独立证据。

## 词性、义项、翻译及短语模型

侧包顶层固定format/schema_version/synthetic/lexicon_sha256，绑定事实层SHA与已知entry ID集合；sources逐项含固定版本、原件/许可/审核SHA及scope。`entries[].pos`缺失时null，非空POS须原tag/tagset版本/来源证据和双审。`senses`有来源或人工明确的稳定ID、entry引用、语言、定义、独立证据；旧CC-CEDICT斜线段不自动生成稳定语义ID。

`translations`引用两个已存在sense的有向多对多关系（equivalent/broader/narrower/related）；记录一向不自动补反向，语言/同义真实性仍须人工。`phrases`保留entry、idiom/collocation/multiword类别、按原顺序的组件引用及来源证据，不将全部多字词自动当搭配。所有非空注释需要两个不同reviewer；有争议还需不同仲裁者。验证器只能核文件声明和关系/数学一致性，**不能证明这些名字是真实独立人员、许可真实或语义正确**。

本轮仅自编fixture演练这些结构，没有将fixture写入正式包。正式POS、义项、翻译对齐和短语注释数据尚未取得；没有真实sidecar时不存在这些能力。

## 人工IME gold与B的依赖

[实际计划](../data/plans/ime-human-gold-v1.json)明确blocked：artifact、许可、标注者、仲裁者、保管者、封存答案和预处理协议SHA均空。先合法取得原文/意图，人工制作可接受输出和完整按键轨迹（含编辑/取消/OOV与所支持方案），再独立双审/争议仲裁。来源家族、规范source unit、跨来源near duplicate必须全局防泄漏，heldout不能调参或补词。

保管者独立于开发者，封存答案后冻结数据、参数、代码、二进制、词典和协议；B既有70/15/15家族与heldout至少2000家族只是前置依赖，不是本轮满足、最优比例或人工结果。C计划与B `cl008-round5-quality-preparation.md` 的lineage/artifact/preprocessing/review字段须将来显式转换、审核；当前没有自动连接或兼容证明。C语言侧包不是gold，也不能解除B质量门禁。
