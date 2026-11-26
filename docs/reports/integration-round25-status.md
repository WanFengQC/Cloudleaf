# Round 25 搜索与推理资源阶段交付

本轮以 `403ace1c9d95993f6117113f94924b58aee998bb` 为基线，在 `codex/integration-round21` 完成工程修改，产品状态为 **partial**。本地完整检查绑定证据清单中的实现提交。最终推送提交、精确 CI、二进制和构建产物的核对以 Git 外 `.local/round25-delivery-receipt.json` 为准。

| 方面 | 状态 | 结果 |
| --- | --- | --- |
| 精确搜索与旧行为 | done | 共享路径键、延迟材质化、紧凑键；837 组完整固定结果、188,325 次机械对照及两组完整参考均一致 |
| 搜索性能与驻留 | partial | 普通全拼两个组累计请求分配约减半；30% P95 时延和分配双指标仅补充标准组达到。驻留计数仍触及 50,000，短查询输入性能与长查询不同 |
| 七个历史驻留终止 | done | 50,000 条预算继续七次终止；离线 200,000 驻留时七次完成，五条目标入池，另两条仍不在 Top-50 |
| 固定离线 Worker 隔离 | partial | 真实 42 轮固定轨迹、16 轮扩展生命周期执行成功。默认 lexical 全部门槛未通过，word 研究模式两个本机策略通过。低优先级与线程档由已有隔离宿主显式选择 |
| 混合原文提交 | partial | 当前修订绑定的原文提交通过正常 CLI 与输入回执、数字和 URL 测试；自动混排及真实窗口交互待验 |
| 授权词典和排序 | partial | 现有补充资源覆盖提升，冻结组从 17/40 到 39/40；同词典新置信重排各 Top-K 不变，保留默认评分。软计数、置信重排和补充词典作为研究 |
| 固定 Marian 与语义保护 | partial | 63 条真实输出逐字一致，受控错误提示 2/12→10/12；合法合句仍有误报，角色与排他关系仍有漏报 |
| 正式授权及独立评估 | blocked | 生产词频仍为 unknown，正式 Top-1、Top-5、MRR 为 null；未取得独立人工 gold 和双语审校，无新语料或模型权重获取 |
| 替代模型接入与安装发行 | not-implemented | M2M-100、MADLAD-400 待权重准入；NLLB 保留排除结论。独立翻译面板和 Bridge CLI 的进程迁移、统一设置、安装链、macOS/Linux 系统适配待实现 |
| 系统验收 | not-run | Windows 窗口仍暂停，TSF、8h/24h 长测未执行，CL-010 保持关闭 |

源码范围覆盖精确搜索层及测试、输入核心和 CLI、现有 Windows 隔离宿主参数、Worker/IPC 计算池、结构完整性、许可约束下的词典和研究评估工具。工作区 unsafe 禁令、搜索排序、常规按键、候选学习、原始译文、复核和显式确认协议沿用。

实现提交 `064fe68d8b5c9b8b98faa21f2cf73fe2cb2f73c4` 的 16 项本地检查全部返回 0。Rust 439 项通过、3 项忽略，Python 202 项通过。权重 IPC 忽略用例另在 Release 真机跑过，Debug 超时记录保留。另两项是需要真实 DLL 路径的独立宿主装载和需显式运行的模拟 COM 基准，未纳入此次桌面准入。五项旧搜索与资源工具返回 0，三项独立语言及人工交接门禁各返回预期 2。失败日志、源码与二进制指纹见本轮证据索引。

后续产品工作优先解决默认 lexical 输入在模型并发下的尾延迟，建立独立的分配峰值、等待和缓存证据，随后将独立面板及 Bridge CLI 接到既有 Worker 进程并验证失败恢复。候选覆盖需继续在获准资源上验证多读音和长句。正式评估、替代模型及真实环境验收仍各有准入依赖。

[搜索与产品入口](integration-round25-search-performance.md) · [推理隔离](integration-round25-worker-isolation.md) · [数据](integration-round25-data-resources.md) · [中文研究对照](integration-round25-language-quality.md) · [翻译质量](integration-round25-translation-quality.md) · [50 项需求](cloudleaf-product-readiness.md) · [本地证据](integration-round25-evidence/index.json)
