# 翻译子系统

本 crate 与实时输入核心单向隔离。注册、词典载入、SecretStore 访问和 HTTP 在设置或后台执行；输入线程只调用有界 `submit` / `poll`。没有默认在线服务，`ServiceLimits::default()` 为离线模式。

## 已实现能力

- `TranslationService` 提供有界 worker、队列、任务状态、超时、取消、内存缓存和提供者注册。
- `LocalDictionaryProvider` 在载入时校验并建立索引。中文词头返回原始英文释义；英文完整释义反查只返回中文词头，结果类型为 `ReverseLookup`，不能视为等价反向翻译。没有逐词替换整句。
- `OpenAiCompatibleProvider` 支持中英双向、Auto 源语言、词/短语/整句的非流式 Chat Completions。用户填写 endpoint/model/credential reference 后，必须同时启用提供者、允许 service 联网，并为每个请求绑定提供者授权。只保存密钥引用，不序列化密钥。
- `TranslationPanel` 保留原文、拒绝旧任务/会话结果。复制和确认只返回 intent；显式确认成功译文后才消费结果一次。研究释义可以查看/复制，不能作为已批准翻译提交。

`Generated` 表示在线模型生成，不表示经过语言质量审核。HTTP 集成测试使用本地真实 TCP 服务器；没有调用实际在线服务或测量翻译准确率。

## 本地词典操作

从已固定的 schema 2 包生成独立研究投影；输入 manifest SHA 必须取自实际包的验收记录：

```text
python scripts/export_translation_dictionary.py --package PATH_TO_LEXICON_PACKAGE --manifest-sha256 EXPECTED_SHA256 --output .local/translation-dictionary
cargo run -p cloudleaf-translation --example dictionary_lookup --release --locked --offline -- .local/translation-dictionary/catalog.json research zh-en word 你好
```

实际导出文件名以工具输出为准。`dictionary_lookup` 是独立命令行工具，只在用户明确查询时显示结果，不写输入历史。它不接入 TSF。

Production 载入必须另行提供 `DictionaryTrustPolicy` 的真实路径及固定 SHA，核对 catalog、来源/许可、当前 mapping 内容与审核文件。修改映射后旧审核失效。仓库没有可用的生产语义映射；正向测试中的角色及映射仅是机制 fixture，不是真人授权。

## 在线 API 的调用约束

1. 在设置线程构造 `OpenAiCompatibleConfig`：完整 HTTPS Chat Completions URL、model、密钥引用，显式 `enabled = true`。HTTP 仅允许另行启用的字面 loopback 地址；不跟随重定向，不自动重试或跨服务回退。
2. 将真实密钥交给 `SecretStore` 实现，配置和日志只保留引用。现有 `SessionSecretStore` 只在会话内保存并在释放 owned secret 时清零；Windows Credential Manager/macOS Keychain/Linux Secret Service 尚未实现，不能把它当作系统安全存储。
3. 保存 `provider.authorization()`，注册时显式允许网络，使用同一个版本值填写 `OpenAiCompatibleConfig.config_version` 和 `TranslationProviderConfig.config_version`，创建 `offline_only = false` 的服务。只有用户明确触发翻译时才给请求设置 `send_authorized_to`。endpoint、model、密钥引用或配置变更后需重新取得授权；存在密钥不代表允许发送。
4. `submit` 返回任务 ID，宿主捕获该请求的 context。换 service 时更换 session/epoch；任务 ID 只在 service 内唯一。向面板传回任意状态时使用 `panel.update(id, captured_context, snapshot)`，不可用当前面板 context 替代捕获值。
5. 新请求、取消或失焦时显式取消旧任务。`Drop` 不阻塞输入线程；需要回收全部 worker 时在后台调用 `shutdown()`。不合作的提供者或系统 DNS 可能延迟实际退出，期间占用既有预算，不生成替补线程。

缓存按提供者/数据/配置版本隔离，同时限制条数与文本 payload 字节；默认关闭在线和整句缓存。payload 上界不代表分配器总内存峰值。资源界限属于单个 service，宿主不能无限创建并丢弃仍有未退出 worker 的 service。

## 尚未实现

离线模型只提供元数据、能力及缺模型错误，没有授权权重或推理 backend；真实 UI、剪贴板、TSF 上屏、平台密钥存储及正式语言质量评估均未交付。新 sidecar 不改变默认输入引擎；不进入 CL-010。协议、边界及验收见 [Round 11 设计](../../docs/translation-round11-design.md)。
