# Round 29 词典运行时

## 资源与状态

设置中心沿用已共享的设置目录。资源配置用独立 revision 保存每个槽的当前与上一版，保存前检查选入文件的格式和全量 SHA；宿主随后重新验证字节并准备索引。保存、后台准备、宿主应用和拒绝各有状态。多个宿主显示各自的期望修订、活动修订、词典与模型身份和用户代次。

| 运行资源 | 条数或状态 | SHA-256 |
| --- | --- | --- |
| CC-CEDICT 固定基础层 | 123,544 词条，固定来源获批 | `03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431` |
| Unihan 17.0.0 基础备选 | 44,431 字符读音记录，固定来源获批 | `1b13b199440c0ca9b59074de9100f58c6a0b847b12a7b986e1b752084ff60f8e` |
| 固定传统字形补充层 | 3,810 条，ResearchOnly，默认关闭 | `edcb27080fac434637c83985eebd4c4f62d12e09943e4278a69fdc000b01e430` |
| CC-CEDICT 与补充层同用 | 127,354 行，逐行多集与历史合并资源相同 | `b52ca157fa3d98cd0281f1d1d016077754f11aac07a878dd76f7444fb04e2f46` |
| Unihan 与补充层同用 | 两个独立无窗口进程收敛到同一快照 | `3aa1b34419145962c169c3edc43dc6badf2668e9bc37b91e1815e36fa2af13ee` |
| 原字典的 lexical 研究模型 | 切到不兼容字典时使用结构评分。回到原字典后恢复精确绑定 | `36dc4d9057bc70e3494fe397da09e8ab6f187c435bd735b66a7734f9d414fce5` |

补充层准入来自固定资源登记，须显式启用。相同稳定 ID 的同事实合并，不同来源与合法多音保留，候选正文由现有搜索去重。同 ID 冲突会拒绝准备。用户词库保持独立存储，候选仍使用现有用户层，旧选择凭据随修订失效。

## 本地证据

资源中立层 all-targets 专项通过 76 项，条件忽略 1 项。固定真实数据专项单独执行 1 项并通过，覆盖只加载基础、选入但关闭补充、启用叠加、基础复原、补充关闭与回退、保存后损坏以及精确模型恢复。原始日志位于 `.local/round21-runs/round29-resources-final` 与 `.local/round21-runs/round29-resources-real-original`。补充层原件已经包含 schema 2 头，第一次由派生文件重复加头导致验证失败。原日志 `.local/round21-runs/round29-resources-real-fixed` 继续保留。两个损坏选择在旧逻辑下可推进配置并损失有效回退，复现证据位于 `.local/round21-runs/round29-resources-repeated-invalid-red`。

### 两个独立 Windows 宿主进程

真实数据专项由一个测试进程启动两个无窗口宿主，共享临时配置目录。每个进程分别读取资源与发布回执，测试要求两条回执拥有不同 PID，相同生效修订上的完整快照 SHA-256 相同。

| 资源修订 | 已观察结果 |
| --- | --- |
| 1–2 | 启动使用 CC-CEDICT 及绑定的 lexical 研发模型。选入补充文件后继续保持关闭 |
| 3 | 空闲宿主先启用补充层。编辑宿主报告 `Deferred`，原组合、光标与候选帧保持原样。随后注入 `Unknown`，记录保留且文档为空。明确恢复后两宿主应用同一快照 |
| 4–7 | 切换基础层到 Unihan，再回退至 CC-CEDICT。保持补充槽独立，随后关闭与重新启用。回到原单基础层时恢复原模型 |
| 8 | 已载入的临时补充文件损坏。两进程均拒绝新配置并保留修订 7 的旧快照 |
| 9–10 | 回退补充槽至关闭，继续使用 Unihan。再独立回退基础槽至启动的 CC-CEDICT，并恢复原模型。两宿主正常退出 |

另外三项专项分别验证切换使旧候选及提交凭据失效、私有词库重新挂载保留磁盘 generation 与既有学习数据、活动资源变更清除面板旧释义和旧译文。仅收到待应用回执时，面板保留当前显示和原文。

### 初次失败与修复

- 真实补充层每边的来源身份长度可达 141 字节，超出旧启动词典的诊断帧预留。首轮同一资源修订的空闲宿主报告 `Rejected`，编辑宿主正确报告 `Deferred`。托管模式为登记资源预留每输入字母 8 字节正文及每条边 160 字节来源身份，并和实际启动预留取较大值。两侧引擎都走已有宿主容量检查，进程 128 MiB 上限沿用。
- 回退到启动基础层之后，旧逻辑再次选入 Unihan 时没有把前一状态 `None` 写入回退槽，导致两槽都指向 Unihan。现在启动状态可以成为上一版本。损坏的磁盘资源继续跳过，保留较早的可验证回退目标。

