# Round18 固定真实词典 K10 研发观测

源码HEAD：`b57f61b22a47f818146c13c4a3d909477e38c616`。源码前后不变：True。全为engineering-only，正式Top1/5/MRR为null。

Engineering-only K10. Default171mixed events/run, no warmup discarded, process launch/loading/stdout excluded from Rust event timing. Scheme profile overhead measured separately from plain query. Ordinary Windows desktop, root may perform light owned-HWND tests concurrently; not controlled dedicated-latency acceptance. No TSF/heldout/user history. No per-algorithm memory claim.

## 默认全拼相同171事件配对

|模式|n|P50 ms|P95 ms|P99 ms|max ms|
|---|---:|---:|---:|---:|---:|
|old-lexical|855|0.7351|4.2114|5.9277|8.1426|
|old-addone|855|0.7127|4.9480|11.1612|16.1028|
|old-witten|855|0.7497|5.5452|11.8701|17.4100|
|new-lexical|855|0.7740|3.8933|5.2491|7.5222|
|new-addone|855|0.7254|5.3363|12.7359|18.4859|
|new-witten|855|0.8042|5.9186|12.1073|17.5397|

15组完整输出一致，只移除新增加且值严格为零/None的兼容元数据；未归一化正文、ID、路径、分数、统计或非零解释成本。没有丢弃首轮或长尾。

## 新方案逐键引擎观测

|方案/模型|逐键n|P50 ms|P95 ms|P99 ms|max ms|
|---|---:|---:|---:|---:|---:|
|full/lexical|420|0.1621|2.1245|3.2879|4.0325|
|full/addone|420|0.2088|1.8573|4.3684|10.2307|
|full/witten|420|0.1731|1.9958|5.3246|6.5835|
|abbreviated/lexical|420|6.5386|25.2546|31.5030|33.7253|
|abbreviated/addone|420|15.3052|22.9495|26.3408|34.9643|
|abbreviated/witten|420|14.3999|18.8248|21.5855|24.1881|
|xiaohe/lexical|126|0.5123|3.4017|4.3235|4.3241|
|xiaohe/addone|126|0.4232|6.0592|13.6949|14.1855|
|xiaohe/witten|126|0.4584|5.8857|12.6405|13.5241|
|fuzzy-nl/lexical|420|0.4641|2.6189|3.5425|4.6748|
|fuzzy-nl/addone|420|0.4760|7.1153|14.0540|16.7864|
|fuzzy-nl/witten|420|0.4318|6.8815|11.9228|14.6079|
|correction/lexical|420|1.0639|5.8981|12.5943|18.1460|
|correction/addone|420|1.3496|14.9875|21.3384|54.1398|
|correction/witten|420|1.1191|13.1466|17.0993|19.0732|

新方案三轮/模型；full/abbreviated/fuzzy/correction每轮20个最终案例，xiaohe13。全部最终profile查询837，预算失败90（30个唯一方案/模型/案例×3），均为Abbreviated+字符两模型Expansions200000。InputEngine预算失败时不提供局部选择，原文保留。所有中间逐键延迟保存，但scheme_trace的EVENT只打印error，不逐键打印budget；这里预算分母专指最终输入profile，不能冒称中间每键都无预算。

## 实际Top10观察

均为来源候选观察，不是gold；完整ID/Features/来源/评分保留原始RESULT与DIAGNOSTIC。`raw-unknown`仅历史测试标签，例如小鹤zz实际可解释为zou，不能据标签认定非法。

