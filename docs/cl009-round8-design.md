# CL-009 Round 8：正式数据交付与语义发布器设计

2026-10-30。固定起点`3d573384ce1f31e437bdb4e7dc109cf438c261f4`，工作区clean。本轮进入外部验收材料与实际数据取得准备，不把增加工程测试作为目标，不改变生产代码、来源锁、词库、格式或发布指针。

## 可独立交付的工作

1. 在`docs/acceptance/cl009-round8/`交付逐源采用决定及精确缺件、现代统计的分母/提取/分词/原件和分发要求、外部执行顺序。
2. 在`data/plans/round8-external-handoff.json`维护唯一待填角色/授权/原件入口；只引用现有12项IME pending和既有协议，不创建第二份人员或审批真相。真实人员/授权未交付时保持null/pending，不改旧工作流格式。
3. 从已批准固定CC-CEDICT包中按稳定ID顺序取得20项真实原始义项审核待办，保留source_record_id、原行/事实SHA、许可/署名；新POS、义项对齐、有向翻译、审核及发布许可全部null/pending。它不是IME gold或代表性评估数据，不能由词典构造独立答案。生成源与固定输入身份进入本轮证据。
4. 将R7发布前缺项展开为版本化生产语义发布器设计：内容与来源/协议摘要、双审和仲裁失效、签名/可信清单边界、复用现有不可变release/guard/原子指针/显式恢复、兼容和用户字库保护。发布器、签署信任根和真实生产数据仍未交付，不调用activate/rollback或写用户词库。

## 实施与验证边界

现有v1侧包结构校验、token/DF/Zipf数学公式及原子发布/恢复没有已复现的新问题，不另造校验器或重跑未改变的机械性能/全包压力。只验证新材料与真实源逐项一致、20项全部待审/unknown、主模板无人员/授权、引用路径及许可边界、历史/生产源码未变，并运行必要fmt/资源审计/diff。旧Rust/Python证据保留原SHA；新最终HEAD仍须Windows/Linux push CI。

没有新获授权现代语料或双语语义原件，来源审核结果只能明确已有采用范围和当前不采用/待授权决定，不能凭再列一份矩阵声称取得新许可。Wiki、SUBTLEX、wordfreq、Ngrams、Leipzig不因本轮设计而批准；Wiktionary片段继续research_only，CC-CEDICT原始词条/义项既有范围不变。

与B约定：B引用本轮唯一角色/授权入口；context_review、intent-family和heldout加密保管由B交付清单定义，C不填答案或上下文、不代签。B旧70/15/15、至少2000heldout家族和正式null门禁不变。新报告`docs/reports/cl009-round8-status.md`明确完成、部分、外部阻断和未实现；不合main、不改基底、不进入CL-010。
