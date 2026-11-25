# Round 22 原生翻译、工作进程与译文完整性设计

基线为 `61dbe04ca10c9d175be7ca5fc117d8dc6fb59d39`。在 `codex/integration-round21` 上继续开发。保持原输入搜索、资源预算、用户学习与 TSF 部署约定。本轮界面验收为 `not-run/user-paused`。

## 权威输入与原生操作

`adapters/windows/isolated_input.rs` 的 `LocalHost` 保留现有实际引擎和用于预检的投影引擎。翻译 UI 由现有窗口宿主提供，控制器捕获当前候选或宿主的英文编辑缓冲区。触发时不提交原文。翻译身份包括会话、epoch、修订、候选、词典、语言资源、方案及实际 worker 请求。输入核心提供只读翻译预检和可复用的来源身份。输入修订、资源、真正的外部失焦或会话变化使未确认结果失效。宿主内控件焦点移动保留原始输入，词库管理显式取消当前未确认任务。

确认时两个引擎先只读预检，再走现有 `same_semantics` 和文档写入路径。Applied 清理一次，NotApplied 保留原文和译文并等待显式重试，Unknown 隔离并保留核对材料。翻译确认不产生 SelectionEvidence。保留 Round 21 的无窗口入口和普通按键语义。

## 进程协议与资源

`cloudleaf-translation-ipc` 只使用锁中已存在的序列化依赖。`cloudleaf-translation-worker` 是独立可执行程序，沿用翻译调度、Marian 和 SentencePiece。输入 DLL 不包含模型、网络与张量后端。隔离宿主显式发起时按需启动专用同目录 worker。启动规则使用固定文件名、绝对目录及父子继承的标准输入输出，避免搜索 PATH。按长度前缀帧读取 UTF-8 JSON，帧尺寸、正在执行的任务和队列均有上限。协议握手、版本、worker generation、任务和请求绑定、错误、取消、超时与停止均需要明确终态。后台线程拥有 IPC I/O 与进程，窗口消息线程使用非阻塞队列，只有宿主可以上屏。

在专用 worker 内比较实际 1、2、4 个 Rayon 线程，观察底层计算、加载、响应、取消和分进程资源。仅加载指定方向时保留目录、manifest 和权重核验。相同方向的重复请求复用模型，方向更换时由后台代码管理驻留与释放。性能对照沿用固定 123,544 条词典、171 键轨迹、原预算及 K 值，交替重复并按实际活跃推理拆分样本。只有实测能决定默认配置，8 ms、空闲基线 1.5 倍和 5 ms 分开记账。

## 译文完整性

轻量 Rust 完整性检查使用有类型的数字、金额、日期、时间、单位和启用术语规则。报告区分等价表达、缺失、变化、歧义和无法核验。美元符号本身不足以确认 USD，中文日期可与英语月份表达等价。术语表独立于拼音用户词库，规定版本、方向、词边界、大小写和冲突行为。原始译文保留，首版显示原始译文。风险确认绑定当前结果版本与身份，变化后必须重新确认。独立语义审核和模型分发边界仍保持原状态。

## 修改和验证范围

- `crates/cloudleaf-input` 与 `crates/cloudleaf-bridge` 提供共用的输入来源身份与只读预检。搜索与排序不变。
- `crates/cloudleaf-translation-ipc` 与 `apps/cloudleaf-translation-worker` 实现实际 IPC 和受控推理进程，`crates/cloudleaf-offline` 保留已有面板接口并新增受限加载配置。
- `crates/cloudleaf-translation-integrity` 实现独立检查，`adapters/windows` 与 `apps/windows-ime` 实现现有宿主控件、焦点与双引擎事务。
- workspace、依赖审计、CI、手工验收矩阵、可重放性能报告和最终回执分别记录。CI 无权重，双向真实推理使用本机获准的固定权重。以独立故障测试验证 IPC、原文、学习、上屏和旧会话回执。所有旧回归、预算与分支保留。
