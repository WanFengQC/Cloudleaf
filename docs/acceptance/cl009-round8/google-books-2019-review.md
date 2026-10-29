# Google Books Chinese 2019：具体bulk许可已定位，数据未取得

2026-10-30。只核对既有`google-books-ngrams-zh2019`候选，不广搜、不下载bulk原件、不新增source-lock。Google官方总说明和中文1gram下载索引、CC官方许可文本的原字节留在本机`.local/round8-source-documents/`；取得时间、HTTP和SHA见[证据元数据](google-source-evidence.json)。整页HTML和许可正文未重复随Git分发。

## 本轮真正消除的缺口

此前只有一般FAQ用途说明，不能代替具体bulk许可。现官方datasets入口重定向到[bulk总目录](https://storage.googleapis.com/books/ngrams/books/datasetsv3.html)，其版本3为20200217，直接链接[Chinese 1gram具体索引](https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/chi_sim-1-ngrams_exports.html)。该具体索引直接标明CC BY 3.0，不是把Viewer图表许可外推到不明数据。

[CC官方BY 3.0许可](https://creativecommons.org/licenses/by/3.0/legalcode.en)第3节提供复制、改作和分发权利；第1(c)的分发定义包含销售，第4节要求保留许可/署名等条件，第3(b)要求识别修改。就该官方声明覆盖的编纂数据而言，存在附条件商业/改作/再分发的明确许可依据；实际分发须按相应原文履行，不混标MIT或CC BY-SA4。本轮没有代项目批准生产采用，也不把索引许可当原书全文授权。

## 固定版本与取得方案

[官方语料说明](https://books.google.com/ngrams/info)将Chinese2019的persistent id列为`googlebooks-chi-sim-20200217`；这与当前Viewer较新语料分开，不能混版本。具体索引只列一个1gram分区和其totalcounts文件。HEAD只核可取得性，未读取任何body：

|资源|官方索引列出的URL|HTTP / Content-Length|原件SHA/CRC|
|---|---|---|---|
|1gram|`https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/1-00000-of-00001.gz`|200 / 60,833,224B|未取得，null/false|
|同版本总计数|`https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/totalcounts-1`|200 / 7,479B|未取得，null/false|

Content-Length、ETag和Last-Modified仅是这次HEAD观察，不等于完整SHA、发布者签名、gzip完整性或有效行数。后续取得须先确认执行范围/存储预算，再保留完整原件、实际传输SHA和gzip CRC及解压大小，不把先取得部分前缀作为完整语料。

## 单位和仍未消除的阻断

总目录的**Version 2示例**把match_count解释为出现次数、volume_count解释为不同书籍数，并给出总1gram计数用于相对频率。它不是v3实际行格式的已验收解析合同；本轮不写v3导入器或凭v2示例填值。对v3应先核原始记录与同版totalcounts，逐年匹配token分母及不同book/doc单位，再预登记年份/标签/标点/去重/分词口径。

官方页面对截断的措辞还须区分：Viewer FAQ说至少40本书，总目录在介绍v1/v2时说全语料超过40次。两者不能无依据地当作2019 v3相同门槛，也不能将未出现的词当观测零。未知/低于截断需区别，原件和特定版本协议未闭合前保留null。

官方说明中文使用统计分词；具体实现/版本/参数和代表性尚未交付。书籍文本不等于口语IME使用分布，旧年代、OCR、自动标注和取样局限要分开评估。token比例不从筛选后分区的计数和猜分母；book DF不冒充token，也不跨年随意相加为去重文档数；Zipf仅能从同词/同源/同范围正token比例派生。

**决定**：将该固定版本列为下一次有界取得和口径核对的首选候选，具体bulk许可出处这一项已落实；原件、协议、项目采用/分发审核和语言质量仍未通过，production继续不批准，所有频率仍unknown/null。未批准来源的限制不因本轮证据而取消。
# 第二阶段补充入口

本文件的 metadata-only 结论固定于 `3d02431`，`google-source-evidence.json`保留当时body=0记录。随后按[先提交的有界设计](bounded-acquisition-design.md)实际取得两个固定候选原件，完整SHA/CRC/实际v3多组三元组行形状见[阶段增补报告](../../reports/cl009-round8-status.md#第二阶段增补固定候选原件已实际取得)。这关闭取得身份缺项，不批准生产、不改变unknown或统计协议状态。
