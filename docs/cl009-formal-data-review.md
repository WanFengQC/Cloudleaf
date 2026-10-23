# CL-009 第三轮：正式数据来源候选与质量门禁

2026-10-23；实施基线 `ed9516bf74e36649b0dc415850c65792732c10d4`。这是取得正式数据前的审查及验收计划，不改变来源锁、数据格式、默认词典、生产音节表或运行时算法。原七条审核记录及其清单哈希保留。本轮没有下载新词库或语料，也没有联系来源作者；候选不是授权通过或数据交付。

## 固定范围

44,431条Unihan记录仍是字符常用读音/原始释义；701条Wiktionary记录仍是固定8MiB前缀的研究摘录。后者全件SHA、gzip全件CRC和逐页例外复核未完成，不能改成正式词库或独立评估集。频率未知保持null/unknown，不从行号、词头出现次数、单字统计或不同语料的计数推测。

下表把官方许可表述与项目采用决定分开。`blocked`表示具体未决项没有关闭，不表示来源违法；`candidate`表示值得进一步审查，不表示已经批准取得或再分发。所有网页复核日期为2026-10-23；未取得固定包的SHA明确为null。动态网页日期或条目数不当作固定原件版本。

## 词条与读音候选

|来源/具体版本|官方来源与取得方式|许可原文、商业/修改/再分发、署名与例外|项目决定与下一证据|
|---|---|---|---|
|Unicode Unihan17.0.0；UAX38 revision39|[官方ZIP](https://www.unicode.org/Public/17.0.0/ucd/Unihan.zip)；已有本地完整锁定原件，纯离线重建|[Unicode License V3](https://www.unicode.org/license.txt)允许使用、修改、销售和分发；保留copyright与permission notice；不得暗示Unicode背书|仅已锁定字符范围通过。不是多字词读音、现代频率或完整多音字；18条隔离读音见后文|
|CC-CEDICT；本轮官方页观测release 2026-10-21 05:46:59 GMT，125,295 entries；原件SHA=null|[发布页](https://www.mdbg.net/chinese/dictionary?page=cedict)列ZIP/gzip；同页禁止脚本/自动访问。只接受另经审核的合法本地原件，不调用下载或换镜像|官方明确[CC BY-SA4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode.en)：商业/修改/分发有条件允许；CC-CEDICT贡献者/MDBG来源链、许可链接、修改说明、派生数据同许可|`blocked-acquisition`。未固定原件，未校验其实际头部、版本和额外条款；现有本地导入器不能代替取得授权链。没有统计词频|
|中文Wiktionary raw，dump2026-10-01，extract2026-10-20；代码c1bb342/6f96ddf；仅前8MiB已锁定|[发布者raw说明](https://kaikki.org/zhwiktionary/rawdata.html)与既有固定前缀。没有重试完整下载，不用postprocessed合并来源|文本按[Wikimedia ToU第7节](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use#7._Licensing_of_Content)及CC BY-SA4；贡献者、逐页/历史URL、提取链、修改说明与同许可；媒体、引用及导入例外需单独审核|仍`approved-bounded-research-only`，正式范围blocked。原件完整性和逐页署名/例外不能由ToU总则保证；不将研究范围批准扩为正式批准|
|THUOCL，commit `a30ce79d895d01ab5132a5c74c29703ff7efb4cc`；领域文件更新2016–2017|[固定README](https://github.com/thunlp/THUOCL/blob/a30ce79d895d01ab5132a5c74c29703ff7efb4cc/README.md)；未来如采用仅官方Git固定commit，当前未取得数据包|[固定MIT原文](https://github.com/thunlp/THUOCL/blob/a30ce79d895d01ab5132a5c74c29703ff7efb4cc/LICENSE)允许商业/修改/分发，保留THUNLP版权/许可；README另明示研究/商业开放、科研引用作者|`candidate-provenance-review`。官方说明词表含网站标签、热词及输入法词库；采用前核实可分发来源链，符合核心不复制他人实现的边界。没有词语读音；DF是含词文档数，不能当token频率或填补现有unknown|

## 频率与统计语料候选

|来源/具体版本|官方来源与取得方式|许可、署名及例外|项目决定与下一证据|
|---|---|---|---|
|Google Books Chinese2019，`googlebooks-chi-sim-20200217`；包SHA=null|[官方Ngram说明](https://books.google.com/ngrams/info)列固定ID及bulk入口；本轮只读说明，未取得分区包|FAQ允许图表/数据用于任何目的，希望署名并链接Google Books Ngram Viewer；这不是本项目已逐包核实全部再分发/派生条件的证据|`candidate-package-review`。bulk页面未可靠取得；先核对具体版本/格式、年计数、分母、分区与条款。中文自动分词、书面/OCR及至少40本书阈值有偏差；不视为当代输入概率，不用图表百分比替代原始统计|
|SUBTLEX-CH2010，DOI `10.1371/journal.pone.0010729`，Files S1；包SHA=null|[作者论文与附录入口](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0010729)；未下载|论文为CC BY，但正文给频率文件的用途写research purposes；wordfreq作者的[NOTICE](https://raw.githubusercontent.com/rspeer/wordfreq/master/NOTICE.md)声称取得“在wordfreq内”分发的邮件授权，要求SUBTLEX作者署名及freely available说明|`blocked-license-scope`。不把论文许可或别人收到的邮件扩为Cloudleaf独立包的授权；商业修改/再分发范围待具体文件许可或权利人澄清。语料年代/字幕偏差也需独立记录|
|wordfreq，审查commit `912caf64b657478d1dff1138efdc078947d54bb1`；数据约截至2021|[官方README](https://github.com/rspeer/wordfreq/blob/912caf64b657478d1dff1138efdc078947d54bb1/README.md)；未安装或导出|[NOTICE](https://github.com/rspeer/wordfreq/blob/912caf64b657478d1dff1138efdc078947d54bb1/NOTICE.md)区分Apache2代码与CC BY-SA4数据；署名Robyn Speer及各来源，SUBTLEX许可另有范围条件|`blocked-provenance`。README明确中文Jieba附表provenance未知，组成数据与可分离性未清；顶层许可证不能自动清除该缺口。禁止调用默认minimum=0把未收录词误写为实测零|
|Leipzig中文频率；具体资源ID/版本/SHA=null|[官方资源目录](https://wortschatz.uni-leipzig.de/en/download)及[许可规则](https://repo.wortschatz-leipzig.de/licenses/en)；当前未选择/下载具体包|目录说明数据通常CC BY-NC，例外须在具体资源页明示；元数据public domain不能替代数据许可。NC版本、修改/再分发细则及署名依具体包核实|`blocked-commercial-scope`。在明确兼容商业的具体例外或独立授权前不采用；不将“通常”写为某包已核实的许可，不从搜索结果猜版本|

## 独立评测与双语候选

|来源/具体版本|官方来源与取得方式|许可、署名及例外|项目决定与下一证据|
|---|---|---|---|
|UD Chinese-GSD r2.18，tag解析commit `e0d85a020182e264d6384be2a59c0f4879a1cc35`|[固定README](https://github.com/UniversalDependencies/UD_Chinese-GSD/blob/e0d85a020182e264d6384be2a59c0f4879a1cc35/README.md)；仅查看元信息/许可，未取train/dev/test或答案|[固定LICENSE](https://github.com/UniversalDependencies/UD_Chinese-GSD/blob/e0d85a020182e264d6384be2a59c0f4879a1cc35/LICENSE.txt)列CC BY-SA4；署名Shen/McDonald/Zeman/Qi及文本来源。README特别说2019去NC只适用于Google的UD标注，不涉及其不主张权利的底层文本|`blocked-underlying-text-and-gold-review`。存在拼音转写不证明独立人工IME答案；先补原句权利链、转写来源/错误审查、重复及来源家族隔离，保留繁体范围。不能只看首页许可证放行全包|
|Tatoeba普通话/英语文本；可变周导出，固定批次/SHA=null|[官方导出](https://tatoeba.org/en/downloads)支持句对与周导出；[ToU](https://tatoeba.org/en/terms_of_use)2022-04-24生效。本轮未下载句子/转写/heldout|默认文本[CC BY2.0 FR原文](https://creativecommons.org/licenses/by/2.0/fr/legalcode)，部分句子CC0；按逐句许可可商业使用、修改、分发，保留作者名、句ID/URL、许可和修改信息。音频另有逐人条款，本轮排除；站方不保证逐句权利无误|`candidate-snapshot-and-annotation-review`。下一步需保留含作者和license字段的完整批次，链接只作句对关系。人工确认普通话读音/可接受输出并独立复核；翻译句不是多义词典，平行句不是现成IME gold|
|Wikidata Lexeme结构化数据；官方策略页oldid2479770906；具体dump版本/SHA=null|[官方数据库说明](https://www.wikidata.org/w/index.php?title=Wikidata:Database_download&oldid=2479770906)提供lexemes JSON/RDF批量入口；未下载|[官方授权范围](https://www.wikidata.org/wiki/Wikidata:Licensing)及[CC0原文](https://creativecommons.org/publicdomain/zero/1.0/legalcode.en)：结构化Lexeme可商业/修改/分发，CC0无强制署名/SA；本项目仍保存实体/修订/来源。讨论页正文为BY-SA、媒体及第三方权利不自动CC0|`candidate-coverage-and-snapshot-review`。先固定dump并检查普通话lexeme、form、sense和明确翻译关系；跨语标签/多语gloss不能推导等义关系，缺POS/读音不猜。尚无覆盖、对齐和双向质量证据|

## 数据结构与质量验收计划

现有JSONL schema3、TSV v2、nullable频率观测和稳定来源ID不变。正式词/短语包获准前，逐条保留词头、语言/文字体系、读音原文及分段、来源记录/行摘要、明确词性、带语言的义项、署名/许可映射。不同读音、来源冲突与多义不通过文字去重抹平；双语关系保留方向、义项ID和来源证据，不能仅用两个标签相等建立翻译。

|门禁|采用前需要的可复查证据|当前情况|
|---|---|---|
|来源与权利|100%接受记录可追到固定原件、许可、可取得方式和署名链；例外单列隔离，未知权利不得进入正式包|仅字符范围通过；词语/频率/双语候选未完成|
|确定性/结构|干净导出、独立解释器双构建九文件一致；接受+隔离+跳过可复核；ID稳定、索引/投影/事实一致；坏UTF-8/冲突不能静默容错|现有固定两来源可重放；新候选尚未导入|
|读音|保存规范依据与原件，不把IPA/方言或单字拼合当词语读音；未支持音节隔离；人工分层审读与仲裁记录|18条只完成字段语法/当前表影响审计，未完成扩表决策|
|频率|观测包含kind/value/unit/corpus_id/period/分母/切分及许可；DF、token count、Zipf分别定义，跨来源不直接加和；缺失继续null|两个现有包全部unknown，未批准现代统计源|
|双语|明确源/目标语言、义项与词性，不将中文释义投影为英语；人工抽检常用/专名/多义/短语/方向，阈值先登记再看评测|216条显式英译不满足正式双语验收|
|独立语言质量|在看heldout前与B共同冻结样本量、分层、阈值和参数选择。按来源/意图家族去重划分train/dev/heldout，人工独立双审/仲裁并登记一致率；OOV/非法/超预算留在分母|未取得获准独立标注集；language_accuracy_valid保持false|
|发布/恢复|固定manifest、旧版本保留、损坏拒绝、真实进程中断后current完整、用户词典不变；记录失败前后状态与PID身份|本轮复测现有工具；物理断电、签名安装器、跨版本用户词典迁移未验|

词典来源可以在生产词库内出现，但评估**答案与读音、输入事件及容许输出**必须独立于被测候选生成，不从词典生成gold。Tatoeba的译文/重复句、UD同源平行树库和Wiktionary派生例句必须按家族处理，不能随机按行分割伪造独立性；heldout不能用于补词、改音节或调参。合成压力数据只验证结构/资源。

## 18条合法隔离读音的复核结论

本轮重跑[既有逐条审计](reports/cl009-reading-audit.md)，18条在[UAX38 revision39的kMandarin](https://www.unicode.org/reports/tr38/tr38-39.html#kMandarin)字段NFD语法内，来源没有因此变成坏记录。7条归为成音鼻音/叹词组（n、m、ng、hm），11条为当前表的字母音串缺口（rua、fiao、nun、cei、wong、din、biang、bong）；重复字符读音仍分别保留，不缩成8条。17字符缺少运行时读音，嗯另有en可用。

“字段合法”和“正式现代普通话输入键编码”是两个决定。UAX说明常用读音的用途，不单独证明上述每项适合本产品的现代标准读音/变体政策；采纳前逐项核对规范/词典依据、声调与分段、成音鼻音和首字母编码的歧义，并与B共同评审。证据不足继续隔离，不能近似替代或静默删除。

本轮交付来源审查计划与现有数据重放；CL-009的正式词库、频率、独立评估和完整双语仍开放，CL-010不启动。真实命令、版本、性能及中断证据汇总于[第三轮报告](reports/cl009-round3.md)。