下列 `result.json` 记录各次命令、时长、抓取时的设计提交 `17482208629d9947ef157b1098fd22297de49942`、实际工作树源文件和两路日志 SHA-256。最终分支验收记录另见 Round 29 汇总与证据索引。

| 本地目录（均在 `.local/round21-runs/`） | 退出码 | `result.json` SHA-256 |
| --- | ---: | --- |
| `round29-host-real-original-layer` | 101 | `f47387fd3dcc9e26ea2cc1d0700c0d281b14c5124010a1a97bc60591232ec45d` |
| `round29-host-managed-reserve-real` | 101 | `d0e4118d0f10e4d4c9863ae8b94da2138ebb4287b16de8cd8cd0e3fb45661ffa` |
| `round29-host-rollback-retest` | 0 | `60240f74f4b7895e54a9ecf2239bc80dd7006b41793ed2b635293e3462b32acd` |
| `round29-resources-startup-rollback` | 0 | `3e964a3d835cd22ff8a4ebcbdcf778b5a0aa624a678b88ae18689c2fc238f8d0` |
| `round29-host-transaction-final` | 0 | `f190ce15223805fdbae2d6d02f4cf7ee6e4b0cf91390413c4576d7a620a59ffb` |
| `round29-host-user-rebase-final` | 0 | `5464a8cb3e635b7d61b362512987c9a99d8fb715ff9317de762ebcd6aed2ad7b` |
| `round29-panel-resource-final` | 0 | `34ad7d63105f1b23ab3ab2589d4820b9b8abb594c46ccd6df639141fd1fb7b14` |

### 精确实现提交复验

在实现提交 `983a9be991b5e1de2a8d87d12fde8c98f5884538` 上复跑了固定资源中立层和真实 Windows 双宿主专项，两项均返回 0。双宿主由同一父测试驱动，记录修订 1–7、9–10 的共同快照。修订 8 按用例预期拒绝，继续提供旧快照。收敛后的修订 10 重新绑定启动时的 CC-CEDICT 与同版 lexical 模型。正常退出后的真实应用与 TSF 验收仍各有独立门禁。

| 本地目录（均在 `.local/round21-runs/`） | 退出码 | `result.json` SHA-256 |
| --- | ---: | --- |
| `round29-resources-exact` | 0 | `3d0cc7e4e0533ee909155a6391efeafbacb089d4404637730a2cff8bdf653675` |
| `round29-host-exact` | 0 | `a9abaf1934aa851d55c281a1d80a2e8b2e9fc0b225f17d8267b119c6e6196e6d` |

`round29-host-exact/stdout.txt` SHA-256 为 `c60491618180c4e1662df6f1a7664b3df6ff9b38e46e7176bbf1e5b5b98458de`。其中三个成功摘要分别来自父测试及两个子宿主，合计为一个双宿主专项用例。

### 重放

在整合工作区将 `$base`、`$unihan`、`$supplement`、`$model` 设置为上表对应原件的绝对路径。补充层直接使用 Round 17 与该 SHA 对应的 `layer.tsv`。运行会在系统临时目录创建测试配置，以及仅由测试写入的补充文件副本。

```powershell
$env:CLOUDLEAF_R29_BASE = $env:CLOUDLEAF_ROUND29_BASE = $base
$env:CLOUDLEAF_R29_ALTERNATE = $env:CLOUDLEAF_ROUND29_UNIHAN = $unihan
$env:CLOUDLEAF_R29_SUPPLEMENT = $env:CLOUDLEAF_ROUND29_SUPPLEMENT = $supplement
$env:CLOUDLEAF_R29_MODEL = $env:CLOUDLEAF_ROUND29_MODEL = $model
python -X utf8 .local/run21.py round29-host-release-recheck cargo test -p cloudleaf-windows-ime --lib --locked --offline real_dictionary_lifecycle_two_owned_processes -- --ignored --nocapture
python -X utf8 .local/run21.py round29-resources-release-recheck cargo test -p cloudleaf-settings --test round29_resources --locked --offline fixed_dictionary_layers_and_model_restore -- --ignored --exact
```

`config/language-resources.json` 固定运行文件、来源锁及许可证，登记文件 SHA-256 为 `293d788d9d6897ec6ffac6e1117448b5aeabb4084d1e3293b28d73f4c5e72d51`。CC-CEDICT 原件及派生记录维护 `CC-BY-SA-4.0`，Unihan 维护 `Unicode-3.0`。许可限于对应范围。正式词频保持 `unknown`，正式语言质量评估、桌面交互和 TSF 准入各按原门禁登记。
