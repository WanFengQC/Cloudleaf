# Round 11：翻译子系统实现约定

2026-10-30。先核真实workspace、翻译v2、路线图、lexicon v2及C Round10实现后冻结本约定。新工作区/分支`codex/translation-round11`从共享`83a2666fa748b4ae9cba41a5880ba5353e71c9a3`建立；现四PR与main不改、不合C代码、不进入CL-010。此处建立独立T1/T2/T3功能线，不改变原平台/语言/数据退出条件。

## 实际复用与缺口

共享workspace六个crate目前没有翻译实现或异步运行时，输入core保持零外部业务依赖。`translation-api.md`仅概念接口。C的独立semantic bundle提供严格文件/审核/外部policy一致性验证，尚无正式POS/sense/有向翻译内容；原CC-CEDICT词头/读音/原始英语gloss在有限许可范围内，不能升级成已人工审定的等价翻译，反查也不产生逆向equivalent。无授权离线模型、真人gold或真实TSF验收。

## 模块与文件范围

新增`crates/cloudleaf-translation`，由`types/provider/scheduler/interaction/dictionary/offline/online/secret`分工；Rust核心输入crate和共享types不改，不依赖translation。注册/载入/HTTP/model在后台或显式设置路径，按键路径只发送有界命令/读取已有回执，不等待网络/磁盘/模型。另加测试和操作示例、依赖/许可政策`config/audits/translation.json`、本设计和最终报告。Cargo workspace/lock只在新分支扩充；原资源/历史测试不删。

## 稳定的源码契约

提供`TranslationRequest/Result/Direction/Provider/Error/TaskId/Cancellation/ProviderCapabilities/ProviderConfig`。Language为Auto/Chinese/English，目标必须明确；Kind为Word/Phrase/Sentence。请求关联平台session/epoch/input revision，task ID由service单调分配，结果显式schema/提供者版本及数据状态。Auto不猜混合/无语言字符，词典仅能在明确方向索引命中时消歧；在线可委托模型识别但不声称检测质量。

错误为固定分类，不携原文/译文、URL、HTTP响应体或凭据；Debug仅输出任务/版本/字节数与类别。结果正文只通过明确内容API取得。秘密不序列化，配置只含credential reference。SecretStore用于后台resolve，提供临时会话存储与明确的无设施错误；Windows Credential Manager/macOS Keychain/Linux Secret Service由平台层未来适配，不把内存存储宣称系统安全存储或实现密码学保证。

## 有界后台任务与缓存

固定1..4后台worker，bounded sync_channel try_send，UI submit/poll不做IO或等待worker。默认2worker/8queued/64retained tasks，表满或队满返回明确Busy，绝不创建无限补充线程。每任务独立回执，Queued/Running/Succeeded/Failed/Cancelled/Expired终态不回退；submit时开始总deadline（含排队），取消token为跨线程信号。worker只执行已注册提供者，不能偷偷fallback或选择网络。

提供者同步trait只允许后台调用，长计算必须合作check cancel/deadline；在线在worker内使用异步HTTP future，取消/总时限丢弃future。非合作第三方模型不能被安全强杀，任务结果失效后仍占worker直到退出，保留有界预算；不宣称任意插件硬中断或DNS瞬时回收。关闭请求取消所有任务，阻塞join只在显式后台shutdown，Drop不在输入线程等待。

缓存仅内存、默认关闭在线/整句缓存，entry与文本字节双界，有明确clear/evict。key包含准确输入/方向/kind/provider/version/config及词典版本；不做会改变含义的规范化，不持久化输入。cache命中绑定新task/context，不能复用旧ID。只缓存成功且允许的结果；失败/取消/超时不入缓存。上下文变化、取消或新请求后旧结果不能覆盖当前交互。

## 本地词典与模型

本地词典预载版本化JSON索引，入口核一次schema/大小/唯一ID/方向/kind/来源许可/内容状态；runtime只查内存。候选raw gloss仅DictionaryGloss，英文反查仅ReverseLookup，确认过的有向词/短语映射才Translation。Production模式拒绝pending/research_only及缺失准入绑定的记录，研究模式必须显式选择且结果始终标研究；不能通过caller布尔值制造审核通过。schema2/semantic bundle文件不被Rust随意直接解析为已审内容，必须显式离线导出/准入契约。真实内容授权与审批外部真实性仍需负责人核验。

