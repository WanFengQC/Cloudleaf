# PR #1 Round 8 最终人工审查清单

2026-10-30。**技术材料可供人工审查，人工决定仍pending；没有批准或执行合并。** PR #1相对main的源代码已有R7逐项审阅，本轮不为重复计数新增核心测试。mergeable=true、CI绿灯、无冲突预览和AI复核均不替代人工签署。

## 审查对象与证据

- PR：[WanFengQC/Cloudleaf #1](https://github.com/WanFengQC/Cloudleaf/pull/1)，base=main。起点main=`a564fd47b0f054ff360592af3ccb8d744021160f`，PR head=`e359e471a66599f0fa4fdc03dd9d802a242ebc3d`；R8仅新增审阅材料后须以GitHub最终完整head重新填写决定。
- main→R7最终共享差异142文件、2,990,191B完整Git patch，身份和目录见R7最终凭据；可从两个完整提交用`git diff --full-index --binary`重建。本轮最新目录、共享核心/政策blob和方向兼容性另存[R8技术证据](pr1-round8-evidence/review.json)，最终包含本报告自身的完整差异在推送后凭据记录，避免自引用SHA。
- 实际生产增量为InputConfig、KeyDisposition、select_candidate、CommitOutcome、UTF-16辅助和精确依赖/资源政策；其余主要是设计、真实公开API测试、报告和CI locked-fetch。生产拼音/词典/候选算法、Cargo.lock、原型资源未改。R8共享不改生产代码。
- [R7审阅与限制](pr1-round7-merge-readiness.md)、[契约v2](../core-platform-contract-v2.md)、[依赖政策](../dependency-audit-policy.md)、R7精确push CI [40908732505](https://github.com/WanFengQC/Cloudleaf/actions/runs/40908732505)及64 Rust/14 Python原文均保留。R8入口重新核run/head/两平台步骤/产物摘要，[原始快照](pr1-round8-evidence/baseline-ci.json)不改绑为R8新提交结果；R8最终HEAD另跑push CI。

## 由真人逐项确认

下列框保持未勾选；“已有证据”是技术材料位置，不代表真人已完成审查。审查人应结合完整diff、实际源码和返回值确认，不只看摘要。

|人工确认|检查内容|已有证据与必须保留的边界|
|---|---|---|
|[ ]|公开API/兼容性|`new/with_config/frame/failed_commit/key_disposition/select_candidate/handle`及新增ErrorCode；workspace源码接口，不是稳定跨进程ABI，穷尽match消费者要重新编译。|
|[ ]|输入状态/错误优先级|inactive、中文/英文、pending、failed、Escape、控制字符/长度和序号耗尽；有错误时先恢复，handled=false不等于可以丢弃或任意重放。|
|[ ]|只读预判|不调用handle/解析/词典，不生成帧或改变revision；同一状态下与执行相符，焦点/上下文变动后重验，旧预判不是永久ownership。|
|[ ]|事件/事务幂等范围|合法重复字符每次消费；重复预判和旧/重复ack不重复提交。native sequence和FIFO归平台；core commit ID仅单引擎内唯一，必须按session/epoch路由。|
|[ ]|三态与Unknown|Applied/NotApplied由平台证明才发对应ack；Unknown保持pending/隔离，不自动重试、不发可重试失败。Escape/失焦取消核心不能证明原文档未写入。|
|[ ]|候选选择/旧ID|先验提交状态，再revision和ID；拒绝不改帧，成功走相同提交路径。真实改帧才令revision失效，导航边界帧未变仍合法；不能按时间推断stale。|
|[ ]|Unicode/编辑单位|UTF-8切片前验证标量边界，UTF-16按code units，代理对/组合附加符/ZWJ有回归。preedit仍ASCII，不具备通用字素编辑或任意Unicode选区能力。|
|[ ]|分页/资源配置|默认5/50，page_size1..9、候选1..128，数字只选当前有效页；保留64ASCII和既有预算，不把原型边界固化为最终产品目标。|
|[ ]|测试真实性|platform_contract_v2/round6_shared_review使用真实公开入口；原64/41/83资源和失败分母保留。模型宿主不等于TSF/真实应用，手写种子不等于正式语言准确率。|
|[ ]|依赖/许可政策|共享6个MIT本地crate、0外部Rust依赖；成员名字及source/MIT/MSRV/edition检查，外部按name/version/license/source精确匹配。资源hash/路径审计不代来源许可真实性；A独立15依赖另审。|
|[ ]|完整差异/身份|核完整PR head、main head、源码/资源blob、报告差异、命令/返回码/原文，不用R6/R7旧成功证明后来代码。人工决定绑定最终SHA和清单内容。|
|[ ]|精确新CI|最终push事件、完整head、Windows/Linux job/所有步骤、两个未过期产物head/digest，不能用默认PR merge SHA替代。|
|[ ]|方向兼容与后续顺序|三个实际merge-base均旧`9af9bc1b2ebacc3ad76124cfa39d09de8eb77bd1`，input/types blob一致只是内容兼容。无冲突预览不代表已整合或集成通过。|
|[ ]|批准范围|仅决定PR #1如何处理；不放行长测、TSF、数据/语言质量、产品发布或CL-010，也不自动合并三个方向。|

## 人工决定及外部条件

待填写的[决定模板](pr1-round8-decision-template.json)不是已签署记录。真人确认参与后填写实际可核验标识、时间、最终完整PR/main SHA、清单/证据摘要和具体意见；选择批准/要求修改/拒绝之一，并另明确合并方式和执行授权。没有这些实物就保持pending，不把机器人或预设角色编号填成真实人。

当前用户只确认R7结果并要求准备人工审查，未批准或执行合并。环境/VM/快照及恢复验证/负责人/长测和TSF授权未准备；标注/双审/仲裁/heldout人员和文件待定；现代词频无获准原件。这些不靠共享PR通过解除。

## 用户实际合并后才执行的方向整合

1. 重新fetch，确认PR #1确实merged，记录合并方式、实际merge commit和新main完整SHA；若主分支另有变化，先审真实变化，不reset用户成果。
2. 确认各方向clean并保留旧head和数据/二进制/报告身份。逐方向正常合并明确的新main；不rebase、强推或自动改基底掩盖历史。
3. 处理共享接口/政策冲突，不删除旧测试、失败证据或数据。C实际来源/任务协议→B显式上下文/评估适配，A平台边界独立验证；不把三方向互相合进共享。
4. 每个新方向head执行适用本地回归/专项并取得自己的Windows/Linux push CI。旧CI、预览或相同blob均不能继承为新整合验收。数据、语言、长测/真实应用仍各自满足原退出门禁后才讨论CL-010。

本轮没有执行上述合并后操作，也未请求更改Draft状态。
