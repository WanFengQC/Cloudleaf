# Round 14：CL-009只读接入审查

2026-11-02。CL-009工作区HEAD为`3f26f34e00c0775124f87e150a021f99fb52c567`，工作区干净；与共享分支当前HEAD的merge-base为`9af9bc1b2ebacc3ad76124cfa39d09de8eb77bd1`。没有修改工作区、下载、提交或推送。

## 实际实现范围

CL-009的正式schema2数据reader/verifier/index/publisher主要是Python标准库模块`tools/data_pipeline/pipeline.py`，不是Rust正式词典reader。当前`crates/cloudleaf-dictionary`自M1基线未变，仍只读v1、16MiB/100000条、必填u32频率，不能读取正式23.6MB/123544条v2包。不要修改正式数据去迁就原型reader或把unknown补0。

CL-008已实现兼容v2的Rust `cloudleaf_search::Lexicon::from_tsv`与`from_tsv_with_limits`。默认128MiB/1000000条/4000000 trie节点，上界可由宿主显式设定；Entry保留稳定ID、pinyin、文本、Option<u32>、FrequencyKind、来源原始ID与许可。v2 unknown必须为空/None，频率特征只在Some时计入。正式包全部unknown，因此此特征不产生观测分数，但不能称为零频词。

C序列化`index.json`为by_pinyin/by_text映射到稳定词条ID；完整verifier会从records重算并逐项核它，同时重算runtime投影。B从runtime重新建立其独有字母trie与词条音节边界，尚无C JSON索引的Rust直接reader。可复用这个现有trie，不应说已直接消费C序列化索引。每次按键仅读已载入Lexicon，不运行Python或重新读取TSV。

## 已找到并实际完整重验的固定资源

目录：`C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a`。

当前CL-009 verifier完整通过，命令和工具SHA、9个实际文件的SHA/字节均见`fixed-release-review.json`；耗时15.856秒是离线完整验证，不是输入路径性能。manifest SHA `5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5`。runtime 23602173B SHA `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431`；index 13028916B SHA `982c7ae244497d7f3cd68165e17f383655fe4c98470be5503182b6387011012e`。

123544条接受、1468隔离（1467不支持读音、1预算），110045多字、13499单字、119781不同文本、85439不同拼音。词频与POS缺失均123544。原件ZIP现存`.local/round4-source-cedict/cccedict-20260604-mirror7019e1c.zip`，实际SHA `fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`。

注意`.local/data-store/current.json`仍指向旧Unihan包`98b881d...`，不是新CC-CEDICT包。不要将current当最新正式词典自动加载。早期round4-candidate-a虽然runtime相同，其manifest/来源锁证据版本不同，优先固定上述Round5已重验目录。

## 真实词条覆盖

实际包有：我wo、正在zheng zai、开发kai fa、中文zhong wen、输入法shu ru fa、今天jin tian、天气tian qi、这zhe/zhei、是shi、一个yi ge、比较bi jiao、复杂fu za、的de/di、问题wen ti、你好ni hao、中国zhong guo、长chang/zhang、西安xi an、先xian。

无“一款”“很好”“我爱你”整词，但一yi/款kuan/很hen/好hao/爱ai/你ni组件存在。因此可由现有词格组合完整覆盖目标句，但不证明能进入有限Top-K或合理排序。不同声调/姓名读音可能在无声调投影形成多个同文本ID，必须按路径正确性与确定性规则去重，不能随机择一并删原记录。完整ID和读音见两个JSON。

## 授权范围

固定CC-CEDICT镜像2026-06-04、mirror commit7019e1c，CC BY-SA4原文/署名/来源/修改声明已有审核登记。可以在批准范围使用词头、读音、原始gloss与合规派生数据；不等于正式语言准确率、现代频率或人工语义对齐。保持署名、许可链接、修改说明、ShareAlike。Unihan批准范围仍字符级；Wiktionary research_only、Google候选和语义sidecar继续维持原状态。

最少必要资源为既有完整九文件包（Git外固定缓存），以及仓库中对应`data/sources/locks/cccedict-20260604-mirror7019e1c.json`、`data/licenses/CC-BY-SA-4.0.txt`、`data/sources/evidence/cccedict-mirror-7019e1c/upstream-documents.json`、来源登记相关记录、`docs/lexicon-v2.md`。不要把source raw ZIP、完整大包或无关语义/频率候选都放进Git。

如果移植现有完整Python preflight，不要只复制pipeline.py：它依赖filesystem.py、recovery.py、cedict_source.py及word_source.py的格式分派，以及源锁/拼音表验证。可以按实际调用挑选模块，保留相应历史测试；运行时核心不调用这些模块。B已有实际v2输入/词格接口，C不需要为本轮重新实现词典算法。

## 版本、失败和缓存

现有C activate/rollback校验完整release，复制后重验、不可变内容版本目录、current/previous单指针原子替换；目标损坏/不兼容拒绝，current保持。rollback允许当前损坏，但验证previous；孤儿恢复只针对真实退出owner，保留用户词库与隔离材料。这些是离线发布器行为，尚无Rust输入引擎热重载或索引缓存实现，不能当作已经集成。

输入集成应把实际manifest SHA作为词典版本，后台一次验证/载入，候选与词格只共享同一只读快照；换版本时旧候选/缓存按配置revision失效。不得仅凭稳定entry ID跨版本复用旧分数，因为稳定ID并不包含所有可变来源释义/资源内容。加载失败保留原拼音与明确回退，不静默混入原型manual频率以制造预期首选。

本只读审查未复现新的C P0/P1。主要接入缺口是真实v2并未连接默认输入状态机、C JSON索引未被Rust直接读取、无生产现代词频/独立gold；这些不应阻止复用B词格做显式实验。未进行新语言准确率、Rust加载内存或每键性能测量。
