from pathlib import Path
import json,re,subprocess,hashlib
root=Path(__file__).resolve().parents[1];ev=root/'docs/reports/input-round17-evidence'
read=lambda p:json.loads(Path(p).read_text(encoding='utf-8'))
checks=read(root/'.local/round17-verified-results.json');assert len(checks)==13 and all(c['returncode']==0 for c in checks)
head='cfedc80f14e4ea7773ffed335ffe5fccfa5c2fcc';assert all(c['head']==head for c in checks)
quality=read(root/'.local/round17-quality/summary.json');perf=read(root/'.local/round17-final-paired-traces/summary.json')
host=read(root/'.local/round17-host-benchmark/summary.json');diag=read(Path('C:/Users/EDY/.codex/tmp/r17-data/final-audit-v2/r16-all32-review.json'))
rust=(root/'.local/round17-runs/verified-04/stdout.txt').read_text(encoding='utf-8')
counts=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',rust)
totals=[sum(int(c[i]) for c in counts)for i in range(3)];assert totals==[228,0,2]
python=(root/'.local/round17-runs/verified-08/stderr.txt').read_text(encoding='utf-8');assert 'Ran 71 tests' in python
summary={'implementation_head':head,'status':'partial','checks':checks,'rust':dict(zip(['passed','failed','ignored'],totals)),'python':{'passed':71},'quality':quality,'performance_ns':perf['percentiles_ns'],'host_variants':host['variants'],'source_data_sha256':{'baseline_runtime':'03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431','combined_runtime':'c9ab8b56b9d960091a1c39c37b7c1df7882ff50de2a64767174af8f4244c219f','news_manifest':'b9fc2992b6a097345483055ad56728fe75054bb65307d6ae0aca1eef4cbcb165','news_dev_targets':'0b3d3cfa050668ba31ed837e3e2f6720c945ce8d017a7bf850f1c8837b2f3ef0'},'formal_language_metrics':{'top1':None,'top5':None,'mrr':None},'default_engine_changed':False,'new_ngram_model':False,'TSF':False,'CL010':'closed'}
(ev/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
lines=['# Round17：覆盖、搜索延迟物化与真实隔离键盘 Host','',
'2026-11-06。结论 **partial，不放行正式产品，CL-010继续关闭**。搜索结果保持正确、繁体正文可达性提高、原生隔离窗口可实际键入和提交；新新闻开发对照没有证明字符模型泛化改善，字符P95仍超过5ms。没有新增N-gram或默认启用研究模型。','',
f'实现提交 **`{head}`**，分支`codex/input-round14`，基线`521472db18ecd7926f6e5b58b0afd7b5da674a07`。最终报告提交及精确push CI、下载产物/二进制身份放在Git外`.local/round17-delivery-receipt.json`，不能用实现提交的CI代替报告最终HEAD的CI。没有修改main、五个PR状态/分支、翻译成果，没有PR6、TSF注册/激活/安装/卸载或8h/24h。','',
'## 实际实现与设计先行','',
'设计先于代码：f9c50c1定义范围；7e0b76c实现搜索；c677260保存补层接口red；3889c3d加入固定来源工具与实际Rust格诊断；ca79ed8保存隔离入口缺失的compile-red；c80e9ec实现窗口；0d3df9a保存真实子控件失活断言失败；0ece0a2记录观察到的焦点/IME问题；1f4758b、d37681f及cfedc80修复窗口隔离、追加保全与容量失败。完整设计见[设计约定](../input-round17-design.md)。','',
'| 文件 | 实际变化 |','|---|---|',
'| `crates/cloudleaf-search/src/search.rs` | Path正文/ID/edge按最终容量分配；字符Exact先计算精确界、比较已有正文代表，再物化分支。评分查询仍计入Scoring；预算检查位置、完整平局和统计语义保留。 |',
'| `scripts/prepare_input_supplement.py`、`tests/test_input_supplement.py` | 固定原件/记录逐行绑定，补缺失繁体单字，稳定ID/来源/许可证与unknown；原runtime逐字节前缀保留，独立研究层/双构建，模型仅改词典绑定、计数不变。 |',
'| `crates/cloudleaf-input/examples/coverage_probe.rs` | 载入真实Rust词典/模型，遍历实际词条格的完整正文路径并输出搜索候选/Features/预算；不是复制生产候选算法的测试替身。 |',
'| `adapters/windows/isolated_input.rs`、`input_review.rs`、`session.rs`、`mod.rs` | 显式普通Win32键盘入口、复用预载Host和两个实际引擎、校验revision/ID、owned文档写后确认、Unknown隔离、保全上限、显式新会话；独立无HWND benchmark。 |',
'| `adapters/windows/candidate.rs` | 同一绘制/命中/分页类增加无TSF构造和本地选择回调，TSF构造仍走原manager/context；已有Rc消息保活机制保留。 |',
'| `apps/windows-ime/Cargo.toml` | 固定windows0.62.2增加一个`Win32_UI_Input_Ime`功能，解除仅本测试父子窗口的继承IME；无新增包，Cargo.lock不变。 |',
'| README、CI、`.gitattributes`、本报告/证据 | 显式工具用法、Windows研发工具产物包含coverage_probe，原始证据按原字节保存。 |','',
'## 全部分母、覆盖与真实失败原因','',
'原32条全部保留。原19条均因基线投影缺繁体词头/读音而无法完整分词，属于B（词典覆盖），并有简繁及自动拼音抽取歧义；并非19个解析器实现错误。原1468隔离记录、123544条和frequency unknown均未改。补层从相同固定授权原件的已批准记录投影3810条/3643字，合并127354条；不是按19句手工补词。实际Rust格32/32可达、最长拼音54字节、没有64字节超限或格预算耗尽。22/32有多读音，新增可达19中14有歧义，例如“的→di、數→shuo、傳授→zhuanshou、說話→shuihua”，可达不等于自然读音正确。逐条缺字、原记录及原文见[完整诊断](input-round17-evidence/corpus/r16-all32-diagnosis.jsonl)，概览如下。','',
'| case | 原因/缺字 | 自动拼音字节 | 补后完整正文 |','|---|---|---:|---|']
for c in diag['cases']:
    if c.get('baseline_complete'):continue
    # 字段取自真实诊断，不凭报告补造缺字。
    missing='、'.join(c['missing_characters'])
    note='；有读音歧义' if c['ambiguous_readings'] else ''
    lines.append('|'+c['case']+'|B：缺'+missing+note+'|'+str(c['input_bytes'])+'|可达|')
lines += ['',
'原13条可映射样本中，目标完整路径存在、无未知模型字符、无最终预算失败。已暴露四条字符退化的主要机制是E（评分更低），并暴露H/I：训练分布与二元上下文限制；未发现这13例的C/D/F搜索丢路径或去重错误。逐条BOS/EOS、实际转移次数、分母、UNK、候选与定点评分见[概率诊断](input-round17-evidence/quality-diagnosis/all-ready-probability-diagnostics.json.gz)。其中离线浮点概率只是解释，真实Rust完整输出保留在runs，不将浮点重算冒称定点评分实现。','',
'- “被烧死/思”：烧→死、烧→思均未观测，死→EOS 97、思→EOS151，句尾分布可使错误“思”胜出。',
'- “不许/不需”：不→许7、不→需87；许/需→例均0，语料偏好无法表达“规定不许例外”的语义。',
'- “那里/哪里”：治→那/哪均0；那→里110、哪→里219，一字符历史不足以表示指代/疑问。',
'- “胜利/生里”：争→胜/生均0；胜→利16、生→里3，但利→了1、里→了30及不同上下文分母共同影响总概率。','',
'没有针对这四句改字、改ID或调权。评分数学定义沿用R16的BOS/字符/EOS定点对数概率：add-one与Witten–Bell均未修改；Lexical仍结构优先、词汇观测只作原有排序。补层只重绑词典与重新编译缓存，训练来源、计数和unknown不变。故本轮不声称新语言模型质量成果。','',
'## 冻结的新来源研发对照','',
'使用固定中文维基新闻32篇的受控文字片段，文章原发布日期跨2020-10-01至2026-01-05，修订/作者历史/原件/采样清单均保存；来源家族独立于Tatoeba。先冻结96槽及文章家族48dev/48heldout，再生成拼音、查看候选。108固定文件1,728,248字节SHA核验，5个稳定任务产物双构建一致。所有人工输入意图、允许答案、审核签名继续null，不伪造第二个人。','',
'[版权方针](https://zh.wikinews.org/zh-hans/Wikinews:%E7%89%88%E6%9D%83%E4%BF%A1%E6%81%AF)当前声明CC BY4.0，2025-01-30前内容曾用CC BY2.5；本轮保留当前及旧转载说明的差异、固定修订、作者历史、署名和修改说明，限文字片段research_only。许可条件见[CC BY4.0](https://creativecommons.org/licenses/by/4.0/)。图像/音视频/引文/联络形状段落及例外flag未导入。来源许可审核不等于人工语义质量或生产批准。完整来源记录见[source-review](input-round17-evidence/corpus/audit/source-review.json)。','',
'dev48槽：40 ready、1拼音超64字节、1缺“閲”、3没有合适片段、3保守来源flag；heldout48槽：40ready、5超长、3没有合适片段，**候选未运行**。页面53152“侵權”为新闻主题词，关键词规则是保守误报，冻结后仍不替换/补位；不是认定该来源已侵权。以明确标准（正文相等或≥8字符3-shingle Jaccard≥0.8）检查Tatoeba训练和文章家族，无匹配，但这不是证明所有语义近重复都已消除。新闻领域偏书面/繁体，环境类别取得0条，未覆盖完整聊天、混合输入和所有现代场景。40ready中26有读音歧义。','',
'40条同一预先冻结拼音/来源正文、相同K10和64/200000/50000预算，在实际Rust搜索上逐例保留全部Top10。全部48dev槽仍在缺件报告中，不缩总分母。来源正文匹配观察如下（**非IME准确率**）：','',
'额外语料计数诊断：Tatoeba训练86737片段/716681字符/5082字符类型；新闻40条仅2条含训练未见字符（昌、軾），但496个来源正文BOS/字符/EOS转移中187个未观测。不能将0首选都归咎UNK。非汉字符截断形成片段边界，BOS/EOS反映片段而非完整会话；短二元上下文、领域与简繁选择仍未解决。详见[news-domain-counts](input-round17-evidence/quality-diagnosis/news-domain-counts.json)。','',
'| 语料/词典 | 模式 | 已比较 | 正文格可达 | 首选同正文 | Top5同正文 | Top10同正文 | 最终预算失败 |','|---|---|---:|---:|---:|---:|---:|---:|']
for r in quality:
    lines.append(f"|{r['corpus']}/{r['layer']}|{r['kind']}|{r['tasks']}|{r['target_reachable']}|{r['source_first']}|{r['source_top5']}|{r['source_top10']}|{r['budget_failures']}|")
lines += ['',
'全部旧32和新闻dev40各运行三种模式、两个词典快照，共432个真实完整查询。新新闻的40条正文可达由9到40，但字符首选仍0；没有因覆盖上升就认为候选准确。旧32中补层Witten–Bell首选5降4，增加同音/简繁竞争造成新增退化仍保留。详细逐例排名、unknown及统计见[quality summary](input-round17-evidence/quality/summary.json.gz)。正式Top1/Top5/MRR继续null，无独立人工gold，不选新默认模型。','',
'## 正确性、性能与资源','',
'实际搜索源码SHA`51916dc49959e5b042c7fa60a1487983a8328c17e0cf57cf1212fd515a98ac5d`与3889c3d严格差分时一致，直到最终实现未再改动。22,320个完备Reference查询、44,640次Exact/Beam候选全对象比较（正文、顺序、IDs、edge、Features、评分）零失败、零不完备。PositionDag按契约拒绝字符历史评分，不能计为第四个等价算法。机械完备Beam无剪枝不证明一般Beam无损。25条实际查询与R16完整SearchResult/Statistics一致；150组已有阶段观测检查通过。差分原文72.2MB经CRC及解压字节SHA验证压缩，未丢失败。见[differential verification](input-round17-evidence/differential/verification/summary.json)。','',
'临时分配探针针对相同25查询：add-one realloc从131619降338，Witten从123826降333；是累计分配/重分配调用，不是峰值存活字节。路径复制、正文/IDs重复扩容和被淘汰分支的提前物化得到实质减少。BTreeMap/堆、完整正文代表和最终结果构造仍有成本，未引入Arena/增量/后台线程。探索性pure-query与最终InputEngine混合事件各自报告，不混为同一P95。','',
'最终同条件对照：R16精确源码另建release，原词典/模型不变；六种组合轮换顺序，每种5个新进程、全部171事件/次（855次），无丢弃warmup或长尾。含InputEngine.handle/帧clone，不含stdout/磁盘/平台；Windows普通桌面环境，未清OS缓存，保留整进程50ms资源采样，不能由它推算单算法内存。表中毫秒：','',
'| 模式 | n | P50 | P95 | P99 | max |','|---|---:|---:|---:|---:|---:|']
for mode,p in perf['percentiles_ns'].items():lines.append('|'+mode+'|'+str(p['n'])+'|'+ '|'.join(f"{p[k]/1e6:.4f}" for k in ['p50','p95','p99','max'])+'|')
lines += ['',
'5ms目标未过；add-one max由21.831升32.2843ms，不能隐瞒。六组全部FRAME/CANDIDATE/COMMIT/SEARCH/PATH在相同模型的新旧实现逐字比较一致。计时属于研发观察而非生产延迟保证。','',
'实际Windows Host另测K5（与上述K10不可直接混比），双实际InputEngine、语义比较、帧与内存事务；每模式3新进程×171=513事件，不创建HWND，UI阶段明确not-run。窗口UI仅在下面真实交互中单独测量，不能把两种时钟相加冒称真实应用端到端。','',
'| Host模式 | n | core P95 | projection P95 | total P50 | total P95 | total P99 | total max |','|---|---:|---:|---:|---:|---:|---:|---:|']
for mode,r in host['variants'].items():
    m=r['metrics_ns'];lines.append(f"|{mode}|{r['n']}|{m['core']['p95']/1e6:.4f}|{m['projection']['p95']/1e6:.4f}|"+'|'.join(f"{m['total'][p]/1e6:.4f}"for p in ['p50','p95','p99','max'])+'|')
lines += ['',
'补层实际Host准入通过，fixed_bytes128155305、epoch_cost(1966080,660352)，仍在原128MiB门槛内，未提高预算。combined模型estimated_bytes7626333（原7504413），快照新增竞争使Host明显更慢；因此既不默认启用补层，也不声称覆盖带来流畅度改进。整进程working/private与峰值采样保留在[host benchmark](input-round17-evidence/host-benchmark/summary.json.gz)，不是COM引用、句柄长测或物理断电证据。','',
'## 实际原生窗口与可靠性修复','',
'使用computer-use技能的`@oai/sky`物理press_key，在程序自己创建的普通HWND完成：n/i/h/a/o→候选你好、退格再修订、Space只提交一次、数字2选择、鼠标真实候选点击、Escape不写文档、qz+Enter原文回退、子EDIT持焦点后切到第二个自有窗口、原文n保全/候选关闭、返回后继续输入。写后Unknown实际文档增加一次“你”，重复Space未变，明确核对后新会话也未重复。最终owned文档为“你好您你你qz”；两个测试窗口均已正常关闭。原始UIA/动作/窗口与二进制身份见[实际交互记录](input-round17-evidence/round17-ui-final/automation.json)。','',
'首次观察发现继承系统IME截走物理n；增加仅本窗口IMM解绑后，实际按键进入真实核心。`type_text`在此宿主未形成WM_CHAR（不支持粘贴），因此后续全部按真实逐键press_key，未冒称批量粘贴是连续键入。第一版父窗口只看WM_KILLFOCUS漏掉EDIT外部失活，实际WndProc失败断言已修复为顶层WM_ACTIVATE。重复失焦覆盖保全历史的两个断言也先失败后通过；现在16KiB满时隔离、当前组合不清除，Unknown先隔离再追加，新会话明确确认后清除。','',
'窗口还输出core/projection/比较/事务/帧/candidate_window_ns；这些零散真实交互样本包含OS绘制调度，只说明路径实际运行，不作为UI P95验收。此入口仅5候选/页5；PageDown验证了单页末尾不越界，**未实际验证多页窗口切换**，原分页组件回归保留。空白区保护、旧revision/epoch、NotApplied显式RetryCommit、Unknown不重放、鼠标回调同步销毁在真实控制器/组件测试中通过，但不将组件测试称为注册TSF应用验收。UTF-16代理对输入、粘贴、系统级自动连发/快速连续压力、多应用全链路均未验收。','',
'## 完整回归、身份与失败保全','',
'最终实现cfedc80上13项原入口逐条返回0：diff/fmt、Clippy -Dwarnings、workspace all-target/doc/release locked/offline、workspace/许可证、Python、RustSec依赖审计、原基准/消融、input/search release examples。Rust228 passed/0failed/2原ignored；Python71通过。固定词典独立来源verifier15.1s返回0，原manifest/runtime/source lock/许可保留。具体命令、返回码和原文见[回归清单](input-round17-evidence/round17-verified-results.json)。1973个旧历史/数据/测试/许可路径未改，旧测试名称和搜索证据源码映射另存[preservation](input-round17-evidence/preservation.json)。','',
'保留真正red：补层接口缺失ImportError、隔离入口缺失编译错误、子焦点断言失败、继承IME断言失败、重复保全/容量断言失败；也保留工程工具错误：临时Cargo锁缺失、缺少Scoring的qz汇总错误、旧/新诊断工具版本误作双构建、NotApplied测试误用Space（改为原核心的RetryCommit）、初次patch换行/新文件头失败、错误包名和空过滤0tests、CI短SHA422。0tests不计作测试通过，没有修改生产核心来迎合错误测试。各错误后的有效命令/真实断言数量和完整日志分别存档。','',
'数据与源码身份分开：','',
'| 资源 | SHA256 |','|---|---|']
for label,h in summary['source_data_sha256'].items():lines.append(f'|{label}|`{h}`|')
lines += ['',
'原CC-CEDICT ZIP SHA`fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`；层SHA`edcb27080fac434637c83985eebd4c4f62d12e09943e4278a69fdc000b01e430`。模型旧/新SHA、lineage及双构建见[supplement identities](input-round17-evidence/supplement/identities.json)，本地产物见[binary identities](input-round17-evidence/binary-identities.json)。大词典/模型仍Git外，未打包发布或伪造词频；报告原件、压缩CRC与工具SHA见[index](input-round17-evidence/index.json)。','',
'实现push CI：[42180998051](https://github.com/WanFengQC/Cloudleaf/actions/runs/42180998051)已通过Windows/Linux各22步骤；三个产物与日志已下载，CRC及GitHub SHA digest逐一相同。完成快照和原始日志见CI目录。最终报告提交另行普通push并核对精确HEAD，完整结果存Git外回执，不能将实现CI自动转绑最终报告提交。','',
'## 状态、未实现与下一步','',
'| 状态 | 内容 |','|---|---|',
'| done | 全部19覆盖缺口及13映射失败候选诊断；独立研究补层；冻结新来源/分母和真实格比较；路径物化优化及严格差分；实际隔离键盘/事务/焦点/Unknown验证；本地13项及专项回归。 |',
'| partial | 工程泛化对照更广但仍偏新闻/繁体、自动拼音歧义；性能有收益但P95>5ms且add-one max变差；隔离交互未覆盖多页和持续快速输入。 |',
'| blocked | 独立真实人工gold/双审与授权、现代词频、正式语义质量、获批可恢复Windows环境和真实TSF应用验收，正式语言指标null。 |',
'| not implemented | 纠错、模糊音、简拼、双拼、用户词典/学习、跨事件抢占取消/异步调度、增量计算；正式生产模型/补层集成、安装发布与其他平台原生适配。 |',
'| not run | heldout候选、真实注册TSF/记事本/浏览器/办公软件系统输入验收、8h/24h、物理断电。 |','',
'仍需关闭的P1/退出门禁包括：来源正文读音/意图尚未人工确认、新闻字符首选0/40、已知四条字符退化及补层WB新增退化、閲覆盖缺失、128MiB内补层更高的双引擎延迟、5ms门槛与长尾。未发现本轮修复后仍可复现的隔离Host丢字/重复写入反例，但这不证明生产TSF无P0；原平台退出门禁仍不放行。','',
'下一轮优先将产品能力推进到显式可关闭的拼音纠错/简拼/双拼与可管理的用户词库，同时补真实人工开发输入意图、通用来源词头/读音覆盖和简繁目标策略；先量化正确输入误改与候选损失，保留默认全拼。不要继续仅在四个旧句上增加N-gram。真实系统体验仍必须先获隔离环境与注册测试授权。**main、五个Draft PR、默认输入引擎保持不变，CL-010不启动。**','']
(root/'docs/reports/input-round17-status.md').write_text('\n'.join(lines),encoding='utf-8',newline='\n')
print(json.dumps({'rust':totals,'python':71,'queries':sum(r['tasks']for r in quality),'report':str(root/'docs/reports/input-round17-status.md')},ensure_ascii=False))
