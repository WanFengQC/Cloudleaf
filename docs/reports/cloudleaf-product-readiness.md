# Cloudleaf 产品能力追踪

Round 28 延续原始 50 项要求和验收定义。真实桌面为 `not-run/user-paused`。原始需求与交付条件见[产品范围](../product-scope.md)，本轮证据见[交付状态](integration-round28-status.md)。

| 原需求 | 当前状态 | 证据与缺口 |
| --- | --- | --- |
| REQ-IN-01 | automated-verified | 全拼、组成、编辑、明确提交与原文回退已有固定事件回归 |
| REQ-IN-02 | automated-verified | 光标、退格和 UTF-16 边界有契约测试，应用输入兼容待核 |
| REQ-IN-03 | automated-verified | 空格、数字、分页与事务测试通过，真实键位覆盖待核 |
| REQ-IN-04 | manual-not-run | 隔离宿主可切换模式，真实系统修饰键矩阵未执行 |
| REQ-IN-05 | implemented | 默认关闭。68 条冻结研发轨迹最终为 55/68 首选、66/68 Top-10，保留原文提交、事务和学习约束。真实应用待验收 |
| REQ-PY-01 | automated-verified | 标准全拼、分隔符及音节解析回归 |
| REQ-PY-02 | automated-verified | 多切分候选格和固定 Scheme 差分 |
| REQ-PY-03 | research-only | 纠错开关可经宿主热应用，独立精确率及误改缺测 |
| REQ-PY-04 | research-only | 模糊音可经宿主热应用，保留固定分组和代价回归，实际误配待核 |
| REQ-PY-05 | automated-verified | 音节解释惰性生成，1,656 键配对候选和提交身份一致。两条简拼仍截断 7、45 次，两个已知目标未恢复 |
| REQ-PY-06 | automated-verified | 小鹤双拼配置、输入轨迹与关闭语义已测 |
| REQ-CA-01 | automated-verified | 可追溯词条格、来源和多条合法路径 |
| REQ-CA-02 | research-only | 简拼减少每键边解释分配，两个同预算目标仍缺失。精确对照和失败分母保留 |
| REQ-CA-03 | research-only | 已验证支持度保护、分词偏差和配对退化，默认排序沿用 |
| REQ-CA-04 | automated-verified | 候选去重、预算诊断和修订过滤 |
| REQ-CA-05 | research-only | 本轮惰性解释限定在单图；跨键缓存仍默认关闭 |
| REQ-CA-06 | research-only | 独立长释义面板按任务与输入修订隔离，候选提交与学习路径保持独立 |
| REQ-LE-01 | automated-verified | 仅可信 Applied 产生学习凭据，译文与 Unknown 不学习 |
| REQ-LE-02 | automated-verified | 关闭与清理学习的存储回归，敏感场景人工验证待做 |
| REQ-LE-03 | automated-verified | 用户词库编辑、持久化、备份与恢复自动回归 |
| REQ-DI-01 | blocked | 固定词库可追溯，正式现代统计词频与独立质量未获批 |
| REQ-DI-02 | automated-verified | 原子管线、双构建、来源核对与坏数据拒绝 |
| REQ-DI-03 | automated-verified | 版本、指针、回滚与进程中断已验证，物理断电未执行 |
| REQ-DI-04 | research-only | 语义索引可按中文词头和完整英语义项查询，保留来源和缺失词性；正式词性和显式义项关系仍缺 |
| REQ-DI-05 | automated-verified | 缺损资源拒绝且保持基础输入，原子替换对旧状态有回归 |
| REQ-TR-01 | research-only | Bridge 与独立 Win32 长释义页具有输入修订门禁，可核验研究索引及来源。真实桌面和正式语义数据待验收 |
| REQ-TR-02 | research-only | 63 条固定双向模型原文一致，增译指令有明确人工复核提示，角色和隐含排他仍缺独立评估 |
| REQ-TR-03 | manual-not-run | 原生面板及隔离宿主确认代码保留，真实交互继续暂停 |
| REQ-TR-04 | automated-verified | 隔离宿主、独立面板和 Bridge 使用同一后台 Worker 协议，定额计算池、取消、方向切换和结果身份均有无窗口回归 |
| REQ-TR-05 | blocked | 已核对替代权重及部署方案，新增权重授权、实测和独立分发审批待交付 |
| REQ-TR-06 | implemented | 独立面板支持用户自定义在线 API 与本地协议回归 |
| REQ-TR-07 | automated-verified | 显式授权、凭据隔离和禁默认联网，独立人工安全验证未执行 |
| REQ-WIN-01 | blocked | 已有原生 TSF 代码，隔离环境和注册批准缺失 |
| REQ-WIN-02 | manual-not-run | 原生事件映射、焦点及事务隐式测试保留，实际跨应用暂停 |
| REQ-WIN-03 | manual-not-run | 隔离宿主候选可构建，DPI、可访问性和实际候选交互未验 |
| REQ-WIN-04 | automated-verified | 三态、未知隔离、迟到回执与恢复路径自动回归 |
| REQ-WIN-05 | manual-not-run | 记事本、浏览器、办公编辑和多行真实应用未执行 |
| REQ-CFG-01 | implemented | 隔离 Windows 输入宿主可对同一配置文件热应用并回执，组合和未知提交延迟处理。正式 TSF 及桌面验收待办 |
| REQ-CFG-02 | implemented | 资源页显示 Worker、宿主活动配置与快照身份，可分槽核验基础和补充词典；分别加载与切换待实现 |
| REQ-QA-01 | automated-verified | 本地全回归和最终 Windows/Linux push CI 单独核验 |
| REQ-QA-02 | blocked | 运行正式就绪门禁，真实独立人工 gold 尚无 |
| REQ-QA-03 | research-only | 171 键八轮真实推理配对绝对时延通过，倍率 1.533× 超门槛；简拼时延和真实系统仍待验收 |
| REQ-QA-04 | blocked | 状态机和有界资源回归保留，8h/24h 准入未放行 |
| REQ-QA-05 | implemented | 隔离发行增加允许列表用户资料迁移及 unsigned 签名请求；正式签名、系统安装器和真实系统验收仍待交付 |
| REQ-QA-06 | automated-verified | 不输出私人输入的研发协议、本地默认与学习保护，经自动检查 |
| REQ-QA-07 | automated-verified | 来源锁、许可原件、依赖和产物校验可重放 |
| REQ-PL-01 | automated-verified | 共享 Rust 核心在 Windows/Linux 构建，平台业务隔离 |
| REQ-PL-02 | not-implemented | macOS InputMethodKit 系统适配待实现 |
| REQ-PL-03 | not-implemented | Linux Fcitx5 / IBus 系统适配待实现 |
| REQ-PL-04 | implemented | 锁定版本、兼容资源与精确提交证据已具备，产品发行流程待验 |

桌面暂停，TSF 与长测未执行。独立人工 gold、正式词频、语义内容和模型分发许可仍各自受限，CL-010 保持关闭。
