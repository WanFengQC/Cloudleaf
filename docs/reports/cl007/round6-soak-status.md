# CL-007 Round 6：可中断的长期机制测试与TSF准入阻塞

2026-10-28。方向起点`7f581dfca43515105fca918ffd79e55f315f7729`；实际被测实现`271704e2db80f556fec477e4523e71456e42c632`。本轮修复工程诊断的实际证据丢失问题，生产COM释放、输入所有权和预算语义没有放宽。**8h/24h档已可配置，但未执行；真实TSF与应用准入仍阻断。没有注册、激活、安装或发布系统输入法，不进入CL-010。**

## 实际审阅与修改范围

已读取AGENTS、roadmap、product-scope、Round 5设计与完整报告，并审阅实际`service/session/watchdog/owner_cleanup/recovery/ownership/budget`生产链。服务Factory/Service/LockServer和Lease负责模块保活；Host/Rc/COM只在owner STA使用；退出证明基于真实复制线程HANDLE，不能按线程ID或Rust is_finished猜测。watchdog任务创建worker前全局持有JoinHandle，只有构造结束、退休请求、独占状态、真实OS线程退出才join/退slot。recovery只有owner或已证明退出的纯值资源具备清理权限；monitor completion、窗口/类注销和账目分开核对。

`DllCanUnloadNow`只做零等待回收请求；模块OBJECTS/LOCKS非零仍S_FALSE。Unknown材料不因超时、显式放弃或新会话变成可重试NotApplied；旧instance/epoch/sequence与迟到回调继续隔离。未析构Host、外部COM强引用、永久卡住或无法证明安全的资源仍可能耗尽额度。这是未关闭的部署级可靠性限制，不能声称已无限恢复；不能跨线程Release或杀死真实用户应用来回收。

本轮独立可复现的问题为：旧`stress_windows_faults.py`只支持180–600秒，并把事件/采样保存在控制器内存，退出时才写summary；控制器中断可能丢失整轮资源证据，直接延长24小时也会累积事件。先行设计`0f204c1`，测试提交`f5ff615`，实现才为`271704e`。

|修改文件|目的|
|---|---|
|`scripts/stress_windows_faults.py`|兼容原有有界档，新增严格8h/24h及显式短诊断档；流式原始事件/资源、周期fsync检查点、精确进程身份、独立新段续跑与失败保全|
|`tests/test_windows_stress.py`|五项旧实现真实失败的配置/原文/分位数/进程身份回归，实际Windows句柄测试不使用裸PID猜测|
|`adapters/windows/render_tests.rs`|仅工程host扩展profile和低速周期，保留所有R5断言；记录自编事件的信封、确认时刻、预算和诊断计数|
|`adapters/windows/service.rs`|仅`cfg(test)`只读模块OBJECTS/LOCKS计数；生产释放和模块保活逻辑未变|
|`docs/cl007-round6-isolation-and-soak.md`、README、本报告和`round6-soak/`|进程/会话边界、准入门禁、真实失败、全部命令与原件身份|

没有改共享接口、核心算法、原64条词典、41事件语料、83诊断种子或原总预算。242项起点历史报告未改，所有旧测试保留。

## 红测、完整回归与固定身份

`f5ff615`的五项测试在旧runner实际报缺少API，原文见[Python红测](round6-soak/red/python-red.raw.json)。实际旧Rust host还在`diagnostic=1`触发原180..600断言，子进程exit101/runner exit1；[原始host输出](round6-soak/red/host-unsupported/segment-0001/stdout.raw.json)和[身份](round6-soak/red/host-red-identity.json)保留。此时Rust host可绑定f5ff615，runner为未提交草稿，全文/工具SHA单独保存，不把混合工作态伪装干净提交。

最终实现下[10项README检查](round6-soak/summary.json)全部exit0：locked fetch/offline debug与release、fmt、所有target Clippy `-D warnings`、157 Rust（其中97原生，另2项默认ignored诊断已显式执行）、doc、资源许可审计、21 Python及精确Windows依赖/RustSec审计。[9项release专项](round6-soak/diagnostics.json)全部通过：真实DLL loader、模拟COM基准、共享预算压力、恢复故障矩阵、原R3/R4/R5原生专项、1000次原型及83诊断种子。完整原始命令/耗时/输出和取证脚本见[运行器](round6-soak/runner-sources.json)。

|身份|SHA-256|
|---|---|
|实现源码`cloudleaf-source-v3`|`ad8760e1fc7dc56f9af924c37095bf61aa137852462768a560a7fa15e82bf193`|
|压力控制器工具|`03bf9ecc4359b265ae42d498197bfd6d9358ded091cffb0002993da4adea2b35`|
|实际Cargo release测试exe|`c7fe4b34c197871bb8d02acb37a005f93ac658800712a015a544312a7e54a0b6`|
|真实release DLL，599,552字节|`204fbb8ee725562a449ba6bcac730ab113944350e92c7a2d6ddd97fd6e73b552`|
|原型数据|`d4a87e958b23097253a4601b9ae373f02964e3e37729bb7f560e20835d5bb5f0`|

各段manifest保存精确实现/源码/工具/二进制/数据/配置/OS身份，后续报告提交不改绑被测版本。依赖锁和审计工具SHA及数据库版本见[审计原件](round6-soak/dependency-audit.json)。只能说明该快照的审计结果，不能替代unsafe或真实应用验证。

## 实际短档与失败保全

实际四次调用固定`diagnostic`、4秒、50ms周期、50ms采样、100ms检查点；每次自有子进程先制造32个写后ExitThread owner，健康会话通过生产原生COM路径执行自编六键输入。Windows11 build26300/i5-11400/12逻辑处理器，桌面负载未完全控制。采样枚举有开销，不是精确20Hz。

