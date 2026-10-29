# CL-009 Round 8：真实语义待办、候选许可取得与发布器设计

下述主体是第一阶段 metadata-only 检查点，已完整提交于 `0f65339`；后续有界原件取得见文末增补，不将早期 HEAD/body=0 证据改写成完整取得。

2026-10-30。起点`3d573384ce1f31e437bdb4e7dc109cf438c261f4`，设计先行`74f30953d396722087e9643d2dd2946df9e69b53`；外部材料冻结并校验于 **`3d02431fadbf349afe513525e81ba407d5947e09`**。其后只归档报告，不改绑原结果。生产代码、测试、来源锁、正式词条包和原发布/恢复机制均未改。本轮没有把重复回归或新增测试数量当作进展。

## 实际交付与文件范围

|交付文件|可由外部人员执行的工作|
|---|---|
|`data/plans/round8-external-handoff.json`|唯一主角色/授权入口：5个角色null、9项材料pending/path/SHA均空；引用现12任务与B自己的上下文/封存合同|
|`docs/acceptance/cl009-round8/README.md`|取得→授权→真实语义双审→协议冻结→独立保管→发布审查的顺序和具体缺件|
|同目录`semantic-review-queue.jsonl`、NOTICE/LICENSE|来自现获批CC-CEDICT的20项真实原始义项待审请求；source_record_id/原行/事实SHA/署名绑定，所有新语义和审核为null/pending，非IME gold|
|同目录`source-decisions.md`、`google-books-2019-review.md`、`google-source-evidence.json`|保留既有批准范围，明确未采用来源；只对已选Google候选精准核官方bulk许可/版本及HEAD元数据，未下载数据分区|
|同目录`semantic-publisher-design.md`|版本化内容/目录/审核/清单、失效、签署信任边界、复用原guard/不可变release/单指针/回退恢复、兼容及用户字库隔离；是设计而非已实现发布器|

未新增或修改单元/集成测试，未修改Rust/实时输入/Python生产工具/共享接口。现词条schema2/3、TSV v2、123,544 runtime和1,468隔离、unknown语义以及12项IME pending均保持。

## 语义原文已可交给实际审核者

20项按固定事实包稳定entry ID排序取得，不是随机样本或语言质量分母。逐项保留原词头、原读音和原始English senses；独立对照原zip member行、source_record_id和事实JSONL原行SHA。没有把英文释义猜成英语目标词头，也没有生成POS、义项ID对齐、反向equivalent、短语、频率或审核结果。

队列原字节SHA **`d7561fea9bf650edef74eb5f310a9f0586e536a7343364718c75192261d3080a`**。全部`status=pending/publication_eligible=false`。队列为CC BY-SA4派生材料，保留贡献者/MDBG/Paul Andrew Denisowski和固定镜像Lester Lin署名、来源、许可与本次抽取说明；不混标MIT。许可副本仅规范末尾多余空行，原许可SHA`28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5`与展示SHA`23ee78c8bae49cf08ea2f0c84945c66b987ebe4520881fb51b3dad4fb43d07c2`分别保存，未编辑条款。

这批待办只为获准原义项的真实语义审读，不是独立人工IME gold。真实作者、两位独立reviewer、独立arbiter和heldout保管者及授权仍待定；工具没有代填名字/签名/答案。C原13字段摘要和三协议review binding不变，B引用主登记并独立准备context/intent-family及seal；B旧配额/封存门禁不变，正式Top-1/Top-5/MRR继续null。

## 本轮新增的来源证据

