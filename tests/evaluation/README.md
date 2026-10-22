# 算法独立诊断种子集 v1

本目录属于 CL-005，先约定数据和指标，再实现评估器。数据由项目手工编写，按根目录 MIT 许可提供；没有导入真实用户输入或第三方语料。它是诊断种子集，不是正式大规模语言准确率评估，也不是生产词库。

## 数据与隔离

`algorithm-v1.tsv` 使用 UTF-8、LF、无 BOM。首行为 `# cloudleaf-algorithm-evaluation-v1`，次行为九个以制表符分隔的字段：

`id split category group input context gold correction note`

- `split` 仅为 `dev` 或 `heldout`；本轮没有 train，dev 也不用于调参。
- `group` 表示同目标词或同输入家族，组不得跨 split；ID 以及 `(input.lower(), context, sorted(gold))` 必须唯一。
- `input` 是完整待评估输入，不是事件脚本；`context` 为独立的意图提示，`-` 表示没有。当前两个算法均忽略 context，报告必须保留这项能力缺口。
- `gold` 可用 `|` 列出多个可接受文本，至少一个，不能用空值掩盖未实现的能力。
- `correction` 为 `clean` 或 `required`。required 是人工构造的拼写错误；当前算法都不产生显式纠错预测，纠错 precision 因零预测为 null，recall 为 0/required。
- 各字段不允许制表符、换行或控制字符；`-` 只用于 context，没有隐式填补。

历史 `tests/fixtures/input-v1.tsv` 的所有 `text:` 输入串、非空预编辑以及完整事件串均进入排除清单；大小写变化不能绕过隔离。不能把旧测试换 ID 后称作 heldout。固定词典里的目标词可以作为新输入/新上下文的 gold，因为它是本轮开始前已固化的只读资源，不能为了提高 heldout 成绩改变词条或词频。

该隔离只防止明确的历史事件复用和本集分组混入，不证明手写 seed 独立于开发者知识。未来真实评估必须扩大来源/规模并保留独立维护的盲测集，记录许可、抽样、去重和 train/dev/eval 分割；首次查看本 heldout 后不得再用它选择算法权重并继续称盲测。

## 比较范围

`production-rules-v1` 调用现有 `parse` 和 `candidate::generate(..., 50)`；`frequency-only-ablation-v1` 对相同解析路径的 `query_limited(..., 50)` 结果合并后，按频率降序、文本字数降序、ID 升序排序，按文本去重取 50。它只移除 Exact 优先级，是排序消融，不是新候选格、语言模型或完整候选算法。

每个样例和算法预热 20 次，正式计时 100 次；每次测量包括解析、查询、排序和结果向量，排除文件读取和输出。两种算法均物化相同的 Candidate 字段再提取文本，避免字段复制量不同干扰排序消融的计时解释。每轮候选/错误结果必须与该样例第一次结果完全一致，否则评估失败。结果记录 P50/P95/P99/max、样本数、全部候选及最佳可接受 gold 的 1 基排名，未命中 rank 为 0。

## 分母和报告

- Top-1、Top-5 与 MRR 的分母始终为所选分组全部样例，OOV、无候选、错误/超长、当前不支持的混合/上下文/纠错/扩展输入不得被删除。
- `lexicon_known` 仅表示至少一个完整 gold 文本存在于固定原型词典中，与本次输入能否检索到它分开。单独报告该子集，不能替代全体指标。
- `candidate_availability` 是有非空候选的样例数/全部样例；`gold_coverage` 是任一 gold 进入完整有界候选集的样例数/全部样例。gold覆盖率@K则明确为gold进入前K项的比例，不能与非空率混称。
- 每个 split/category/group 统计数量、命中、Top-1/Top-5、MRR、覆盖、纠错与延迟分位数。样例级差异列出两算法候选、rank、排名变化与新增/移除候选，不能只给总体均值。
- 当前纠错能力为 false，零纠错预测 precision 为 null，required>0 时 recall=0；不是“算法判断全部不需纠错”。context ignored、数据微型且手工等限制进入 JSON/Markdown。

运行：`python scripts/evaluate_algorithms.py --output-dir docs/reports/algorithm-v1`。脚本先执行固定 workspace 的离线 release 构建，从 Cargo JSON 的 `compiler-artifact.executable` 取得实际产物路径，记录数据、源码、二进制指纹、工具链、Git 提交、环境与逐样例结果。结果只是本 seed 上的真实测量，不能据此宣称排序成熟或消融方案总体更优。
