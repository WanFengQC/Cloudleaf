# CL-009 第四轮：完整授权词条、质量与恢复证据

2026-10-23；方向基线`e3dc4f92a56a1c005491fcaf0f7b798755dd754e`，分支`codex/cl009-data`。本轮新增自研完整CC-CEDICT ZIP导入器和实际125,012条固定源记录，接受123,544条，包括110,045条多字词。来源许可及工程复现通过不等于生产语言质量通过：现代词频、独立IME gold、分词/POS和结构化双语/双向翻译仍未交付，CL-009不关闭，CL-010不启动。

[设计](../cl009-round4-design.md)先提交于`65ae777`，镜像容器/授权范围补充先提交于`fb2a743`；其后实现`7afd507`、无损许可证据封装`2401b16`、特殊归档成员门禁`0dee2e27fa7749d952df24e44bfaefc9a2331c6d`。没有修改共享schema2/3、九列TSV v2、默认原型、核心音节表或A/B接口，没有覆盖旧产物/报告，没有合并或改写main。

|本轮修改文件|具体作用|
|---|---|
|`docs/cl009-round4-design.md`|来源、容器、稳定身份、隔离和质量门禁设计，后追加固定镜像范围|
|`tools/data_pipeline/cedict_source.py`、`pipeline.py`|独立ZIP导入、事实身份核对、格式分派及派生质量重新计算|
|`tests/test_cedict_source.py`|六项测试及特殊成员类型等独立subcase；不删旧测试|
|`data/sources/locks/cccedict-20260604-mirror7019e1c.json`、`evidence/cccedict-mirror-7019e1c/upstream-documents.json`|固定原件、许可和无损原始授权证据|
|`data/sources/source-review.jsonl`、`data/manifests/data-source-review-v1.json`|追加第八条独立批准来源并更新资源清单，旧七记录保留|
|`tools/data_pipeline/README.md`、本报告和`docs/reports/cl009/round4-*.json`|命令、完整成功/失败记录、质量分母、性能、复现和恢复边界|

## 取得方式和数据权利

