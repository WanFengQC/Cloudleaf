# Round 23 可追溯语言资源

**partial**。现用固定基础词典 123,544 条保留，生产词频均为 unknown。训练记录、离线模型和语义投影留在 Git 外。

| 来源 | 固定材料与用途 | 准入 |
| --- | --- | --- |
| CC-CEDICT | 2026-06-04 固定镜像，ZIP SHA-256 `fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`，双语原始释义，词典身份独立 | 既有许可范围保留，CC BY-SA 4.0 要求署名、变更说明及相同方式共享。生产义项、关系和人工语义审核仍待交付。[MDBG](https://www.mdbg.net/chinese/dictionary?page=cc-cedict) |
| Unihan 17.0.0 | SHA-256 `f7a48b2b545acfaa77b2d607ae28747404ce02baefee16396c5d2d7a8ef34b5e`，字符读音与原始英文释义 | 保留 Unicode 3.0 许可与原审批范围。[Unicode](https://www.unicode.org/license.txt) |
| Tatoeba | 2026-10-31 固定周导出，原始 bz2 `374dc649bd61ed5597c58ad9f098edb831e7bbe4ea0831f82947bec1fe5c8bb8`，兼有原创与翻译句 | CC BY 2.0 FR 研究。当前所有者不能自动作原始作者；本轮生产与再分发暂不放行。[Tatoeba](https://tatoeba.org/en/terms_of_use) |
| 中文维基新闻 | 固定文章与各来源版本由既有锁保留，仅作新闻开发对照 | 历史页面许可与归属逐页登记；训练与冻结工具不从开放页面补抓内容。[维基新闻版权信息](https://zh.wikinews.org/wiki/Wikinews:%E7%89%88%E6%9D%83%E4%BF%A1%E6%81%AF) |
| 中文 Wiktionary | 固定不完整前缀研究来源，旧版隔离状态沿用 | 原件不完整且发布权限未通过。 |
| Google Books Ngrams | 固定中文切片 508,674 行、19,074,163 个有效三元统计记录、464 行分母，执行原件 SHA 和 gzip 完整校验 | Google 公开派生统计许可与源书权利分别记录。既有词汇模型属历史研究对照，不用作本轮真实频率的分母。[Google 数据导出](https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/chi_sim-1-ngrams_exports.html) |
| 中文 Wikipedia | 固定自然数条目 rev 94816134、SHA `56e173a1f64ee94c13e1061e3cc970ebe3153e75f8e8cffc50c8293073392f12` | 验证原件后返回准入 2，权利、作者链及分词协议未解决。 |

## 真实研究观测

运行 `scripts/build_word_context.py` 及 `scripts/freeze_word_context_validation.py`。两份干净构建的 TSV 和回执逐字节一致。保留 84,959 片段与 65,615 个关联家族，全球来源家族仍只有 1。剔除相同训练片段 1,768 条和 10 条跨原划分完全重复。词语 541,722 个，含 unknown 102,400，观察相邻关系 626,681 次。当前只有历史 Han 片段用于训练，原文本仍保留其它字符，因此本轮未证明中英混合训练覆盖。

实际相邻／一元表 172,549 行，2,619,128 字节，SHA-256 `f8062a4a8e456135840b9df51857c84e3ea3f16f59ced4f9dc85a5bd19fade3a`。21,974/123,544 词条共享观察表面，101,570（82.21%）未观察。观察次数与表面词相关，未作为同字形多个读音的独立观察，也未写回生产词典。正式 token frequency、document frequency、Zipf 均为 null。未取得合规的多域现代统计语料。

补充词典完整重训练触及 200,000 行上限并被拒绝。固定基线字节前缀校验后，只进行同观测数据的显式重绑，新增词表面仍 unknown。冻结 40 个内部连接家族，基础与补充分别可映射 25 和 40 条。本语料与训练共享同一全球来源家族，近重复核验不构成独立的跨域 gold。

## 实际语义查询

导入 PR3 的发布、验证和恢复工具，旧词典与现有生产审批保持。固定研究选样有 34 个词条、78 条原始释义、235 项发布缺件。Release 可按稳定词条 ID、中文词头、英文精确原始释义或英文词项检索，返回原始来源及多义项。真实 `nihao` 候选的注释不改变输入帧或学习，Backspace 后拒绝旧请求。英文词项匹配只返回原始释义。已审核有向关系、POS 和短语关系依然缺失。生产验证和激活均返回 2，未创建发布目录。

机器可读[统计回执](integration-round23-evidence/round23-public/word-base-receipt.json)与[语义回执](integration-round23-evidence/round23-public/semantic-query-proof.json)记录资源身份。
