# Round18 双拼规则核验

2026-11-10，技术调查，不代表人类审核、正式产品放行或素材分发授权。仅临时目录写入；实际工程未修改。

## 小鹤：可实现纯双拼

主来源为方案作者当前官网，不取其他输入法核心或词库：

- https://flypy.cc/help/up.md ：完整音节是声母键+韵母键；零声母规则与文本口诀。
- https://flypy.cc/images/hejp.png ：声母及韵母键位图，已实际查看。
- https://flypy.cc/images/uplu.png ：零声母图，已实际查看。
- 导航证据：https://flypy.cc/help/_sidebar.md，首页https://flypy.cc/。

图文原件保存在`primary-rules`，URL、HTTP响应、时间、SHA见对应acquisition/identity文件。不将原图、官网全文、音形编码、词库或安装包转入生产包。项目只独立实现公开功能事实键位；官网未授予图文或码表的开放再分发授权，不将读图当作这种授权。公开方案名称用于识别兼容规则，不表示作者背书。

编码事实提案在`xiaohe.rs`。布局版本`xiaohe-flypy-up-2026-09-27-v1`绑定读取到的官方规则，构建时静态编译，不在输入路径读文件。

零声母：a/ e/ o重复为aa/ee/oo；ai/an/ao/ei/en/er/ou保持原样；ang为ah，eng为eg。其他合法规范音节按声母和韵母分别映射：zh/ch/sh为v/i/u，其余单字母声母不变；韵母表见静态事实表。`v`表示n/l后的ü，`ve`/`ue`同在t；ju/qu/xu/yu按现有规范拼音u处理，不擅自增加jv等别名。

每个完整音节固定两键。单键仅作尚未完成编码的前缀，不是“小鹤完整单键音节”。输入层应保留所有共享同一前缀的合法音节，不用全拼分段猜测双拼；一个双键不存在合法音节时不能逐字符回退成两个全拼。撇号可由Cloudleaf作为显式音节边界，不能穿过双键中间；大小写和原始字节偏移由统一入口处理。单词/整句仍进入现有词条格，而非复制中文词典。

现有Cloudleaf规范音节409个，实际Rust编译运行全部映射到两个ASCII字母；唯一完整编码碰撞为`lo`对应`lo`与`luo`，反向映射必须保留两者。两个独立例子/键表测试通过，原始输出`verify-*.stdout/stderr`、`syllable-verification.json`，不是语言准确率。

与快捷键：此纯方案仅a-z，不需要分号、斜线或系统键，平台修饰键仍由既有层过滤；不得截走Ctrl/Alt快捷键。

## 微软：本轮规则未闭合

已读Microsoft官方支持文档：
https://support.microsoft.com/zh-cn/windows/hardware/input-devices/microsoft-simplified-chinese-ime

该页确认双拼输入模式及相关快捷键，未给出完整键位表、零声母及所有特殊音节规则。不可依赖第三方博客或凭记忆导入完整微软键位。当前结论`not implemented / primary-rules-incomplete`；不要建立同名但猜测映射的方案。官网原件已保存在旧acquisition.json对应文件。没有通过UI打开系统设置或修改输入法配置。

## 自然码：本轮规则未闭合

原方案官网https://www.zrm.com.cn/当前web工具无法访问。官方实现者Sun/Oracle文档可读：
https://docs.oracle.com/cd/E19253-01/819-0398/819-0398.pdf
《简体中文Solaris用户指南》，2005年1月，表4-3，印刷页56–57。

它提供一张自然码键位表，但f行印为zh，且未给出可独立闭合的自然码零声母规则。这个具体冲突不能自行改成“en”后声称已由原件验证。较新社区实现还明确区分老自然码ü打法，说明仅同名不保证全部语义一致。当前结论`not implemented / primary-rules-incomplete`。不复制Sun受限制文档或其他输入法源码。保留URL和冲突事实即可；后续取得可固定的方案作者原件再实现独立版本。

## 自主实现边界

`xiaohe.rs`不包含语言候选、词条/词频、搜索剪枝或其他输入法业务。它只将已在统一入口确认合法的规范拼音音节映射为两键；模糊音、纠错、简拼均由Cloudleaf自己的统一解释层另行配置，不能混入键位方案。