[MDBG官方CC-CEDICT页](https://www.mdbg.net/chinese/dictionary?page=cedict)确实列bulk ZIP/gzip和CC BY-SA4；同站footer又禁止脚本访问。本轮没有找到明确bulk例外，因此没有自动调用该站的导出、查询或API。robots允许访问不是授权替代。这里区分开放数据的有条件再分发权与某站的取得限制，未把旧阻塞结论改成无条件放行。

另行选择公开许可再分发的固定GitHub镜像`poliwhirl555/cc-canto-cedict-jsons`，完整commit为`7019e1c3ec3de3a44b165b15177ce2bb39a026d3`。[固定README](https://github.com/poliwhirl555/cc-canto-cedict-jsons/blob/7019e1c3ec3de3a44b165b15177ce2bb39a026d3/README.md)提供带日期的原始ZIP用于程序访问；[固定LICENSE](https://github.com/poliwhirl555/cc-canto-cedict-jsons/blob/7019e1c3ec3de3a44b165b15177ce2bb39a026d3/LICENSE.md)明确CC-CEDICT原始/JSON数据为CC BY-SA4。只取单一原始CC-CEDICT ZIP，没有取镜像解析代码、JSON、CC-CANTO或粤语读音。Lester Lin是此镜像分发者，不替代MDBG及原CC-CEDICT贡献者署名。

[源码锁](../../data/sources/locks/cccedict-20260604-mirror7019e1c.json)固定URL、版本、压缩/明文SHA、完整头部、授权原文摘要、署名、修改和同许可条件；新来源审核登记与旧MDBG直接取得blocked记录并存。商业使用、修改和再分发按[CC BY-SA4原文](https://creativecommons.org/licenses/by-sa/4.0/legalcode.en)保留署名/许可、说明修改、派生数据同许可。项目MIT仅适用于工具代码，不把数据重标MIT。媒体、镜像代码及其他数据未纳入本包。

[授权原件封装](../../data/sources/evidence/cccedict-mirror-7019e1c/upstream-documents.json)完整保留README/LICENSE的UTF-8原文，重建Git blob分别等于`c6d46273f7f1fb3c7cf2179f3df313f6aa20752f`、`086685f145c6f3b1a1fe7c18056e6a62ab2d2da8`。不把该镜像称作最新官方版本或出版者签名。

|固定输入|字节数|SHA-256|
|---|---:|---|
|2026-06-04镜像ZIP|3,975,327|`fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`|
|唯一成员cedict_ts.u8|9,867,130|`9a3a8627c39701ecbd10246647f759f737ccd560cdbd29ef50bed8244e9ebd03`|
|完整CC BY-SA4原文|20,138|`28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5`|

[单次取得原始报告](cl009/round4-source-acquisition.json)保存HTTP200、请求/最终URL、Content-Length、ETag、完整源头部和全件ZIP CRC结果。头部明确CC-CEDICT、publisher=MDBG、v1/ts/UTF-8、entries=125012、date=2026-06-04T08:02:31Z和CC BY-SA4链接。固定镜像日期早于本轮官方页观测版本，未冒充当前最新release。

## 实现与质量分母

新`cedict-zip-v1`只接收锁定单成员，不落盘解包；压缩/展开/源行分别有界，全件CRC、明文SHA、UTF-8、头部属性及实际源条数均核对。归档路径、额外成员、目录、链接、加密、显式FIFO/socket/device类型和超限拒绝。DOS无Unix类型位的普通成员可读取。

事实层保留繁简、大小写敏感的原始数字声调读音、v1斜线分段原始英文义项和源行。来源ID仅由繁、简、原读音决定，释义修订不改变来源身份；完全重复合并证据，同来源身份的不同释义全部隔离。未知词频/词性不推断、不补零；不从单字组合词语读音，不改生产解析器表。超长注释仅省略运行时投影，完整义项仍在records侧包。旧版斜线分段不是已完成语义消歧或结构化英语翻译。

|统计|结果与解释|
|---|---|
|源条目 / 接受 / 隔离|125,012 / 123,544 / 1,468；原始125,042行还含头部与注释|
|接受单字 / 多字记录|13,499 / 110,045，保留来源读音与不同大小写身份|
|不同简体词头 / 规范拼音键|119,781 / 85,439；这是源内覆盖，不是外部语言覆盖率|
|繁简不同 / 含大写原读音|76,573 / 20,598，没有静默合并|
|多个旧式义项段 / 范围外文字记录|45,146 / 55；后者按导入器固定Han码点范围筛查，不是人工文字分类|
|缺频率 / 缺词性|123,544 / 123,544，全部null/unknown|
|缺原始英语段 / 超长运行时注释省略|0 / 241；不证明英译完整性或正确性|
|unsupported_cedict_reading|763条，含数字、混合字母或非本格式明确声调音节；不猜读音|
|unsupported_reading|704条，含当前音节表缺项/不支持音串；不记为源坏行|
|运行时预算隔离|1条，原行与原因保留；没有放宽现有预算|
|源坏行 / 重复 / 来源冲突|0 / 0 / 0；自编夹具另验证重复合并与全部冲突隔离|

build和verify分别从最终事实重算新增质量计数，重写quality/manifest摘要不能绕过派生计数关系；source-only统计仍明确`source_only_statistics_rebuilt=false`，只有本轮固定完整原件重建证明源条数/原始审计。质量报告的train/dev/heldout均null、accuracy_claim=false，没有从词典产生gold。旧Unihan18条合法但当前不支持读音的结论沿用[第三轮审计](cl009-round3.md)，本轮未扩表或宣称重新审读全部18条。

## 本轮重新核验的频率、评测与双语取舍

以下只表示2026-10-23第四轮实际重开相应官方/维护者页面的结论；[收尾读取记录](cl009/round4-source-review-closeout.json)保留成功及超时。THUOCL、wordfreq和SUBTLEX论文读取成功；Tatoeba收尾读取超时，其许可判断沿用第三轮而非本次重新确认。未重新检查其余第三轮候选，不能把旧表整体写成新核验。没有下载这些候选的数据包、训练集或heldout答案。

|来源及本轮核验|权利、语义与当前采用决定|
|---|---|
|THUOCL固定`a30ce79d895d01ab5132a5c74c29703ff7efb4cc`的[README](https://github.com/thunlp/THUOCL/blob/a30ce79d895d01ab5132a5c74c29703ff7efb4cc/README.md)、[MIT](https://github.com/thunlp/THUOCL/blob/a30ce79d895d01ab5132a5c74c29703ff7efb4cc/LICENSE)|MIT及README明确研究/商业开放，保留THUNLP版权、许可，科研按其引用要求。统计是文档出现数DF，来源语料含2014–2016博客、2008–2016新闻及搜狗，不是现代token频率。词表还含网站标签/热词/输入法词库，逐源来源边界未闭合。保留为可单独研究的DF候选，本轮未下载/导入或接入排序，不能填现有unknown。|
|SUBTLEX-CH2010[作者论文](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0010729)及wordfreq固定NOTICE|论文开放许可与频率可用于研究的描述并存；其他项目获得“在wordfreq内”分发邮件授权不自动等于本项目独立包授权。本轮没有取得覆盖Cloudleaf商业修改/再分发的具体文件条款；`blocked-license-scope`仍成立，不宣称永远无法获得。字幕年代和切分也需独立定义。|
|wordfreq固定`912caf64b657478d1dff1138efdc078947d54bb1`的[README](https://github.com/rspeer/wordfreq/blob/912caf64b657478d1dff1138efdc078947d54bb1/README.md)、[NOTICE](https://github.com/rspeer/wordfreq/blob/912caf64b657478d1dff1138efdc078947d54bb1/NOTICE.md)|Apache代码和CC BY-SA4数据分开，须署名Robyn Speer与各来源；README仍明确中文Jieba附表provenance未知，数据约截至2021。中文混合来源可分离性未核实，维持`blocked-provenance`，未安装/导出，也不调用默认minimum=0制造观测零。|
|Tatoeba[ToU](https://tatoeba.org/en/terms_of_use)第6节；收尾读取超时，许可判断沿用第三轮|此前审查为文本默认CC BY2.0 FR、逐句可选CC0，需作者/句ID/逐句许可链，音频另审。本次没有新增许可确认、固定dump、逐句作者/许可核验或人工IME读音/可接受输出。保留独立文本/句对条件候选，未批准为gold；平行句不等于多义词典。|
|已取得CC-CEDICT原始英释义|本轮正式采用的是完整固定词条/原读音/原始英释义范围，CC BY-SA4权利链如前。POS全部缺失、斜线分段不构成稳定语义义项ID；分词标注、跨词义对齐、双向词性/固定搭配/翻译质量仍未实现，不能把本包等同完整翻译数据。|

现代token频率尚无已选择且批准的固定包；独立评测也尚无获准、经人工双审/仲裁的IME gold。下一步先冻结具体候选版本、署名和分发链，再单独设计DF/token/Zipf观测类型及义项/句对侧包；不从词典收录数推频率，不在heldout上补词或调参。Google Books、Leipzig、UD与Wikidata等其余状态仅沿用第三轮2026-10-23候选审查，本轮未更新其结论。

## 独立复现和可复查输入

最终代码[干净双构建报告](cl009/round4-final-clean-replay.json)从`0dee2e27fa7749d952df24e44bfaefc9a2331c6d`导出git archive，在全新checkout和无pip venv中以Python3.12.10、`-I -X utf8`运行。模块路径检查指向归档，第三方distribution为空，构建禁止urllib网络；两个新目录各自build/verify、全部九资源逐字节一致，完整55项Python测试通过。首次`2401b16`批次的[九文件/55测试证据](cl009/round4-clean-replay.json)和[本地检查](cl009/round4-local-checks.json)另存，未被最终批次替换。

两个批次runtime、facts、index、quality、rejected、source-lock、NOTICE与LICENSE内容一致；最终manifest因builder源码摘要更新而不同，不写成跨代码版本九文件一致。冻结给B的是`.local/round4-clean-2401b16/release-a`，B另复制到其独立目录并逐文件核验，保留该明确输入身份，不强迫实验中途换包。

|关键资源|SHA-256|
|---|---|
|共同runtime.tsv|`03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`|
|共同source-lock.json|`a7cbdd759ae2487ce238aea8a2d43eeef3248a1a124004def9ad35764e58bd57`|
|B冻结2401 manifest|`85b6f837e0644716605f9458a59ebbc3044ed2b03a1c6d26f497ac2fa5993de2`|
|最终0dee manifest|`25b72932432b927316febdb691ff9edfad8380a08cc6de5ef38751853c037c9d`|

## 真实升级、中断与回退

[七点实际包恢复报告](cl009/round4-final-real-interruption.json)全部完成。父写者及故障子进程均从`0dee2e27fa7749d952df24e44bfaefc9a2331c6d`干净归档导入，代码ZIP摘要`fbb2aec73efa01d3672c5e8144f684ec83b7bbf4002d446f8aed3b580ee44825`。旧44,431字符包manifest为`6012638a09d1a2e8d2d7347ad10c78a357c3b953bba68cff610795fb609770b0`，新123,544词条包manifest为上表最终0dee值。七个独立新store最多三路并行；只对自身创建并持有句柄的子进程注入`os._exit(73)`，不终止外部进程。

|中断点|中断后有效current|显式恢复隔离内容|结果|
|---|---|---|---|
|持有发布锁|旧包|owner锁|通过|
|复制半途|旧包|owner锁及半成品发布目录|通过|
|不可变缓存重命名后|旧包|owner锁及发布目录|通过|
|临时指针写出、原子替换前|旧包|owner锁及临时指针|通过|
|新指针原子替换后|新包|owner锁|通过|
|回退指针替换前|新包|owner锁及临时指针|通过|
|回退指针替换后|旧包|owner锁|通过|

全部子进程exit73，owner PID与锁中身份一致且恢复时已退出；current始终指向完整、可校验的包。每例显式recover后再次activate新包、rollback旧包并分别verify，用户词典探针SHA始终`97de41d09a06013362436e2ace797b7499bf0c10e197c5a41e8c5ff4528aa2b1`。报告保存完整命令、注入代码、PID、指针前后状态和空stdout/stderr；每例总耗时118.162–183.451秒包含初始化、独立校验、恢复和再激活，不等于故障子进程等待时间，子进程180秒上限未触发。

损坏资源/索引、回退目标不合法、活PID或身份未知、链接/重解析路径等拒绝行为仍由55项管线测试覆盖，本轮新增完整大包的七点进程恢复证据；没有把它写成物理断电、正式安装器或用户词典迁移验收。

## 本地检查、性能与失败保留

[最终九条README检查](cl009/round4-final-local-checks.json)全部exit0：release/debug locked离线构建、locked fetch、fmt、Clippy全部target且warnings拒绝、60项Rust测试、doc tests、资源审计、55项Python测试。审计仍6crate、0外部Rust依赖、64条原型/41事件/83种子，来源登记由7增为8。没有删旧测试；新增6测试覆盖完整容器/改本地名确定性、ID/大小写/释义修订、重复/冲突、读音/预算/坏行分类、归档边界和重新计算的质量门禁，特殊文件类型使用同测试的独立subcase。

最终完整数据的构建时间、后台索引和峰值以干净双构建原始报告为准；它们包含后台完整校验及Python事实/索引结构，不是Rust候选或实时输入内存。当前单机桌面后台负载未隔离，不能据此宣称算法最优或性能提升。

[旧核心基准](cl009/round4-final-core-performance.json)仍仅64条原型：79,000事件P50/P95/P99为3,400/9,200/14,900ns，最大2,222,500ns，进程峰值6,017,024字节；release二进制SHA与第三轮相同。运行时同期有数据验证负载，原型计时变化不能解释为本轮核心回退/提升，未连接新词库或TSF。

本轮保留两项实际纠正。最初直接保存上游README/LICENSE时，原文件自带尾随空白导致`git diff --check`非零，未删除原空白或降低检查，而在后续提交中改为可无损恢复原文的JSON封装并重算Git blob。[收尾只读核证](cl009/round4-closeout-audit.json)另重放`git diff --check fb2a743 7afd507`并保存非零状态和完整错误；它是对固定旧提交的重放日志，不冒充首次终端捕获。初始缓存直接沿用含冒号的Git文件名，在NTFS上形成备用数据流；[另存证据](cl009/round4-safe-cache-copy.json)记录离线复制为普通文件名、3975327字节与相同SHA，旧流/取得记录不删除，最终重建使用普通缓存。来源元信息收尾复核另保留Tatoeba一次读取超时，没有据此放行新来源。这些失败均未被伪装为来源或语言质量通过证据。

收尾核证还逐个重算两个最终包全部九文件、源码归档和普通源缓存的摘要，确认B冻结旧包未改、跨实现版本只有manifest因工具摘要变化而不同；七个实际store最终指针及探针也与报告相符。基线已有45份历史报告文件无差异，Round1/2/3证据保留。

正式签名安装器、物理断电持久性、跨版本用户词典迁移、生产资源预算和真实语言覆盖/准确率仍待验收。本轮精确推送提交的Windows/Linux远程CI需推送后单独核实；历史或本地通过不替代新提交CI。

## 退出条件、阻塞与后续依赖

|状态|已完成范围或尚需证据|下一步依赖|
|---|---|---|
|完成|本轮固定完整CC-CEDICT来源许可链、确定性导入、质量/隔离、两个实现检查点的独立双构建和最终完整包七点进程恢复|提交后的精确Windows/Linux CI另验，不替代正式语言验收|
|部分完成|多字词、原始声调读音与英语原始义项可追溯；241条注释省略、1,468条隔离全部保留|分层人工读音/英释义审读、当前音节缺口与B共同评审；不自行改共享接口|
|阻塞|没有批准的现代token频率、N-gram统计或独立人工IME gold；THUOCL DF、SUBTLEX和wordfreq各有上述明确语义/权利/来源缺口|固定可获授权的统计包、计量口径、来源家族和人工双审/仲裁；heldout不用于调参|
|未完成|正式POS、稳定语义义项、双向词性/搭配/翻译模型及质量、签名安装和物理断电持久性、用户词典迁移、产品资源预算|分别设计和冻结格式/质量目标后实现；本包不能直接当这些能力已交付|
|未开始|默认输入引擎接入新完整包及CL-010全链路集成|A/B/C分别达到退出条件后再评估；本轮不启动|
