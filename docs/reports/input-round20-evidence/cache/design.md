# Round20：跨键复用不可变Trie音节转移

基线4d42363091361552ad525f47e3c6b1d581ae3a4c。本子任务不修改真实工作区。

`schemes::from_phonetic`每个相交状态都会重新按音节字母遍历词典Trie；同一node与规范音节在同一次、追加、退格、中间编辑及不同方案中重复出现。缓存仅记录这个纯函数的下一node或不存在，不保存Candidate、旧TopK、正文或整句，不复用旧路径。

缓存完整键为不可变Lexicon的Arc identity、起点node、规范音节。不同raw/cursor/session/revision/scheme/模型/偏好可能改变音节图及路径排序，但不会改变给定词典节点的字母转移。每次仍从当前完整音节图构建所有边并运行现有完整搜索；原相交visits/预算检查位置不变。因此不跳过来源、编辑成本、搜索排名或候选身份生成。缓存有无命中不改变统计分母和错误类型。

词典切换按Arc identity清空；InputEngine的资源、模型和配置替换构建新backend；方案变化和clear_context显式清缓存。revision仅因追加/退格变化时保留纯转移；候选仍按新revision生成，旧candidate ID按原规则拒绝。会话内raw变化不清缓存，不保留原始输入文本。

固定4096个direct-mapped slot，每个24字节（node,next,syllable编码），约96KiB及对象余量，第一次非默认方案查询才分配。碰撞只覆盖旧缓存，不删当前搜索状态；未命中永远走原Trie。规范音节为解释器已验证的最长6个ASCII字母，base27编码u32无冲突，不在内部重复校验。缓存开启/关闭/clear有累积hits/misses/evictions/invalidations，不写输入日志。

默认全拼直接走原Lattice路径，不访问/分配缓存。先独立验证实际收益，收益不足不默认开启。Host必须预留两引擎最大缓存字节，原128MiB门禁不变；root修改session.rs，本子任务不触碰Windows适配。

文件：search新增transition_cache.rs及测试、lib.rs导出、schemes.rs增带缓存构造入口；input sentence/lib少量接线、clear与只读统计/开关；独立新集成测试和研发benchmark example。search.rs和character.rs由另一子任务独占。

严格验证缓存ON/OFF：全部Candidate/Features/edge/排序/fallback/预算和SchemeStatistics一致；追加、退格、中间修改、全拼/简拼/小鹤、模糊/纠错、用户词/偏好和模型/资源切换；同node不同词典不得串用。真实固定词典同机配对测量跨键复用、命中与淘汰，保留所有时延、失败、SHA，不声称语言或TSF质量。
