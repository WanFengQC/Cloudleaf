# Round20：精确搜索成本、局部增量复用与整合准备

2026-11-16，基线`4d42363091361552ad525f47e3c6b1d581ae3a4c`，分支`codex/input-round14`。先核对AGENTS、路线、Round19设计/报告/回执及真实源码。工作区起始干净，保留所有旧测试、数据及历史失败。窗口自动化维持用户暂停，不启动GUI、TSF或长测，不改main和五个Draft，不进行分支合并。

## 已观察问题与范围

Round19在相同837个最终查询、K10、200000展开/50000驻留下仍有6次ResidentPaths失败，为两字符模型的简拼`n×32`各三轮；完整查询延迟未达5ms，Lexical与字符模型都存在退化。搜索状态保留完整正文、ID和edge向量，后缀也保存完整路径；需先实测识别分配、平局比较、驻留计费与后缀重复工作，不能猜测热点或提高预算。

搜索变更限定`cloudleaf-search`现有精确路径和必要测试，不改Reference定义或完整Candidate比较。不同字符历史、解释代价、偏好和路径来源不得未经证明合并。未完成Reference不能充作答案。原始6次失败、超时和新增失败均保留，预算失败仍有原文回退。

局部增量复用限定可证明兼容的已有计算，不把旧Top10当新输入答案；缓存身份、失效、容量与观测先补充到本约定，再实现。输入session、revision、raw/cursor、scheme、词典/模型/用户状态及排序配置的变化必须明确处理。复用仍生成当前revision的候选身份，取消不能发布半成品。没有实测收益的实验不默认开启。

`cloudleaf-input`负责局部计算复用及事务边界，Windows只补无HWND实际worker/Host协议回归；存储不重写，不自动启用学习。原Applied证据、Unknown隔离、后台文件操作与128MiB估算门禁保留。

## 本轮独立工作与验收

真实输入工作区仅主执行者写入。搜索优化、局部缓存及只读整合审查分别在独立临时副本完成并提交设计、反例和结果，主工作区依次审查应用；不并发修改共享接口。

所有对照固定原123544词典、研究语言模型、837最终查询、171逐键事件、用户层OFF/空/4/512/4096、K与预算，分开记录最终查询、Engine K10与Host K5，保留P50/P95/P99/max、完整性、资源与缓存观测。完整Candidate/Features/edge/来源/顺序严格比较，不能只比较正文。正式语言指标继续null。

实际修改后逐命令执行原13项检查，绑定源码、数据、工具和二进制SHA。普通提交推送后等待最终完整HEAD两平台push CI并核验日志/产物SHA与ZIP CRC，再生成Git外交付回执。

只读Git比较核实五个方向的实际HEAD、merge-base、历史与patch差异，必要时使用不改ref的merge-tree预览；输出`cloudleaf-integration-readiness.md`。未来人工窗口清单写`testing/round20-windows-manual-acceptance.md`，全部未执行步骤保持`not-run/user-paused`，不虚构按钮、人工批准或真实Applied。整体仍需各自原门禁，CL-010关闭。
