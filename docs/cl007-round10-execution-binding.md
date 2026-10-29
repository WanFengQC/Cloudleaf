# CL-007 Round 10：执行授权绑定完整运行身份

2026-10-30。起点`1a05633c27c133bc0558b0fb955b0f916edb6b72`。先审Round 9实现，再提交旧实现失败反例，随后最小修复工具。生产Rust、共享接口、资源报告器与历史材料不改；不运行长测、独立native/loader或TSF。

## 实际缺口与接口

Round 9实际授权文件只绑定环境、人员、时间和档位。runner比较的完整`approval.identity`位于外层可重填的请求；更换二进制、源码、数据、工具或配置并同步外层identity，旧授权文件原字节仍可通过。此处是准入一致性缺口，不是已经观测到生产丢字。

执行授权升级为schema 2，增加`execution_identity_sha256`，必须等于本次完整identity的规范JSON SHA-256。规范为UTF-8、键排序、紧凑分隔、保留Unicode、不接受非有限数字；只排除执行通过后加入的`execution_approval_sha256`以避免自引用。其他字段包括commit/source/data/binary/tool、配置和实际系统环境全部参加；改动任一字段需要用户重新核验并出具新授权原件。旧schema 1不得自动升级。SHA只证明声明与运行身份一致，不证明真人签署或快照实际可恢复。

只读材料检查分列`materials_consistent`、`execution_identity_binding_checked`和`ready`。无实际identity时可以报告材料一致，仍`ready=false`并列明尚未核本次执行身份，不能以独立checker的exit0授予执行。完整runner仍在创建segment/child前使用同一入口核验；短档行为不变。原先正向fixture升级为schema2并传入同一临时identity，保留原断言；字段缺失测试也提供identity，避免因无identity一概失败而掩盖断言。

## 变更范围与验收

- checker增加一个可复用的规范身份摘要概念、schema2绑定和报告字段；必要时runner只传播报告诊断，不改变执行流程。
- 新增“外层身份更新、旧授权不变必须拒绝”的真实红测，覆盖二进制/源码/数据/工具/commit/配置；新增独立材料检查不放行及自引用字段排除测试。
- 新Round 10 pending授权、环境、runner批准和真实应用执行步骤/记录模板保留缺件；不改Round 8/9历史模板，不填人员或授权，不虚构结果。
- 现有逐segment中断/续跑、原始分母、COM/Unknown/预算测试全部保留并回归；不中断重跑或拼造长期样本。
- 全套fmt、Clippy/all-targets/doc/release locked/offline、Python、workspace/依赖审计逐命令保存返回码和原文；绑定源码、数据、工具和二进制SHA，普通push后由共享方向核新最终HEAD的Windows/Linux CI。

隔离环境、真实回滚、负责人和执行授权仍未落实，8h/24h、系统操作和真实应用验收均not-run；本工具不消除这些外部阻塞，不进入CL-010。