R7只有Google一般FAQ和未核bulk条件。本轮限定查看官方info→bulk目录→具体Chinese1gram索引，以及CC官方BY3许可，无广泛来源搜索。[具体中文索引](https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/chi_sim-1-ngrams_exports.html)本身直接标CC BY3，解决了具体bulk许可出处缺项；[CC官方许可](https://creativecommons.org/licenses/by/3.0/legalcode.en)第1(c)/3/4节提供附条件复制、改作及分发依据，包括销售定义与署名/许可/修改识别要求。不是从Viewer图表说明猜许可，也不当作原书全文授权。

官方persistent版本为`googlebooks-chi-sim-20200217`。已取得说明/索引/许可原件并固定HTTP/时间/SHA，正文留本地而不复制到Git：

|官方材料|实际字节 / SHA-256|
|---|---|
|info页|482,739 / `a6be62ab51663f1fed92b8e2f865b0382e80ed02b13fdd6a15731dcddaffb886`|
|datasets官方入口→datasetsv3.html|5,220,205 / `b2f66a50d5713f91e181c8b5f685e03c9ceda80c2ce9ab7d69083d69a0f4bf50`|
|Chinese20200217具体1gram索引|2,875 / `cd676d9817f7cc9ceb515f4610e01574c1e7d06e32127e0ea23a91772a701f17`|
|CC官方BY3文本|19,467 / `e6bc9e9c474700b708f568bac9e5a8a9bcb2b1dad53442f5ba449fcb848b8e76`|

取得说明页设6MiB上限，索引/许可各1MiB上限。web工具对5.2MB目录返回内容过大，改用有上限的标准库HTTP只读取得；没有取得任何bulk body。索引所列两个资源另仅HEAD：1gram gz HTTP200、Content-Length60,833,224；totalcounts-1 HTTP200、7479。ETag/Last-Modified可核，但**body读取0、原件SHA为null、gzip CRC=false**，不以HEAD冒充完整数据。

具体v3行格式/分母/时期/筛选仍需原件和协议。总目录的Version2示例将match_count解释为出现次数、volume_count为不同书籍数，不能直接视作v3已验收解析合同。FAQ的40本书与总目录介绍v1/v2的超过40次不能自动当同一2019阈值，缺行不能当零。token比例、book/document DF和Zipf分开，实际值保持null；书面语料不是口语IME分布。该候选可列为下一次有界取得首选，**仍未批准production，未创建来源锁或原件**。

Unihan、CC-CEDICT固定原件均在既有本机缓存找到且核SHA/完整性；许可采用范围与语义质量分开：

|原件|本机已核路径 / 字节 / SHA-256|
|---|---|
|Unihan17|`.local/data-sources/Unihan-17.0.0.zip` / 8,518,517 / `f7a48b2b545acfaa77b2d607ae28747404ce02baefee16396c5d2d7a8ef34b5e`|
|固定CC-CEDICT|`.local/round4-source-cedict/cccedict-20260604-mirror7019e1c.zip` / 3,975,327 / `fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`|

路径相对本方向工作区，绝对路径和九包文件SHA见[原件identity](cl009/round8/materials/identity.json)。Wiktionary继续research_only；Wiki页例外/全部作者/提取/分词未齐；SUBTLEX/wordfreq/Ngrams生产采用/Leipzig不因本轮文档而批准。没有新现代词频观测或正式语义内容。

## 生产发布器设计与实际未实现

设计在现有结构侧包之外明确内容/来源授权/协议摘要和真实双审仲裁绑定、改版失效、固定资源清单及外部可信签署；SHA、Git分支或自签key不替信任根。英语目标目录/义项未提供时不能拿中文entry ID或原英文释义猜对齐。复用现有guard、不可变release、原子单指针、previous与显式recover思路；现verify_release严格接受原词条格式，新bundle verifier/reader及发布器均待实施与审查，现store不迁移、不激活。

用户词库/学习/隐私配置不在bundle资源或清理范围。没有实际内容/授权/签署根，候选流程停止，不创建production包或调用activate/rollback。该设计不是签名、迁移、物理断电、正式数据或平台验收。

## 实际检查、身份与保全

[六项真实命令](cl009/round8/verification/checks.json)为rustc版本、fmt、资源/许可审计、新材料核对、原12任务audit、diff，返回 **0/0/0/0/2/0**，均符合预期。材料核对覆盖5角色null、9材料pending、12旧任务pending、20原义项/源行/事实SHA、新语义全部空、官方4说明身份和两个HEAD body0，以及本地文档链接。audit2是缺真实材料的拒绝，不是正式语言门禁通过。

生产代码和测试没有变化，因此没有重复R7的60 Rust/89 Python或全包双构建/380秒压力，不新增测试来增加数量；原证据仍绑定R7真实提交。新最终HEAD的Windows/Linux push CI另由共享R8汇总核验。保留的本机release CLI实际SHA`d53f54a25bd6b4fb8c114a06966010d4b98bf0597efb7e9c5e897e96c59e5b80`只作为现有产物身份，没有本轮新构建或运行性能宣称。

起点153份历史报告、所有旧测试、生产工具、来源锁/清单和旧正式九文件SHA均保持；[材料核验](cl009/round8/verification/materials.json)和[21项发布字节校验](cl009/round8/checksums.json)分别记录。原始18份UTF-8检查/材料文件有[无损封装](cl009/round8/raw-artifacts.json)，八个运行器源码和原路径在[runners](cl009/round8/runners.json)。两项准备失败（内联PowerShell引用及许可副本EOF空行）单列，失败原进程字节未另存的限制明确；修正生成源后检查通过，未修改原许可或生产代码。

|状态|结论|
|---|---|
|已完成|真实语义原文待办、唯一外部人员/授权入口、原件本机路径/完整SHA核对、具体Google bulk许可出处及HEAD取得方案、可审阅生产语义发布器设计|
|部分完成|现代词频来源采用条件；Google许可出处有新证据，但无原件/完整协议/生产采用；原义项可审读但新语义仍空|
|外部阻塞|真实人员、授权新语料、提取/分词/时期和截断口径、语义双审/仲裁、heldout保管/封存、正式签署信任根|
|未实现/未执行|生产语义发布器/新格式reader/签署部署/用户迁移；新频率统计、正式语义内容、独立语言评测、物理断电；未调用生产激活/回滚|

PR #3保持Draft，不合main，不进入CL-010。下一步先由负责人取得并核固定Google原件/分母和协议，落实真实审读人员与授权，随后按设计审阅生产发布器实现；不能用待办或CI替代这些外部事实。

## 第二阶段增补：固定候选原件已实际取得

设计先行提交 **`fc539e9f3bd97c062aaf2af8351ba8ee58d43fd0`**，见[有界取得设计](../acceptance/cl009-round8/bounded-acquisition-design.md)。仅在全新 Git 外 `.local/round8-ngram-acquisition` 请求两个已由官方索引/HEAD确定的资源；无重试、无覆盖、无额外语料抓取。运行65.047秒，Python3.12.10/Windows11构建26300。原件不进入Git，完整URL、绝对路径、HTTP、时间和身份在[取得原记录](cl009/round8-acquisition/acquisition.json)，运行器与原始检查日志另归档。

|固定资源|实际完整字节 / SHA-256|
|---|---|
|`20200217/chi_sim/1-00000-of-00001.gz`|60,833,224 / `6ab29862a8c904c261bb9d5a02a342285c4dbabf79cee5c4f8818076fd44fb86`|
|同版`totalcounts-1`|7,479 / `f4a136b63419c0f34dfbfcf76f501ac80a745ad535f5cc9bda2666189a35e9f3`|

两项均HTTP200、长度与HEAD/GET一致、流式SHA完成。gzip读到EOF并通过标准库CRC/ISIZE，解压195,878,750字节，SHA `f5535ee7d0c0a5ba18bff3b4019d150da0e63265b8e2abdebb2d442db45b95f0`；严格UTF-8解码508,674行。实际每行首字段之后是数量不等的逗号三元组，共19,074,163组三个十进制字段，tab字段数2–457；**并非直接套用v2四列逐年格式**。totalcounts实读464条空白分隔记录、每条四个十进制字段。这是结构观察，尚未批准三元组语义、年份/分母、提取/中文分词或筛选协议；未计算产品token/DF/Zipf或合并词频。

本次解决“没有候选原件SHA/CRC/实际v3形状”缺口。来源登记与批准锁完全未改，Google仍candidate-not-approved，既有123,544 runtime词频仍unknown、1,468隔离记录不动。原件属于已有具体CC BY3编译数据候选，不代表原书全文许可、现代IME代表性、正式数据或独立gold已接受。发布器仍仅设计，生产词频与语义发布继续阻塞。

[追加检查](cl009/round8-acquisition/checks.json)保存原始输出和返回码：原件/初始21项Git清单/18原件/153历史报告与生产边界核验、资源许可审计、fmt、diff。取得脚本的终端输出仅在执行工具转录中，未另存原始stdout字节；`acquisition.json`是实际执行生成的完整记录，后续检查stdout/stderr原字节无损封装，不重造取得日志。新最终HEAD的Windows/Linux push CI由共享R8最终汇总绑定，第一阶段CI/旧回归不转绑。

最终已完成：候选原件完整取得/CRC及结构观察、来源与角色交接、语义待审实物、发布器设计。仍需负责人完成统计协议和生产采用审查、实际授权人员与语义双审/仲裁、独立heldout保管；token/DF/Zipf及正式Top-1/Top-5/MRR继续null。未执行任何生产激活/回滚、真实平台/物理断电验收或CL-010。

追加归档的两次准备失败亦保留：[初次环境失败](cl009/round8-acquisition-initial-check/checks.json)中workspace audit因子进程PATH缺cargo返回1，补显式既有cargo路径后通过；[文档检查失败](cl009/round8-acquisition-diff-failure/checks.json)中diff返回2指出新增末尾空行，规范文档末尾后通过。原始封装和修正前脚本/文档字节留存，未重取网络原件、未修改审计规则或放宽断言。
