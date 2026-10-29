# Round 8 来源采用决定

2026-10-30。本页基于现有固定许可/原件和本轮一次Google官方候选核对，不把再次列矩阵当成取得新授权。已有`data/sources/source-review.jsonl`和R7记录保留原样；新证据是补充审阅，未新增production来源锁。

|来源|现可采用的内容|本轮决定与尚需外部材料|
|---|---|---|
|Unihan17固定原件|既有已批准字符读音与原英文释义；Unicode-3.0|保留原范围。不能变现代词频/词短语/POS；原zip实际缓存与SHA核对。|
|CC-CEDICT固定镜像|既有已批准词头、读音、原英语义项与合规派生；CC BY-SA4|保留原范围。20项待办仅使用这批许可原文；新语义质量、POS/对应关系和独立gold未验收。|
|Google Books Chinese2019/20200217|本轮具体官方中文1gram索引明确标CC BY3，许可出处已找到|选为下一次有界取得与口径核对的首选候选；不取bulk分区、不批准production。缺固定原件SHA/CRC/全部分母、v3行格式/筛选与年份协议，书面统计代表性另评。|
|Wikipedia固定自然数revision|已有正式正文身份和候选许可资料|页例外、全部作者、提取/分词未齐，继续不批准统计用途；不重复取得全文。|
|Wiktionary固定前缀|原批准范围仅bounded资源/一致性研究，CC BY-SA4|research_only不变；缺完整原件SHA/CRC和逐页例外审核，不把研究POS/翻译声明升为生产内容。|
|SUBTLEX-CH|无已获准现代统计原件|商业/再分发范围待明确，不下载或导入；可由外部数据负责人提交特定原件及授权。|
|wordfreq-zh|无已准中文组成来源完整链|中文来源及可分离性待明确，继续不采用；代码许可不代数据许可。|
|Leipzig候选|无已准特定bulk原件|商业/分发范围待核，继续不采用，不重复广搜。|
|CC-CEDICT官方网页取得|数据许可与网站自动取得条件分开|仍不自动请求MDBG网页/API/export；现固定镜像已有独立许可与完整zip证据，不因此绕过站点限制。|

现有固定原件、锁和许可：`data/sources/locks/unihan-17.0.0.json`、`cccedict-20260604-mirror7019e1c.json`、`wiktionary-zh-20261001-prefix8388608.json`；完整许可见`data/licenses/Unicode-3.0.txt`和`CC-BY-SA-4.0.txt`，镜像原件说明见`data/sources/evidence/cccedict-mirror-7019e1c/upstream-documents.json`。本机缓存路径/SHA写入本轮identity，路径不是其他机器的默认值。

正式频率负责人交付时须分别声明token occurrence、包含词条的文档数、总tokens/总文档数、时期、分词和缺失/截断语义。原始book-volume count不直接当token；跨年相加不自动等于去重文档数。没有观测仍unknown/null，没有新增现代频率值。
