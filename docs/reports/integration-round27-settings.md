# Round 27 统一设置中心验收

基线为 `a6e48cbe28d48a010629a65537cfb6743522356a`。[设置设计](../integration-round27-settings-design.md) · [本轮交付](integration-round27-status.md)

| 区域 | 已接入与可验证行为 |
| --- | --- |
| 输入方案 | `cloudleaf-settings` 的 schema 1 共享输入偏好映射到核心方案与分页。全拼、简拼、小鹤双拼、七组双向模糊音、全拼或简拼纠错、混排与分页通过同一校验入口。混排默认关闭，重复或非法组合拒绝保存。 |
| 读写应用 | `input-settings.json` 与原翻译 `settings.json` 分别原子保存。输入保存校验现有 revision，使用有效上一版备份，保留损坏文件至下一次显式保存。未来版本受保护。面板后台给出读取和保存回执。已运行的输入宿主保全现有组合，显式指定目录的新宿主在启动时加载。 |
| 用户词库 | 后台直接复用 `cloudleaf-user::Store`，读取已绑定的绝对目录及代次，再执行添加、更新、移除、导入、导出与学习设置。显式恢复核对选定备份代次。故障或冲突呈现明确回执，编辑区文字独立保全。 |
| 翻译与联网 | 翻译 schema 3 原路径兼容旧 schema 1/2，重启恢复离线。沿用按端点绑定的 Windows 凭据命名空间。测试 API 消费一次显式授权，仅发送固定探测文字，响应不可复制为正常译文。保存失败维持原活跃服务；设置应用导致旧任务失效。新 Worker 只在旧 Worker 回收确认后启用。 |
| 资源诊断 | Win32 选项绑定 1/2/4 或自动线程与正常、Windows 低优先级研究档。离线 catalog 在后台按内容重新计算 SHA，状态窗口分别显示提供者与已观测的 Worker 模型、实际线程、时间和 PID。模型清单、权重和许可证状态通过原入口的首次任务校验。 |

## 验证与未完成边界

`cargo test -p cloudleaf-settings -p cloudleaf-translation-panel --all-targets --locked --offline` 的首轮完整专项通过 28 项，日志和精确源码 SHA 在 `.local/round21-runs/round27-settings-final`。该轮包含实际用户 Store 的增删改、导入导出、学习清理、设置回退、隔离凭据和回环测试。首轮 Windows 编译失败原因是旧 GUI 测试夹具手工初始化缺少新字段，已改为共用生产构造函数，原四条断言保留。原始材料位于 `.local/round21-runs/round27-settings-dev`。

首次专项 Clippy 报告 `collapsible_if`，返回 101。调整设置加载分支后，定向测试和 Clippy 复跑分别保存于 `.local/round21-runs/round27-settings-scope-verified` 和 `.local/round21-runs/round27-settings-clippy-verified`。复跑源码状态由记录的 HEAD `42a96d8fb63d4acdb7db4764db98e80f3ed20e22` 和逐文件 SHA 一起绑定。专项测试 29/29 通过，退出码 0，11.282 秒。测试 stdout SHA-256 为 `3af37b16dda3fdb15842fbd888e669d5943702078868d5f2f946a874f1511233`。严格 Clippy 退出码 0，3.281 秒。Clippy stderr SHA-256 为 `7bd8a7ede3af8b1cef2d6b9621a0a4befcd3b013d43c6e7f9fb795b1e6b9fee6`。复跑新增旧 schema 3 默认线程和正常优先级、实际设置绑定固定 catalog SHA 和显式线程档的断言。

主动保存缺少资源的提供者仍遵循已有行为，应用后一并显示不可用状态与原因。回环测试覆盖授权而不请求外部在线平台。语言资源诊断显示本面板已加载的提供者和模型。基础/补充输入词典的跨进程管理、自动语言方向的离线支持、全局热应用和对活动组合无损重配等待后续交付。Windows 设置界面在本轮构建，真实窗口和可访问性验收继续 `not-run/user-paused`。未做物理断电测试。
