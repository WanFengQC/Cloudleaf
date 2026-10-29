# PR #1 Round 7 合并前审查准备

2026-10-29。结论：**可以进入人工合并审查**。这不是人工批准、自动合并或CL-010放行。共享分支本轮只写计划、审阅及证据，不修改生产接口或合入方向实现。

## 实际审阅版本与差异范围

`main=a564fd47b0f054ff360592af3ccb8d744021160f`；起始PR #1为`c48584d411eceb82dcc6b8a30df9a33d9e995762`。本轮完整本地检查在计划提交`4f19e96`上执行，共享代码仍为`ce2c16e8f53da9913f2624675c7807d07c044877`。后续交付提交只增加审阅和报告，最终精确push CI在推送后另核，不用父提交成功代替。

完整main→Round 6共享树差异为112个文件、2,337,028字节Git patch，逐文件状态、Git blob、字节数和SHA见[差异证据](pr1-round7-evidence/review.json)。完整patch在本机`.local/round7-pr1-review/main-to-round6-shared.patch`，可用记录的两完整SHA和`git diff --full-index --binary`重建。Round 7新增计划及本文件属于审阅材料，不改变该生产差异。最终交付后另在`.local/round7-final-receipt.json`保存main→最终HEAD的全部实际目录和patch SHA，覆盖本报告/证据自身，避免自引用提交身份；最终完整HEAD和CI也登记于PR #1。

实际可执行增量集中在：`cloudleaf-types`、`cloudleaf-input`、两组真实公开API集成测试、`check_workspace.py`、精确`core.json`和CI locked-fetch；Cargo.lock、生产候选/拼音/词典、三份原型资源未变。审查了这些实际源码/测试/政策差异及契约v2；生成的历史结果表逐文件核身份，不宣称已逐行人工确认所有语言或平台结论。

## 公共接口与边界

|核对项|实际实现与审阅结论|
|---|---|
|状态与预判|`key_disposition`只读字段，不调用handle、解析、词典、快照或分配候选；与handle保持inactive/pending/failed/Escape/mode、长度/控制字符错误顺序一致。预判只代表同一状态，焦点/上下文改变后不能沿用旧许可。|
|提交三态|Applied/NotApplied由平台证明后才能发送成功/失败确认；Unknown无核心确认事件，保留pending并隔离，不转成可重试的CommitFailed。明确失败只在RetryCommit显式请求时取得新ID；取消后旧确认返回StaleCommit。|
|事件幂等范围|重复只读预判不改变状态；重复成功/失败确认不会再提交。普通字符Key本来就会逐次消费，合法自动连发不能去重。native sequence/session/epoch去重和排队归平台，不声称核心对所有Key事件幂等。|
|候选失效|`select_candidate`先校pending/failed和当前可提交状态，再revision和ID；拒绝不改帧；成功走同一begin_commit。实际改变帧的选词/编辑/导航及失焦使旧revision失效；已经处于导航边界且帧未变时保留原revision，不要求仅因按键发生而失效。ID只在单个引擎内唯一，不替代平台完整session身份。|
|配置与预算|page_size=1..9、max_candidates=1..128由构造入口校验，默认5/50；数字选择限制当前页有效索引。保留64ASCII输入保护和候选库硬预算，没有扩大默认产品承诺。|
|Unicode与编辑|辅助函数在切片前用is_char_boundary拒绝越界/UTF-8中间字节，按UTF-16 code units计数；补充平面汉字/代理对、组合附加符和ZWJ有回归。当前preedit仍ASCII，光标字节移动与退格安全；没有因此实现字素簇或任意Unicode组合编辑。|
|耗尽与错误|revision耗尽停止状态改变；checked_add在提交变更前拒绝序号耗尽。Blocked/handled=false有错误时宿主必须先处理恢复，不能把它解释成可以随意重放或丢弃。|
|兼容性|新ErrorCode枚举项和公开方法是workspace源码接口变化，不是冻结二进制ABI；外部穷尽match需重新编译。现有三方向input/types Git blob与共享一致，各自CI编译，未发现当前内容冲突。|

## 真实测试、依赖与安全审计

`platform_contract_v2.rs`及`round6_shared_review.rs`从公开入口构造真实Dictionary/InputEngine，检查预判/派发、候选/分页、事务和Unicode，没有复制候选排序或用mock替代核心。原单元/41事件回归、83种子及失败分母保留；平台模型只证明协议机制，不证明真实TSF或正式语言质量。

本轮12项命令全部exit0：Rust1.90.0、Python3.12、locked fetch、offline debug/release、fmt、Clippy `-D warnings`、64 Rust、doc、资源审计、14 Python及diff检查。原始输出、命令/返回码和release宿主身份见[验证](pr1-round7-evidence/validation.json)。源码未改时不重复R6机械性能矩阵，CI smoke不等于平台性能验收。

本共享树实际6个MIT本地crate、0外部Rust依赖；lock与MSRV/edition保持。workspace政策校验精确名字集、本地来源、MIT、MSRV和edition；外部包按name/version/license/source四字段精确匹配，资源按独立政策完整对应。重复/缺失/冲突/越界及hash损坏有真实反例。三资源的原件SHA、格式、条数和MIT文件通过；一般资源审计不等于上游授权真实性或数据质量审查。Windows分支依赖有独立政策，不以共享零外部依赖替代其审计。没有把“未发现缺陷”解释为全部代码无安全风险。

已重新核对起点精确push CI：共享40853466412、A40705631876、B40706957058、C40852885211的完整SHA、Windows/Linux job/步骤及两个未过期产物digest，[原响应](pr1-round7-evidence/baseline-ci.json)仅证明各Round 6版本。本轮最终共享与方向HEAD的新CI另存Round 7汇总和PR最终交付。

## 方向差异、阻塞与后续整合

三方向实际merge-base仍为`9af9bc1b2ebacc3ad76124cfa39d09de8eb77bd1`，不是当前共享分支。各方向相对merge-base、相对共享的完整文件清单和非破坏性merge-tree预览见差异证据。input blob=`ae704e7affd9d267328e778b27872195755c8f9e`，types blob=`2a35eec1d787deb979e096b62c834244c4723505`，内容一致不能证明已包含共享的新审阅测试/政策收尾。预览只产生临时Git树，不修改分支，也不是集成CI。

- **仍需修改**：本轮已审共享生产范围没有新的可复现P0/P1需要代码修复；不为增加修复数量扩大接口。审查材料和方向独立缺陷修复不等于人工批准。
- **阻塞项**：人工审查/合并决定未完成；A连续长测、部署级COM/Host隔离恢复与真实应用未验收；B独立真人gold/封存和正式资源门禁未通过；C现代频率、正式语义和真实授权审核材料不足。后三类是方向退出阻塞，不能以共享PR通过取消。
- **整合顺序**：只有人审PR #1决定合并后，才固定实际新main完整SHA和合并方式。保留每方向原head，分别正常合并明确的新main；先检查共享接口/审计兼容，再按C数据协议→B评估适配协调，A平台独立验证。逐分支处理冲突而不删除旧测试/证据，重跑本地全套与方向专项，并核新head的两平台push CI；无强推、不自动改PR基底、不用旧绿灯替代新提交。

四个PR保持Draft，不合main，不注册、激活、安装或卸载系统输入法，不启动未获批长测或CL-010。