|调用|实际结果|原始证据与分母|
|---|---|---|
|完整短档|runner/child均0，80周期/480写，32个Unknown逐项验证且不自动重放|[562事件/32资源样本](round6-soak/complete/segment-0001/summary.json)，机制通过、长期通过false|
|2.5秒受控中断|runner130，child被自己的持有句柄终止；失败/不完整|[176事件/16资源样本](round6-soak/resumed/segment-0001/summary.json)，150条逐键延迟保留；未到尾部恢复核对，成功率null|
|相同身份续跑|OS证明原runner/child已结束，新segment、独立会话，80周期/480写|[562事件/37样本](round6-soak/resumed/segment-0002/summary.json)，不与前段拼接成连续时长|
|第3周期显式失败|child101/runner1，符合预期，机制未通过|[22事件/7样本](round6-soak/injected-failure/segment-0001/summary.json)，18条逐键延迟与panic全文保留；尾部恢复成功率null|

完整短档480个逐键样本的admission P50/P95/P99为36,600/199,400/1,408,700ns；到该周期drain末的确认延迟为231,300/661,400/2,474,900ns。**admission计时包含native调用之后的诊断snapshot克隆取证，确认时刻是整周期drain末，不是单键真实TSF延迟。** 原始事件保留实际instance/epoch/sequence/VK/LPARAM及两个时长；最终分位数读取并排序延迟数组，末处理不是恒定内存。长期运行期间不积累全事件，但末处理最多约24h×6×2个数字；没有将其声称为恒定资源算法。

完整短档采样峰句柄146、线程7、working set15,605,760B、private commit6,144,000B，OS峰working set另记录为16,199,680B。采样峰不等于精确物理内存峰，private commit不是RSS，4秒不能得出长期趋势。每周期固定计费hosts33、batches32、fixed118,958,850B、text41,943,040B相同；末尾健康Host释放后旧Host/epoch各32且S_FALSE。Host Rc强引用1、模块Lease132/Lock0、自有IUnknown AddRef/Release返回4/3分别标名，不当作真实全局COM引用数，外部COM总数null。两个完整短段的尾部32/32验证成功率1.0只属于指定机制；两失败/中断段保持null，没有把未执行分母计通过。

原始stdout、events/samples/checkpoints JSONL持续落盘；检查点先fsync原件，再同目录原子替换最新checkpoint，历史检查点仍在JSONL。resume要求全部内容身份一致，并用OS创建时间和真实句柄证明前一runner/child退出；存活/未知拒绝，不按PID杀进程。恢复启动新segment，不重放旧Unknown。断电持久性、磁盘永久写失败后的完整证据及任意进程崩溃后的文档恢复不在短档证明范围。

原始UTF-8日志可从`.raw.json`的raw_text编码恢复；发生CRLF或BOM规范化的结构化原件另有`.original.raw.json`。展示件、原件和Git发布字节分别核SHA，[校验表](round6-soak/checksums.json)不包含自身。不能把显示件的换行SHA当原始SHA。

## 8h/24h重放与真实TSF准入

通过Cargo `test --release --no-run --message-format=json`取得实际测试exe后，在新目录显式运行：

```text
python scripts/stress_windows_faults.py --test-binary <实际release测试exe> --profile 8h --sample-seconds 1 --checkpoint-seconds 60 --output-dir <全新目录>
python scripts/stress_windows_faults.py --test-binary <实际release测试exe> --profile 24h --sample-seconds 1 --checkpoint-seconds 60 --output-dir <另一全新目录>
```

长期档分别严格28800/86400秒，默认每秒一个六键周期，只能选1000..60000ms周期；不能用`--seconds 4`缩短后保留长期标签。短档注入参数只允许diagnostic。`--resume`必须原manifest配置完全相同，只创建新段；某一段实际独立满足完整时长且成功才有continuous_long_profile_completed=true。**本轮没有启动或完成8h/24h，长期门禁未通过。**

受阻注册方案为[先行准入设计](../../cl007-round6-isolation-and-soak.md)的下一阶段草案：CLSID`f573e0d6-2c67-4fd8-a75a-7a58278dc1b0`、profile`d845b8e7-f6c2-4d28-9bc1-cac205f9f62b`、语言0x0804来自实际源码；不表示已注册。需要先确定独立VM/测试用户、快照和负责人员、位数/签名/实际DLL SHA、注册前状态备份、撤销/恢复演练、异常Host部署隔离策略及连续8h/24h证据，然后提交具体执行方案等待人工批准。当前这些条件未满足，不运行管理工具的注册/启用/安装命令。

真实应用矩阵仍待在获批隔离环境执行：记事本、浏览器表单、办公软件、多行编辑器；逐例记录系统/应用/词库/二进制身份、组合/选词/退格/取消/失焦、只读/密码、同步/异步拒锁、重入、重复/迟到回调、Unknown实际文档结果与可逆撤销。现有COM工程host和加载器不是该验收工具链完成或真实应用通过证据。

|状态|退出条件|
|---|---|
|已通过|指定runner证据丢失缺口修复、短档流式证据/身份/新段续跑/失败取证、全部旧回归与原release专项|
|部分完成|进程/会话隔离边界及8h/24h工具可运行；没有完成长期时长或生产部署级隔离|
|阻断|永久Host/COM引用/锁无法安全回收、额度耗尽后的部署恢复策略、实际连续8h和24h及真实应用准入材料|
|未执行|系统注册/激活/安装/发布、真实Windows应用注键/长期输入、物理断电和真实宿主恢复|
|不放行|CL-007未达到正式退出条件；PR #4保持Draft，不合main，不进入CL-010|

最新最终交付SHA的Windows/Linux push CI由共享Round 6汇总单独核对，本地成功、旧CI或PR合并快照不能代替新head验收。
