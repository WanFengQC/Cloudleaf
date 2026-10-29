# PR #1 Round 10：方向稳定后的人工审阅准备

2026-10-30。共享生产源码、公开接口、测试与资源相对`e296c300ea1b1d41c6cdc36500f690dec0906964`不变。本轮只归档设计、技术核对、方向证据与汇总，人工决定保持pending；Codex不替用户签署、切换Ready或合并。

## 审阅对象

main仍`a564fd47b0f054ff360592af3ccb8d744021160f`。保留[R8十四项未勾选清单](pr1-round8-human-review.md)与[R9补充](pr1-round9-human-review.md)，不重写历史、不将已有审查包装为新功能。[本轮空白决定模板](pr1-round10-decision-template.json)须由用户本人绑定最终完整PR/main SHA、清单/补充SHA及实际新CI后填写。

技术核对读取实际`key_disposition/handle_key/select_candidate/begin_commit/ack/retry`、提交三态、UTF-16转换及真实公开API测试；无新的可复现共享P0/P1发现。完整main→本轮实际被测head的文件/blob/SHA与patch身份归档到`pr1-round10-evidence/review.json`；含最终报告自身的完整差异和最终push CI在提交后另存`.local/round10-final-receipt.json`并绑定PR #1。不能用起点CI或实现提交冒充最终报告head。

## 本人仍须决定的内容

|待决定|技术边界|
|---|---|
|[ ] 公开契约和失败语义|只读预判不是永久事件ownership，执行前平台须重验上下文；revision/候选ID仍须session/epoch隔离。|
|[ ] 提交与Unicode|Unknown不映射NotApplied、不自动重试；Escape/失焦取消不证明原文档未写。UTF-16辅助只保证标量边界与code units，不保证字素编辑。|
|[ ] 完整差异及兼容|Rust源码契约不是稳定跨进程ABI；穷尽match消费者需重编。方向实现未合入共享，相同blob不等于集成通过。|
|[ ] 最终精确工程证据|最终push完整head、Windows/Linux全部步骤、两个未过期产物head/digest；mergeable和AI技术核对不能充当批准。|
|[ ] PR #1处理方式|本人填写批准/要求修改/拒绝及实际意见，另明确合并方式与执行授权；目前全部pending/null/false。|

## 方向兼容及外部风险

方向最终A=`e0ab78c2786d652a3a19cb544b4dc83996e63264`、B=`59dabdc2f8a3dc87cbd7f19ed89401ee607f7954`、C=`3f26f34e00c0775124f87e150a021f99fb52c567`。input/types blob保持`ae704e7affd9d267328e778b27872195755c8f9e`/`2a35eec1d787deb979e096b62c834244c4723505`；各merge-base仍为旧`9af9bc1b2ebacc3ad76124cfa39d09de8eb77bd1`。本轮没有合并预览或方向整合。

C新语义模块是独立入口，旧schema2默认verifier保留；生产调用必须显式提供包/store之外的受信策略，不能绕到旧通用入口当语义生产核验。文件声明一致不证明人、权利、语义真实性或密码签名。B计数合同通过仍有预算/剪枝、约95–99ms慢例以及字节内存/取消未实现，正式质量null。A准入绑定已修，真实环境/恢复/人员/授权、长期与应用验收仍缺。用户的PR决定不能解除这些门禁。

用户实际合并PR #1后才fetch并核真实新main/合并方式，保存方向旧身份并确认clean，逐方向普通merge、审冲突、执行适用全回归和新head两平台push CI；不rebase、强推或把方向代码互合到共享。当前四PR继续Draft、main不合、CL-010不放行。
