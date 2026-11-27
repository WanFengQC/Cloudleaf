# Round 28 设置运行时

共享设置中心在后台写入 `input-settings.json`。同一目录内的宿主独立订阅原子写入结果，只有宿主回执能确认实际应用。保存方持有短期文件锁，多个设置进程只能有一个提交相同的前置 revision。旧有效配置继续备份，设置读写不进入按键线程。

隔离输入宿主在双核心都空闲时同步应用方案、模糊、纠错、混排、页大小和候选总数。真正的组合、游标、未确认的提交继续由原上下文处理。发生 Unknown 时设置保持待应用，直到原上下文经明确核对后结束。新配置在可应用时推进输入修订号，旧翻译和候选随修订失效。预检或磁盘校验失败保留已应用配置和词典。各宿主回执独立标明活动及待应用版本、生效代次、词典快照和原因。宿主崩溃留下的回执经过心跳有效期后不再视为在线。共享档案由设置中心统一写入，未绑定共享档案的独立研发宿主保留本地 F6/F7/F8 调试快捷键。

## 验证

专项运行均以工作区 HEAD `938835f7c6a4e8bf69c1425b096450d89bb54dc9` 为基底。每次实际源码的 SHA-256、命令、用时和返回码写入各自的 `result.json`。源码整合后的完整回归在本轮汇总单独登记。

| 运行 | 返回码 | 耗时 | 结果 |
| --- | ---: | ---: | --- |
| 锁更新前构建 | 101 | 0.359 s | 共享 workspace 依赖变化后锁文件尚未更新 |
| 双核心预检首次编译 | 101 | 34.437 s | E0500，已修正待应用配置的 Rust 借用冲突 |
| Windows 宿主和设置包专项 | 0 | 83.359 s | 139 通过、3 个手动诊断保留 ignored |
| 最新双核心热应用专项 | 0 | 9.531 s | 4 通过，包括磁盘损坏回滚和跨修订保全 |

原始日志和运行清单位于下表目录内，分别为 `stdout.txt`、`stderr.txt` 和 `result.json`。实际命令依次为两次 `cargo check -p cloudleaf-settings -p cloudleaf-windows-ime --all-targets --locked --offline`、`cargo test -p cloudleaf-settings -p cloudleaf-windows-ime --all-targets --locked --offline`，及 `cargo test -p cloudleaf-windows-ime --lib --locked --offline hot_settings`。

| 本地证据目录 | stdout SHA-256 | stderr SHA-256 | 逐文件源码清单 SHA-256 |
| --- | --- | --- | --- |
| `.local/round28-settings-evidence/r28-settings-first` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `a0a91a8fd228f11f8b48556452172c6f05a0fa9f65daeaae7a09c9f4a350f93b` | `ad888132422d767f521c92a7d218ad7cdea263fd8291194033f2395e4504d0e5` |
| `.local/round28-settings-evidence/r28-settings-check` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `853cceac52f822d98994e478905bfa8108d05f4c76634c94e6c2ba923c2e4d91` | `c9d3615b8c065ba73adbac39126ba6be949122097405ce6b686c9a9131c0bcc6` |
| `.local/round28-settings-evidence/r28-settings-tests` | `ce72ca93aec8540f5ea97dcdfea0adf0012edae989dd81f140165e7980fd6a8a` | `a9f4fd57d378fb96417a6034fd935609dd3405c2185b891908d107b385a303e8` | `62a916f13583a074109b3ba687f8439c241f7f539d069ecb8cd9dee1c7f584de` |
| `.local/round28-settings-evidence/r28-settings-hot-final` | `4980e91dc36adfeb9371821508e4cf57ed5a5f191da00927b5c4213833739024` | `67702b731ea6e749459af4918fe3330f8b5cd2c0ca9f4bcb92f2a0282c1afebb` | `1a94c3f041a8aea305777f9f7907dc8fe8d8fcf7073bc3eedb5f4bba42b0c0a3` |

跨进程用例由测试二进制真实创建额外进程保存设置，两个监听器分别产生 Applied 和 Deferred 回执，后续验证每份生命周期独立。竞争写入只产生一次 revision 2。Windows 热应用专项覆盖组合中原拼音、光标及候选不变，应用后旧翻译不可提交，两个核心候选上限实际变为 4，以及主配置损坏后已生效代次不回退。

## 状态与范围

- `done` · 隔离普通窗口的检查、验证、内存双阶段热应用及每宿主回执。
- `done` · 后台原子保存、跨进程通知与同修订竞争校验。
- `partial` · 双宿主同时编辑时会暂时运行不同的已应用 revision；回执明确展示等候状态，只有安全收尾后才趋同。
- `not-run/user-paused` · 真实 Windows 桌面与用户应用中的交互验收。
- `blocked` · 系统 TSF 宿主的热应用验收仍依赖单独可靠性和授权门禁。
