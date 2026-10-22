# CL-009 第二轮：真实词条摘录、读音审计与进程中断恢复

2026-10-22。实现与测试提交为 `68b5201f7431fe5582a42ed71a23fdd76b6fd97f`，设计先提交于 `971ebec`、`979351e`、`72cd403` 和 `926fddb`。本轮交付的是可追溯真实词条的**有界研究快照**、18条合法读音审计及发布进程中断恢复；CL-009退出条件仍未满足，不能进入CL-010正式验收。

## 真实来源与实际取得范围

[Kaikki中文raw页面](https://kaikki.org/zhwiktionary/rawdata.html)登记中文Wiktionary dump 2026-10-01、提取日期2026-10-20，Wiktextract代码 `c1bb342f3f3afb42e2200dea4dfc8bdc9fb0a4f4`、wikitextprocessor `6f96ddf5f3466707f1869fe6618390fd77dfd109`。使用发布者提供的[raw批量入口](https://kaikki.org/zhwiktionary/raw-wiktextract-data.jsonl.gz)，没有使用未知镜像或混入postprocessed额外来源。

完整下载首次中断后仅启动一次Range/If-Range续传。服务器返回206和正确Content-Range，但连接在总计93,929,095字节提前EOF，广告总大小为234,450,401字节。下载器最初把EOF当结束，人工核对长度后立即保留为`.part`；**未进入完整gzip来源锁或构建**。该残片SHA-256为 `b3995e6cc4672a4f385caf2b8d32492b32dd267386e9407fd32cf9472fc70a45`，只是残片摘要，全件摘要仍为null。HTTP ETag为`"6ad75338-df96de1"`，Last-Modified为`Tue, 20 Oct 2026 11:40:40 GMT`。本轮没有继续重试或声称完成下载。

另按先提交的有界摘录设计，将已取得字节的固定前8MiB复制成独立、不再增长的文件；全件门禁保持不变。

|对象|固定大小/范围|SHA-256或验证范围|
|---|---|---|
|压缩前缀|8,388,608字节，0..8,388,607|`fdf3ae6918edb0e6af91cad20dfb51a2ecad8ce59ab026b4ab4e1e7f33581518`|
|摘录JSONL|46,788,375字节，66,593完整LF行|`ca4df3453261f9f12f1223deb11a26d979078ea6e1af870b44da2d3dcf4e2998`|
|截断尾部|139字节|不补齐、不导入|
|原gzip全件|未取得完整原件|完整SHA=null，全件CRC=false|

独立来源ID为`wiktionary:zh-20261001-prefix8388608`，格式`wiktionary-zh-prefix-jsonl`。`source_subset=true`、`research_only=true`、`full_source_download_complete=false`随来源锁和质量报告保留。文件前缀有源顺序偏差，不能代表完整中文Wiktionary覆盖率。完整来源锁见[data/sources/locks](../../data/sources/locks/wiktionary-zh-20261001-prefix8388608.json)，取得记录见[机器证据](cl009/round2-source-acquisition.json)。

## 许可、词条模型与质量

[Wikimedia使用条款第7节](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use)说明文本重用及CC BY-SA4条件，也提醒导入文本可能有额外署名或例外；这不是每一页绝无例外的证明。保留逐条`attribution_url`、原行摘要/行号、提取链、许可原文和修改说明；可得额外权利标记的记录隔离，媒体和引用实例不输出。**逐页额外署名/例外未全部人工复核**，限制随包保留，不能把研究范围批准解释成产品词库授权全部完成。

派生文本仍为CC-BY-SA-4.0，工具源码为MIT。[许可原文](../../data/licenses/CC-BY-SA-4.0.txt)保持官方20,138字节，SHA-256 `28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5`。包内NOTICE说明截取、筛选、规范化、未知频率与署名方式。

schema3保留中文释义语言`zh`、原词性、明确普通话/拼音、明确英文翻译及来源字段。v2运行时annotation只投影原件明确的英文translation word；中文释义留在事实层。仅取2..16个汉字的原词头，不推导简繁、不从单字拼出词语读音，不按词条出现次数填频率。

|实际统计|数量|含义|
|---|---:|---|
|接收事实记录|701|521个不同多字词头、532个不同拼音键|
|未知词频|701|全部frequency及frequency_observation为null，TSV为空|
|来源词性已保留|701|源POS，不推测语言学准确率|
|有明确英语翻译|216|其余485条annotation为空；无完整双语声明|
|运行时不支持的读音|94|能力范围隔离|
|读音歧义隔离|30|不按预期答案选分段|
|额外权利标记隔离|18|许可待审，未进入词条输出|
|源坏记录/冲突|0|上面142条不能算源坏记录|
|缺明确普通话拼音而跳过|111|不补读音|
|其他语言而跳过|65,265|不冒充普通话|
|不在多字汉字词头范围|608|范围筛选，不是源坏行|

`verify`重算可由事实层和拒绝报告推导的统计、语言投影、稳定ID、索引、scope及accuracy_claim=false。原始行数、目标属性数、源冲突/跳过量等仍需原件重建；自包含校验返回`source_only_statistics_rebuilt=false`。本轮干净重放重新读取固定摘录得到相同质量报告，不能据此宣称任意包自身证明了原始来源统计。

## 干净环境重放与真实发现

[完整机器证据](cl009/round2-clean-replay.json)记录配置、builder文件指纹、每次命令/输出、Python环境、构建耗时/内存和九个产物摘要。

1. 对实现提交`68b5201`执行`git archive --format=zip`；归档SHA为`f142d57793d172a4323981d27ecbb7c2040e8bebaac21d5da126ef16fdd556f5`，109个文件。导出不含`.git`或旧`.local`，103个指定文本文件经UTF-8、无BOM、LF检查；官方许可另按原字节哈希核对。
2. 建立全新`venv --without-pip`；Python3.12.10，无第三方distribution。每条命令使用`-I -X utf8`新进程，只显式加入归档源码目录；构建/摘录命令阻止urllib网络访问。
3. 从同一固定压缩前缀分别重建`prefix-a.jsonl`、`prefix-b.jsonl`，独立完整摘要与66,593行均相同，再从不同本地文件名两次build并verify。
4. **全部九个文件逐字节一致**。manifest SHA为`df137d60c7299d84cdaa262386556bafa4e56103fde2ae592738a31bbe292d6f`；runtime SHA为`b47287d0fd405c225409653cd8d08f34f9fcb31ef8bb8c9b9e54dbcf5fd7741c`。同环境重放Unihan18条审计亦字节一致。
5. 干净归档运行49项Python测试，全部通过；真实子进程使用同版本base executable、`-I -X utf8`和显式归档源码路径，持有的Popen句柄对应实际写者。

此前重放确实发现两处问题，修复前的结果没有登记为成功：本地basename进入词条证据，导致不同文件名的构建不一致，现由来源锁固定逻辑`source_member`并新增回归；Windows venv入口是launcher，`Popen.pid`可能不同于真正持锁进程，现测试直接启动base executable并断言owner PID一致，恢复实现的保守PID门禁保持不变。另用`-X utf8`明确覆盖`-I`忽略PYTHONUTF8环境变量后的编码行为。

本轮先交给B的冻结目录`.local/wiktionary-zh-prefix8388608-frozen`由`44cecd4`生成，保留不覆盖；最终归档包在`.local/clean-round2-68b5201/release-a`。二者runtime、records、index、quality、rejected、LICENSE、NOTICE七份数据资源摘要完全相同；manifest/source-lock因builder指纹和逻辑member契约更新而变化。B使用历史冻结锁的报告应保留该历史身份，不能混换元数据。B的资源/一致性机制实验不构成语言准确率。

## 进程中断恢复与检查

恢复采用操作系统单写者guard；崩溃自动释放guard但遗留owner锁需要显式recover。Windows仅用OpenProcess/WaitForSingleObject/GetProcessTimes只读查询并关闭句柄，不使用Windows的`os.kill(pid,0)`。活进程、PID复用或权限/状态不明均拒绝；先无副作用校验指针和所有孤儿安全，再隔离固定命名对象和更新显式回退指针。符号链接导致失败时current和owner锁保持原样。

独立审阅进一步发现旧`ca6656c`的链接门禁有实质缺陷：[旧junction复现](cl009/recovery-review-before.json)显示存储本体、祖先和嵌套孤儿junction均被错误接受；[旧硬链接复现](cl009/recovery-review-hardlink-before.json)显示`current.tmp`的硬链接可被`wb`截断，影响审查者自建的外部fixture。原先43项通过不能覆盖这些风险，不能沿用为修复验收。

在设计`926fddb`后新增统一未resolve祖先的`lstat`门禁：拒绝POSIX symlink、Windows全部重解析点、普通文件硬链接及`..`父级跳转；恢复逐层检查孤儿，第一次创建guard前及持有后均检查。`atomic_json`使用独占`xb`创建，已有临时文件不截断；可捕获失败仅清理本调用创建且文件身份未变的临时文件，保留正常重试行为。新增6个测试方法覆盖真实junction、内部目标、release、metadata硬链接、临时指针和父级跳转；[独立修复后9例复测](cl009/recovery-review-after.json)全通过，源码指纹对应`68b5201`；审阅者另实际精确运行父级跳转测试1项通过。[独立venv PID检查](cl009/recovery-review-pid.json)亦确认实际Popen/owner身份一致。

该链接门禁限定普通本地目录，保守拒绝云文件重解析点。它不提供对不遵守guard的其他进程并发替换路径的handle-relative安全沙箱；此边界仍未实现，不作额外安全保证。

[真实词典中断证据](cl009/round2-real-interruption.json)使用已有44,431条Unihan与本轮701条词条包，在独立本地存储中实际启动自己持有Popen句柄的子进程，并以`os._exit(73)`覆盖拿锁后、复制中、完整缓存rename后、临时指针写后、current替换后和rollback替换前/后七个点。每次遗留锁PID匹配实际子进程，旧或新current指向完整包；显式恢复后可再次activate/rollback，用户词典探针摘要始终不变。另一项测试只对自身Popen创建的子进程执行terminate，证实guard释放与owner锁恢复语义。**这不是物理断电、文件系统持久性或正式安装器验收。**

README要求的locked fetch、离线build、fmt、clippy（warnings拒绝）、60项Rust测试、doc tests、资源/依赖审计通过，[8条本地检查命令](cl009/round2-local-checks.json)保留各自实际输出。审计覆盖7条来源登记、4个资源和6个无外部依赖crate。最终Python测试在上述干净归档执行49项。代码未改A/B/shared或默认64条原型数据。

基准见[离线索引结果](cl009/round2-clean-replay.json)和[旧输入核心对照](cl009/round2-core-performance.json)。真实701条包的Python精确索引10,000次查询P50/P95/P99为100/200/300ns，载入1.4795ms，进程峰值33,222,656字节；它只测Python后台JSON索引，不能替代Rust候选或TSF延迟。64条旧原型核心79,000次事件P50/P95/P99为1,900/4,800/8,400ns，源码和二进制指纹随报告保留；83条诊断种子对照未用于调参，仍只是原型回归。桌面后台负载未隔离，均非产品性能承诺。

## 完成、部分完成与阻塞

|状态|本轮结论|
|---|---|
|完成|18条合法读音可重放逐项审计；schema3语言/词性/来源事实；有界真实摘录导入、隔离分类、逐条署名链；干净环境九文件双构建；真实进程中断/显式恢复证据|
|部分完成|可追溯真实多字词数据701条供机制实验；仅216条有明确英语翻译；可得权利标记自动隔离，逐页例外未全部人工复核；发布指针/缓存恢复限当前工具契约|
|未完成|完整Wiktionary原件、生产词库覆盖、完整双语、语言准确率/独立gold、扩展音节正式决策、正式安装器签名和断电验收、跨版本用户词典迁移|
|阻塞|现代统计词频与正式语料尚无通过具体资源许可和偏差审查的输入；MDBG脚本访问限制不绕过；Google Ngrams仅来源候选待核对bulk许可/单位/版本，Leipzig通常NC范围未获商业许可，不填频率|

CL-009保持开放。CL-010必须等待A/B/C各自退出条件和真实平台/独立语言质量证据；本轮不以摘录或机制实验替代这些条件。