|方案|模型|输入案例|Top10正文|预算|
|---|---|---|---|---|
|full|lexical|zgr||None|
|full|lexical|bjdx||None|
|full|lexical|zg||None|
|full|lexical|zhg||None|
|full|lexical|zhonggr||None|
|full|lexical|wzgs||None|
|full|lexical|nihao|你好 / 呢好 / 你号 / 尼好 / 拟好 / 呢号 / 你毫 / 你耗 / 呢毫 / 尼号|None|
|full|lexical|nh||None|
|full|lexical|fuzzy-lin|林 / 临 / 邻 / 磷 / 赁 / 琳 / 麟 / 淋 / 霖 / 鳞|None|
|full|lexical|fuzzy-lilan|力兰 / 里兰 / 力栏 / 理兰 / 利兰 / 里栏 / 理栏 / 力蓝 / 立兰 / 力滥|None|
|full|lexical|typo-adjacent||None|
|full|lexical|typo-transpose||None|
|full|lexical|typo-long||None|
|full|addone|zgr||None|
|full|addone|bjdx||None|
|full|addone|zg||None|
|full|addone|zhg||None|
|full|addone|zhonggr||None|
|full|addone|wzgs||None|
|full|addone|nihao|你好 / 你号 / 你豪 / 尼好 / 你嚎 / 你毫 / 你諕 / 你皓 / 你恏 / 你昊|None|
|full|addone|nh||None|
|full|addone|fuzzy-lin|林 / 令 / 淋 / 临 / 另 / 零 / 邻 / 灵 / 琳 / 龄|None|
|full|addone|fuzzy-lilan|立兰 / 离兰 / 理兰 / 里兰 / 李兰 / 立郎 / 离郎 / 理郎 / 里烂 / 里懒|None|
|full|addone|typo-adjacent||None|
|full|addone|typo-transpose||None|
|full|addone|typo-long||None|
|full|witten|zgr||None|
|full|witten|bjdx||None|
|full|witten|zg||None|
|full|witten|zhg||None|
|full|witten|zhonggr||None|
|full|witten|wzgs||None|
|full|witten|nihao|你好 / 你号 / 你毫 / 你豪 / 妮好 / 尼好 / 逆好 / 你嚎 / 你耗 / 泥好|None|
|full|witten|nh||None|
|full|witten|fuzzy-lin|林 / 令 / 临 / 淋 / 零 / 琳 / 邻 / 灵 / 另 / 铃|None|
|full|witten|fuzzy-lilan|历览 / 里烂 / 里懒 / 李兰 / 立兰 / 理兰 / 里兰 / 离兰 / 李郎 / 立郎|None|
|full|witten|typo-adjacent||None|
|full|witten|typo-transpose||None|
|full|witten|typo-long||None|
|abbreviated|lexical|zgr|撰稿人 / 张国荣 / 中国人 / 在个人 / 中个人 / 这个人 / 在工人 / 种个人 / 之个人 / 者个人|None|
|abbreviated|lexical|bjdx|北京大学 / 比较大学 / 不仅大学 / 比较对象 / 北京对象 / 不仅对象 / 不决定性 / 比较东西 / 背景大学 / 北京东西|None|
|abbreviated|lexical|zg|中国 / 这个 / 职工 / 整个 / 主管 / 中共 / 最高 / 资格 / 主观 / 足够|None|
|abbreviated|lexical|zhg|中国 / 这个 / 职工 / 整个 / 主管 / 中共 / 主观 / 正规 / 转轨 / 照顾|None|
|abbreviated|lexical|zhonggr|中国人 / 中个人 / 种个人 / 中工人 / 种工人 / 重个人 / 中干扰 / 中感染 / 中公认 / 中固然|None|
|abbreviated|lexical|wzgs|物质公司 / 外资公司 / 文章公司 / 完整公司 / 位置公司 / 文字公司 / 网站公司 / 武装公司 / 物资公司 / 为止公司|None|
|abbreviated|lexical|nihao|你好 / 呢好 / 你号 / 尼好 / 拟好 / 呢号 / 你毫 / 你耗 / 呢毫 / 尼号|None|
|abbreviated|lexical|nh|内涵 / 农户 / 浓厚 / 女孩 / 南海 / 男孩 / 内核 / 内河 / 脑海 / 能耗|None|
|abbreviated|lexical|fuzzy-lin|林 / 临 / 邻 / 磷 / 赁 / 琳 / 麟 / 淋 / 霖 / 鳞|None|
|abbreviated|lexical|fuzzy-lilan|力兰 / 里兰 / 力栏 / 理兰 / 利兰 / 里栏 / 理栏 / 力蓝 / 立兰 / 力滥|None|
|abbreviated|lexical|typo-adjacent|你护送 / 你欢送 / 呢护送 / 呢欢送 / 尼护送 / 你后送 / 拟护送 / 尼欢送 / 拟欢送 / 呢后送|None|
|abbreviated|lexical|typo-transpose|拟合哦阿 / 拟合噢阿 / 你好哦阿 / 拟合哦啊 / 你好噢阿 / 拟合噢啊 / 你好哦啊 / 拟合喔阿 / 你好噢啊 / 你好喔阿|None|
|abbreviated|lexical|typo-long|今天提起暖气很好 / 今天提起拿起很好 / 今天提前暖气很好 / 今天提前拿起很好 / 今天提青年期很好 / 今天体青年期很好 / 今天提取暖气很好 / 今天提取拿起很好 / 今天提起怒气很好 / 今天提请暖气很好|None|
|abbreviated|addone|zgr||Expansions|
|abbreviated|addone|bjdx||Expansions|
|abbreviated|addone|zg||Expansions|
|abbreviated|addone|zhg||Expansions|
|abbreviated|addone|zhonggr||Expansions|
|abbreviated|addone|wzgs||Expansions|
|abbreviated|addone|nihao||Expansions|
|abbreviated|addone|nh|你好 / 你会 / 你和 / 你行 / 你喝 / 你花 / 女孩 / 你回 / 男孩 / 您好|None|
|abbreviated|addone|fuzzy-lin|林 / 淋 / 临 / 邻 / 琳 / 粼 / 蔺 / 躙 / 遴 / 𬴊|None|
|abbreviated|addone|fuzzy-lilan||Expansions|
|abbreviated|addone|typo-adjacent||Expansions|
|abbreviated|addone|typo-transpose|你会嚄啊 / 你会噢啊 / 你会哦啊 / 你会喔啊 / 你还嚄啊 / 你还噢啊 / 你还哦啊 / 你还喔啊 / 你很嚄啊 / 你很噢啊|None|
|abbreviated|addone|typo-long||Expansions|
|abbreviated|witten|zgr||Expansions|
|abbreviated|witten|bjdx||Expansions|
|abbreviated|witten|zg||Expansions|
|abbreviated|witten|zhg||Expansions|
|abbreviated|witten|zhonggr||Expansions|
|abbreviated|witten|wzgs||Expansions|
|abbreviated|witten|nihao||Expansions|
|abbreviated|witten|nh|你好 / 你会 / 男孩 / 女孩 / 你和 / 你喝 / 你行 / 你害 / 你还 / 你很|None|
|abbreviated|witten|fuzzy-lin|林 / 临 / 淋 / 琳 / 邻 / 粼 / 吝 / 蔺 / 躙 / 遴|None|
|abbreviated|witten|fuzzy-lilan||Expansions|
|abbreviated|witten|typo-adjacent||Expansions|
|abbreviated|witten|typo-transpose|你欢噢啊 / 你行喔啊 / 你号哦啊 / 你会哦啊 / 你会噢啊 / 你会喔啊 / 你河喔啊 / 你和哦啊 / 你和噢啊 / 你和喔啊|None|
|abbreviated|witten|typo-long||Expansions|
|xiaohe|lexical|xiaohe-nihao|你好 / 呢好 / 你号 / 尼好 / 拟好 / 呢号 / 你毫 / 你耗 / 呢毫 / 尼号|None|
|xiaohe|lexical|xiaohe-zhongguoren|中国人 / 中国任 / 中国认 / 种国人 / 中国仁 / 中国刃 / 中国忍 / 重国人 / 中过人 / 中国韧|None|
|xiaohe|lexical|xiaohe-beijingdaxue|北京大学 / 背景大学 / 北京大雪 / 背景大雪 / 被经大学 / 北京达学 / 被境大学 / 北京打学 / 背景达学 / 被京大学|None|
|xiaohe|lexical|collision-lo|罗 / 落 / 洛 / 络 / 逻 / 螺 / 裸 / 锣 / 骆 / 萝|None|
|xiaohe|addone|xiaohe-nihao|你好 / 你号 / 你豪 / 尼好 / 你嚎 / 你毫 / 你諕 / 你皓 / 你恏 / 你昊|None|
|xiaohe|addone|xiaohe-zhongguoren|中国人 / 重国人 / 中国任 / 终国人 / 种国人 / 中果人 / 众国人 / 仲国人 / 忠国人 / 钟国人|None|
|xiaohe|addone|xiaohe-beijingdaxue|北京大学 / 北京大雪 / 被警大学 / 被京大学 / 背景大学 / 被竞大学 / 被敬大学 / 被竟大学 / 被惊大学 / 北竟大学|None|
|xiaohe|addone|collision-lo|落 / 罗 / 咯 / 萝 / 络 / 螺 / 洛 / 骆 / 泺 / 鸁|None|
|xiaohe|witten|xiaohe-nihao|你好 / 你号 / 你毫 / 你豪 / 妮好 / 尼好 / 逆好 / 你嚎 / 你耗 / 泥好|None|
|xiaohe|witten|xiaohe-zhongguoren|中国人 / 众国人 / 重国人 / 中国任 / 种国人 / 终国人 / 中果人 / 仲国人 / 中国认 / 钟国人|None|
|xiaohe|witten|xiaohe-beijingdaxue|北京大学 / 北京大雪 / 背景大学 / 北京大血 / 被警大学 / 背景大雪 / 被京大学 / 北京大靴 / 被警大雪 / 被敬大学|None|
|xiaohe|witten|collision-lo|落 / 罗 / 咯 / 萝 / 螺 / 洛 / 骆 / 络 / 裸 / 逻|None|
|fuzzy-nl|lexical|zgr||None|
|fuzzy-nl|lexical|bjdx||None|
|fuzzy-nl|lexical|zg||None|
|fuzzy-nl|lexical|zhg||None|
|fuzzy-nl|lexical|zhonggr||None|
|fuzzy-nl|lexical|wzgs||None|
|fuzzy-nl|lexical|nihao|你好 / 呢好 / 你号 / 尼好 / 拟好 / 呢号 / 你毫 / 你耗 / 呢毫 / 尼号|None|
|fuzzy-nl|lexical|nh||None|
|fuzzy-nl|lexical|fuzzy-lin|林 / 临 / 邻 / 磷 / 赁 / 琳 / 麟 / 淋 / 霖 / 鳞|None|
|fuzzy-nl|lexical|fuzzy-lilan|力兰 / 里兰 / 力栏 / 理兰 / 利兰 / 里栏 / 理栏 / 力蓝 / 立兰 / 力滥|None|
|fuzzy-nl|lexical|typo-adjacent||None|
|fuzzy-nl|lexical|typo-transpose||None|
|fuzzy-nl|lexical|typo-long||None|
|fuzzy-nl|addone|zgr||None|
|fuzzy-nl|addone|bjdx||None|
|fuzzy-nl|addone|zg||None|
|fuzzy-nl|addone|zhg||None|
|fuzzy-nl|addone|zhonggr||None|
|fuzzy-nl|addone|wzgs||None|
|fuzzy-nl|addone|nihao|你好 / 你号 / 你豪 / 尼好 / 你嚎 / 你毫 / 你諕 / 你皓 / 你恏 / 你昊|None|
|fuzzy-nl|addone|nh||None|
|fuzzy-nl|addone|fuzzy-lin|林 / 淋 / 临 / 邻 / 琳 / 粼 / 蔺 / 躙 / 遴 / 𬴊|None|
|fuzzy-nl|addone|fuzzy-lilan|立兰 / 离兰 / 理兰 / 里兰 / 李兰 / 里烂 / 里懒 / 例兰 / 历兰 / 立烂|None|
|fuzzy-nl|addone|typo-adjacent||None|
|fuzzy-nl|addone|typo-transpose||None|
|fuzzy-nl|addone|typo-long||None|
|fuzzy-nl|witten|zgr||None|
|fuzzy-nl|witten|bjdx||None|
|fuzzy-nl|witten|zg||None|
|fuzzy-nl|witten|zhg||None|
|fuzzy-nl|witten|zhonggr||None|
|fuzzy-nl|witten|wzgs||None|
|fuzzy-nl|witten|nihao|你好 / 你号 / 你毫 / 你豪 / 妮好 / 尼好 / 逆好 / 你嚎 / 你耗 / 泥好|None|
|fuzzy-nl|witten|nh||None|
|fuzzy-nl|witten|fuzzy-lin|林 / 临 / 淋 / 琳 / 邻 / 粼 / 吝 / 蔺 / 躙 / 遴|None|
|fuzzy-nl|witten|fuzzy-lilan|历览 / 里烂 / 里懒 / 李兰 / 立兰 / 理兰 / 里兰 / 离兰 / 例兰 / 历兰|None|
|fuzzy-nl|witten|typo-adjacent||None|
|fuzzy-nl|witten|typo-transpose||None|
|fuzzy-nl|witten|typo-long||None|
|correction|lexical|zgr||None|
|correction|lexical|bjdx||None|
|correction|lexical|zg|资格 / 足够 / 祖国 / 自古 / 阻隔 / 责怪 / 子宫 / 组阁 / 自贡 / 杂感|None|
|correction|lexical|zhg|这个 / 职工 / 主管 / 主观 / 足够 / 祖国 / 治国 / 直观 / 主干 / 扎根|None|
|correction|lexical|zhonggr|中隔 / 中个 / 中各 / 种个 / 种各 / 中合 / 中格 / 种合 / 重个 / 重各|None|
|correction|lexical|wzgs||None|
|correction|lexical|nihao|你好 / 呢好 / 你号 / 尼好 / 拟好 / 呢号 / 你毫 / 你耗 / 呢毫 / 尼号|None|
|correction|lexical|nh|努 / 怒 / 奴 / 弩 / 孥 / 胬 / 砮 / 呶 / 笯 / 驽|None|
|correction|lexical|fuzzy-lin|林 / 临 / 邻 / 磷 / 赁 / 琳 / 麟 / 淋 / 霖 / 鳞|None|
|correction|lexical|fuzzy-lilan|力兰 / 里兰 / 力栏 / 理兰 / 利兰 / 里栏 / 理栏 / 力蓝 / 立兰 / 力滥|None|
|correction|lexical|typo-adjacent|你好 / 呢好 / 你号 / 尼好 / 拟好 / 呢号 / 你毫 / 你耗 / 呢毫 / 尼号|None|
|correction|lexical|typo-transpose|你好 / 呢好 / 你号 / 尼好 / 拟好 / 呢号 / 你毫 / 你哈 / 你耗 / 呢毫|None|
|correction|lexical|typo-long|今天天气很好 / 今天天气很号 / 今天天气很毫 / 今天天气很耗 / 今天天气狠好 / 今天天气很豪 / 今天天气很浩 / 今天天气恨好 / 今天天气狠号 / 今天天气很郝|None|
|correction|addone|zgr||None|
|correction|addone|bjdx||None|
|correction|addone|zg|自高 / 阿家 / 自果 / 阿果 / 自国 / 自格 / 阿高 / 自狗 / 咱果 / 阿狗|None|
|correction|addone|zhg|这个 / 这狗 / 只狗 / 这果 / 这句 / 这歌 / 这国 / 这高 / 这过 / 这鬼|None|
|correction|addone|zhonggr|中歌 / 中个 / 中格 / 中哥 / 中合 / 重歌 / 重个 / 重格 / 中革 / 终歌|None|
|correction|addone|wzgs||None|
|correction|addone|nihao|你好 / 你号 / 你豪 / 尼好 / 你嚎 / 你毫 / 你諕 / 你皓 / 你恏 / 你昊|None|
|correction|addone|nh|怒 / 努 / 奴 / 胬 / 砮 / 弩 / 呶 / 孥 / 笯 / 驽|None|
|correction|addone|fuzzy-lin|林 / 淋 / 临 / 邻 / 琳 / 粼 / 蔺 / 躙 / 遴 / 𬴊|None|
|correction|addone|fuzzy-lilan|立兰 / 离兰 / 理兰 / 里兰 / 李兰 / 里烂 / 里懒 / 例兰 / 历兰 / 立烂|None|
|correction|addone|typo-adjacent|你好 / 你号 / 你豪 / 尼好 / 你嚎 / 你毫 / 你諕 / 你皓 / 你恏 / 你昊|None|
|correction|addone|typo-transpose|你好 / 你号 / 你好啊 / 你豪 / 你和啊 / 尼好 / 你虾 / 你嚎 / 你毫 / 你哈|None|
|correction|addone|typo-long|今天天气很好 / 今天天起很好 / 今天天祈很好 / 今天天歧很好 / 今天天骑很好 / 今天天器很好 / 今天天亟很好 / 今天天碛很好 / 今天天葺很好 / 今天天祇很好|None|
|correction|witten|zgr||None|
|correction|witten|bjdx||None|
|correction|witten|zg|足够 / 祖国 / 资格 / 阿根 / 自高 / 自格 / 阿贾 / 咱各 / 自哥 / 阿公|None|
|correction|witten|zhg|这个 / 只狗 / 这句 / 这狗 / 这鬼 / 这歌 / 这果 / 足够 / 这国 / 祖国|None|
|correction|witten|zhonggr|中个 / 中歌 / 种各 / 中格 / 重个 / 中哥 / 中合 / 种个 / 终个 / 重歌|None|
|correction|witten|wzgs||None|
|correction|witten|nihao|你好 / 你号 / 你毫 / 你豪 / 妮好 / 尼好 / 逆好 / 你嚎 / 你耗 / 泥好|None|
|correction|witten|nh|怒 / 奴 / 努 / 胬 / 砮 / 弩 / 呶 / 孥 / 笯 / 驽|None|
|correction|witten|fuzzy-lin|林 / 临 / 淋 / 琳 / 邻 / 粼 / 吝 / 蔺 / 躙 / 遴|None|
|correction|witten|fuzzy-lilan|历览 / 里烂 / 里懒 / 李兰 / 立兰 / 理兰 / 里兰 / 离兰 / 例兰 / 历兰|None|
|correction|witten|typo-adjacent|你好 / 你号 / 你毫 / 你豪 / 妮好 / 尼好 / 逆好 / 你嚎 / 你河喔 / 你和喔|None|
|correction|witten|typo-transpose|你好 / 你好啊 / 你号 / 你和啊 / 你毫 / 你豪 / 尼泊啊 / 妮好 / 尼好 / 你活啊|None|
|correction|witten|typo-long|今天天气很好 / 今天天气很豪 / 今天天起很好 / 今天天气很耗 / 今天天骑很好 / 今天天祈很好 / 今天天气很号 / 今天天器很好 / 今天天歧很好 / 金天天气很好|None|
