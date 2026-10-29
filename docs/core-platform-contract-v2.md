# 核心与平台契约 v2：本轮增量设计

设计先提交，后续实现状态见测试/提交记录；本文件不把计划接口冒充现有功能。v1身份、原生事务三态、队列/重入和隐私语义继续适用。

- 新增 `KeyDisposition::{PassThrough, Handle, Blocked(ErrorCode)}` 与 `InputEngine::key_disposition(Key)`。它只读状态，不调用handle/解析/查询，不输出帧、不分配候选；原生OnTestKeyDown使用该判定，执行阶段再派发同一个已归一事件。Ctrl/Alt/Win的未注册快捷键仍在平台归一前透传。
- 新增 `select_candidate(revision: u64, id: &str) -> EventResult`。验证当前修订、候选ID和可提交状态后走相同提交确认流程；过期/不存在分别返回StaleCandidate/UnknownCandidate且状态不变，不用模拟Down或平台直接拼接上屏。宿主必须额外验证session/context。
- 新增 `InputConfig` 和受校验构造：page_size为1..9，max_candidates为1..128（不超过现有候选库公开硬上限）。默认仍5/50，既有行为不变。输入长度仍受原型64ASCII保护，未通过算法评测前不改成无界。
- 公共 `CommitOutcome::{Applied,NotApplied,Unknown}` 表达宿主确定性。只有前两者可映射现有成功/失败事件；Unknown归平台隔离，不发送可重试失败。core仍保留pending，生命周期取消使迟到确认失效。
- 提供严格UTF-8字节偏移到UTF-16 code-unit偏移辅助：拒绝越界/非字符边界；当前ASCII预编辑可用，中文/代理对文本的长度不能直接用字节数。没有因此实现Unicode组合编辑或任意选择区。

只读判定必须与同状态下handle结果的handled/error语义一致；pending/失败普通按键Blocked，Escape可取消。长度/控制字符错误不能因预判而丢失原始事件。状态改变发生在执行阶段，旧预判不是永久接管许可。

共享类型不含HWND/COM指针，不读文件或网络。平台线程、edit session、原生键序号、事件队列及安装属于A。B的新候选图/模型和C的数据格式不改变此契约；CL-010前的引擎接入另经review。
