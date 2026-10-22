# M2 候选排序回归报告

日期：2026-10-22（Asia/Shanghai）。数据：prototype-v1，64 条项目手写词条。语料：input-v1，41 条。

这是首次建立的排序基线，没有历史版本可比较。以下为测试宿主逐条执行并对预编辑、完整候选顺序、高亮、提交文本和最终状态断言通过后的输出。候选包括当前帧全部候选，编号为全局零基索引；不同页的编号仍由宿主按每页 5 条显示。空值记为 `-`。

命令：`cargo test -p cloudleaf-input --test corpus --locked --offline -- --nocapture`。

| 用例 | 预编辑 | 实际候选顺序 | 高亮 | 实际提交 | 最终状态 | 验收 |
|---|---|---|---:|---|---|---|
| common-word | nihao | 你好 | 0 | - | CandidatesVisible | 通过 |
| country | zhongguo | 中国 | 0 | - | CandidatesVisible | 通过 |
| world | shijie | 世界 | 0 | - | CandidatesVisible | 通过 |
| brand | yunjian | 云笺 | 0 | - | CandidatesVisible | 通过 |
| reference-annotation-not-committed | - | - | - | 参考 | Idle | 通过 |
| umlaut-lv | lv | 绿 → 旅 | 0 | - | CandidatesVisible | 通过 |
| umlaut-nv | nv | 女 | 0 | - | CandidatesVisible | 通过 |
| ambiguous-xian | xian | 西安 → 先 → 线 → 县 → 现 | 0 | - | CandidatesVisible | 通过 |
| explicit-separator | xi'an | 西安 | 0 | - | CandidatesVisible | 通过 |
| partial-last-syllable | zhongg | 中国 | 0 | - | CandidatesVisible | 通过 |
| trailing-separator | xi' | - | - | - | Composing | 通过 |
| invalid-pinyin-preserved | zzzq | - | - | - | Composing | 通过 |
| no-entry-fallback-preserves-case | - | - | - | Zzzq | Idle | 通过 |
| backspace-to-empty | - | - | - | - | Idle | 通过 |
| backspace-refresh | nihao | 你好 | 0 | - | CandidatesVisible | 通过 |
| cursor-insert | nihao | 你好 | 0 | - | CandidatesVisible | 通过 |
| cursor-backspace | nhao | - | - | - | Composing | 通过 |
| selection-down | ni | 你 → 尼 → 呢 → 泥 → 拟 → 逆 → 倪 → 匿 → 腻 → 霓 → 昵 → 溺 → 您 → 宁 → 凝 | 1 | - | CandidatesVisible | 通过 |
| selection-up | ni | 你 → 尼 → 呢 → 泥 → 拟 → 逆 → 倪 → 匿 → 腻 → 霓 → 昵 → 溺 → 您 → 宁 → 凝 | 1 | - | CandidatesVisible | 通过 |
| second-page | ni | 你 → 尼 → 呢 → 泥 → 拟 → 逆 → 倪 → 匿 → 腻 → 霓 → 昵 → 溺 → 您 → 宁 → 凝 | 5 | - | CandidatesVisible | 通过 |
| third-page | ni | 你 → 尼 → 呢 → 泥 → 拟 → 逆 → 倪 → 匿 → 腻 → 霓 → 昵 → 溺 → 您 → 宁 → 凝 | 10 | - | CandidatesVisible | 通过 |
| previous-page | ni | 你 → 尼 → 呢 → 泥 → 拟 → 逆 → 倪 → 匿 → 腻 → 霓 → 昵 → 溺 → 您 → 宁 → 凝 | 0 | - | CandidatesVisible | 通过 |
| page-local-number-selection | - | - | - | 倪 | Idle | 通过 |
| unavailable-page-number | - | - | - | ni9 | Idle | 通过 |
| zero-is-normal-digit | - | - | - | ni0 | Idle | 通过 |
| enter-no-duplicate | - | - | - | 你好 | Idle | 通过 |
| space-no-duplicate | - | - | - | 你好 | Idle | 通过 |
| number-selection | - | - | - | 尼 | Idle | 通过 |
| candidate-with-punctuation | - | - | - | 你好， | Idle | 通过 |
| unknown-unicode-preserves-raw | - | - | - | ni🙂 | Idle | 通过 |
| idle-chinese-punctuation | - | - | - | 。 | Idle | 通过 |
| english-passes-through | - | - | - | - | Idle | 通过 |
| mode-switch-cancels | - | - | - | - | Idle | 通过 |
| mode-resume | nihao | 你好 | 0 | - | CandidatesVisible | 通过 |
| escape-cancels | - | - | - | - | Idle | 通过 |
| focus-loss-cancels | - | - | - | - | Idle | 通过 |
| deactivation-clears-and-ignores-input | - | - | - | - | Inactive | 通过 |
| idle-system-keys | - | - | - | - | Idle | 通过 |
| cancel-then-compose | zhongguo | 中国 | 0 | - | CandidatesVisible | 通过 |
| consecutive-commits | - | - | - | 你好 / 中国 | Idle | 通过 |
| uppercase-raw-is-preserved | NiHao | 你好 | 0 | - | CandidatesVisible | 通过 |

41/41 固定语料通过。附加单元测试验证数据重排不改变候选顺序、完整匹配优先于更高词频的前缀匹配、频率平局按文本字数和稳定 ID 排序、跨分段去重以及有界 top-k。手写预期保存在 tests/fixtures/input-v1.tsv，不由候选算法生成。
