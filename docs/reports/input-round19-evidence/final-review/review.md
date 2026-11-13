# Round19 最终报告只读复核

核对时间：2026-11-13。实际源码 HEAD：`e0d5cb26d9e173f6dd4999ec7628cc30e7394e11`。未修改实际工作区，未调用桌面工具。

结论：未发现需要推翻报告 partial 结论的数字或完成状态夸大。报告、`r19-search/delivery-finish/summary.md`、`user-layer-observations.md`、`r19-engine-perf/handoff.md` 的关键数值一致。

- 独立核算 `locked-04/stdout.txt`：281 passed、0 failed、2 ignored；文档测试为0个，不能把0个声称为额外成功用例。
- `locked-08/stderr.txt`：71 tests / OK。`round19-locked-results.json` 共13个不同命令，所有返回码0，全部绑定上述 e0d5cb26 HEAD。
- 默认 Engine 5130事件、Host 2052事件、42进程；与用户层最终查询4185和严格Candidate差分188325是独立分母，报告没有混用。
- 90/837降为6/837、仍有ResidentPaths失败；简拼延迟退化及默认字符P95超过5ms均保留，不应改成“性能通过”。
- 报告明确说明真实窗口只完成添加、保存、显式开启学习及全拼/简拼候选。构造IsolatedOwnedWindow来源标签的worker测试不等于真实窗口Applied，报告对此界限准确。
- 抽查实际源码：`Window::sink`写后读回，`sample.selection`只在Applied分支读取；普通CommitSucceeded没有直接学习；窗口入口对active learning、busy和snapshot身份检查，worker对generation、会话epoch和字典epoch检查。该抽查不是穷尽安全证明或真实TSF验收。
- Store析构显式unlock及保留重复描述符的回归确实存在。原Linux Busy失败应随最终证据保留，不以本地通过覆盖。

交付前需补齐：

1. 报告写明锁修复后最终281/0/2与Python71的实际总数，并与旧280的身份区分。
2. GUI段“不拿16项存储测试”是旧口径；当前cloudleaf-user为17个测试（1单元+2契约+14存储），建议省略该数字，避免把17全称作存储测试。
3. 本次检查时 `round19-ui-final/completion.json` 尚不存在。需明确记录用户暂停、未关闭窗口、Applied学习/重启/删除/导入导出等未完成。
4. 本次检查时证据目录及最终Git外交付回执仍未生成；报告的目标链接不是已存在证据。最终归档后校验所有链接、index、数据/工具/二进制SHA，再以最终报告HEAD的新push CI填写回执。
5. 性能和实际窗口绑定8418738二进制，最新文件锁修复绑定e0d5cb26，不能把旧性能/UI二进制身份替换成最新release身份。

仍不放行CL-010；不合并main、不改变五个Draft、不操作TSF、不运行长测。窗口自动化按用户指令继续暂停。
