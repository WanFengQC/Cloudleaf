# 语义 bundle 操作与实际未放行范围

本轮实现离线基础设施，当前没有实际获准生产语义 bundle。用户本人是项目负责人；机械 fixture 的作者/审核/批准文件只测试代码，不能拿到真实发布中使用。真实20项源义项队列仍 pending，POS、人工sense对应与有向翻译没有补值，默认输入引擎也不加载该包。

## 候选 reader 与严格清单

`python -m tools.data_pipeline.semantic_bundle build --source <完整新候选目录> --output <新bundle目录>`。

候选目录严格包括 `content.json`、`reviews.json`、`migration.json`、`NOTICE.txt`，内容引用的实际许可/权利/协议文件，以及每个catalog的完整旧schema2词典目录。内容、审核和迁移严格字段见[设计](../../cl009-round10-semantic-bundle.md)。候选构建拒绝未知或缺失资源，不覆盖输出；manifest固定全部路径/字节/SHA和当前builder原字节SHA。同一固定工具原字节与输入可重建相同manifest；尚未对比Windows/Linux跨平台成品哈希，不宣称跨平台字节复现已验收。

`python -m tools.data_pipeline.semantic_bundle verify --bundle <bundle>` 只读核结构/资源/ID引用及可发布缺件，pending payload可为null。API `read_bundle` 返回已核快照、records和只引用原关系ID的reverse_lookup，不生成逆向equivalent。候选结构通过不是生产授权。

旧 `cloudleaf-linguistic-sidecar-v1` 不自动升级；旧 `pipeline.verify_release/activate/rollback/recover_store` 未提供新可选参数时仍走原词典schema2逻辑。语义store另给路径，不能重用当前真实词典store或用户字库目录。

## 外部可信版本清单与真实审批

生产 `verify --production`、`activate`、`rollback`、`recover` 显式要求 `--trust-policy <实际外部策略JSON> --trust-policy-sha256 <实际原字节SHA>`。文件不得位于bundle或store内部；实际审批文件同样在外部、Hash匹配。只读取策略明确允许的精确manifest/content/review-binding/version与来源scope，包内自带approved不形成信任根。

信任策略格式 `cloudleaf-semantic-trust-policy-v1`，releases每项严格manifest_sha256/content_sha256/review_binding_sha256/reader_version/approval(path+SHA)/sources。approval实际文件格式 `cloudleaf-semantic-release-approval-v1`，固定同样的身份、status、authorized_by、reason。未提供真人材料就不填身份/批准，不把测试文件复制来凑通过。

外部文件与不同ID仅证明声明和字节一致，**不证明签字、权利或独立身份真实**。本轮没有密码签名器、密钥/签署部署或新密码依赖；`cryptographic_signature_verified=false`、`human_identity_or_rights_proven=false`。项目负责人必须在工具之外落实可信审批来源和实际权限，工具不为自己签发授权。

## 审核、升级和恢复

- 两reviewer不同且非作者；拒绝/分歧必须有独立仲裁、理由和当前binding。内容、来源原件/实际许可授权、协议和迁移变化使旧审核失效，不由工具代签新摘要。
- 首次manifest上一版为null；升级必须绑定当前实际manifest。新增/删除/修改/拆分/合并ID需显式迁移覆盖。scope/ID不完整或版本未知拒绝。
- activate复用原guard/owner lock/复制重验/不可变release和单指针。rollback验证previous，目标损坏拒绝；用户词库不复制、不清理、不回滚。
- recover需明确已退出owner PID及外部策略；即使首发在lock阶段中断没有current，也先核外部策略再处理遗留材料。有current时继续完整验证目标。无current恢复仅隔离孤儿，不批准或创建release/current。
- 活owner、PID复用/权限不明、危险路径仍使用原恢复拒绝规则。子进程中断测试不证明物理断电耐久性。只对测试自有临时文件夹运行正向发布/回退/恢复，不触真实store。

当前缺真实语义双审/授权、目标英语catalog/义项对应和可信生产审批；无生产语义包发布、TSF、8h/24h、main合并或CL-010。性能只可陈述本轮实际工具和回归运行，不用CI成功推断语言或真实平台通过。