离线模型使用同Provider trait及模型id/version/path/availability元数据，缺模型返回ModelNotFound/ProviderUnavailable；不填假输出、不逐词拼句。真实模型推理backend与分发权重本轮未交付，机械provider仅测试生命周期。

## 用户自定义在线API

使用用户显式填写的完整Chat Completions endpoint，不猜默认服务或默认模型。配置启用且请求明确允许向所选provider发送文本才可联网，离线策略拒绝所有network provider；API key存在不等于同意发送。不自动重试/跨provider回退。请求model/messages/stream=false；响应要求完整assistant文本，拒绝refusal/toolcall/截断或错误shape，不执行远端内容。

HTTPS证书正常校验，禁止重定向/系统代理/URL userinfo/query/fragment，HTTP仅显式loopback测试或本地服务模式；连接和总deadline、request/response字节上界。401/403、429有界数字Retry-After、5xx、断连/超时、非法响应分类清楚，无响应体日志。测试只用本地TcpListener，不使用真实key/API。

拟精确采用缓存已有reqwest0.13.3（default-features=false，rustls-no-provider）、rustls0.23.40 ring/std/tls12、tokio1.52.3及必要serde/serde_json；最终实际lock/transitive metadata与许可证原件逐项审核并固定政策，未核者不能隐式加入。核心crate依赖closure继续不含网络库。参考[官方Chat协议](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)与[固定reqwest文档](https://docs.rs/reqwest/0.13.3/reqwest/struct.ClientBuilder.html)，本轮不是官方服务接入或翻译质量证明。

首次实际RustSec审计拒绝上述缓存rustls/tokio版本（4条rustls advisory及tokio unsound警告）。保留失败数据库commit与原文，实施前改为精确rustls0.23.46/tokio1.54.1并重锁/重审，不忽略警告；reqwest0.13.3无该次告警。TLS库实际默认protocol retry也显式关闭。系统DNS不能即时硬取消时保留worker直至实际退出，不通过提前drop runtime制造无限遗留DNS线程。

## 输入法交互

平台无关面板保留原文、请求上下文、当前task/result version。选择词/短语或明确触发整句→等待→展示原文/译文→复制或确认。复制只是返回给平台的clipboard intent，不直接操作系统；提交只有显式确认且task/context/result version仍当前时返回commit intent，不能自动调用输入核心。取消保留原文与输入状态；失焦/新会话/新请求取消旧task并拒绝迟到结果；同结果不得重复确认。真实复制/TSF/候选UI不在本轮连接。

只读复核复现：新Service的TaskId可从1重新分配，旧失败snapshot不含result.context，原update仅核ID会污染新epoch。先保留失败反例；接口改为`update(task_id, originating_context, snapshot)`，所有状态先核请求发起时捕获的context，不能由宿主用当前面板context替代来源。无需全局ID或新增线程。TaskId仍仅Service内唯一，换Service必须更换session/epoch。

## 失败测试、回归与交付

先提交设计，再以真实实现测试覆盖Unicode/空白/超长、双向/Auto、能力/配置、worker隔离/队满/取消/超时/迟到、cache entry/bytes/version、离线零连接、API零许可/密钥缺失/泄漏、429/断连/大响应和确认后提交。使用真实本地服务器、内存数据及受控provider同步，不复制生产算法。fixture明显标机制用途，不伪造真人授权或正式语义。

逐条保存命令/返回码/原始日志，fmt/Clippy/all-targets/doc/release locked/offline、Python与workspace审计；不复用旧CI，最终新head核Windows/Linux push。最终`docs/reports/translation-round11-status.md`区分词典检索、整句API、仅接口模型、UI/TSF和真实质量/许可阻塞，绑定源码/工具/数据/二进制SHA及新独立Draft PR，不改变旧四PR。
