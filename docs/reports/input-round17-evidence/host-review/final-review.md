# Round17隔离Host只读代码复核

复核基线：`c80e9ec3b03bd75120e2005f57eedd1912625f75`。不修改实际工作区，不运行Cargo、UI或性能采样。

## 已确认的具体缺口

- P1，整窗失活：父窗键盘焦点移到只读EDIT时，父窗WM_KILLFOCUS因同一GA_ROOT被忽略；之后EDIT向外失焦，父窗不再收到WM_KILLFOCUS。父窗WM_ACTIVATE原未处理，组合/候选未取消。已准备实际隐藏HWND测试与最小WM_ACTIVATE修复；主任务报告本地red/green均已执行。这里不冒充本审阅者执行。
- 实际键盘验收阻断：主任务观察到当前系统IME截获字母，隔离核心修订不变。已核固定windows0.62.2的窗口级IMM绑定与微软原始说明，准备仅本窗口/子窗口解绑的feature、测试与实现补丁。IACE_CHILDREN只作用子窗口，父窗另需flag0。实际GUI是否恢复必须由主任务重验。
- 重复失焦材料边界：retained在每次FocusLost赋值覆盖，先nihao失焦、再hao失焦会覆盖早先材料；Unknown也覆盖旧retained。该事实不涉及猜测竞态，且仅属于隔离Host。若不实现显式材料确认或有界保全，则不能声称重复失焦过程普遍不丢字。

## 已核的实现边界

- 候选回调使用现有消息级Rc强引用，新增同步销毁测试覆盖同一实际WndProc；没有发现需要另造跨线程生命周期模型的证据。
- Unknown不重试，核心停在Committing，quarantine阻止新事件；明确新epoch使旧候选失效。没有发现该隔离实现会向其他应用提交文本的路径。
- 主流程/默认引擎、TSF注册与生产COM提交没有本轮修改。
- WM_CHAR不组合UTF-16代理对、没有剪贴板粘贴路径，README已经标注未实现；不作为Unicode完整验收通过。
- 无窗口benchmark使用真实双引擎与内存文档事务，但窗口/TSF阶段为not-run；这与平台端到端响应有明确差别。

## 历史保全

`review-preservation.json`绑定同一复核提交：旧1973个docs/reports、data、tests、licenses路径没有改动；222个旧Rust测试函数全部保留。新补丁和脚本SHA在对应JSON内。该证明不包含之后主任务尚未提交的修改，最终交付仍需以新HEAD再次绑定。

## 临时文件

- `tests-only.patch`、`implementation-after-tests.patch`：先焦点失败测试，再焦点实现。
- `tests-only-ime.patch`、`implementation-after-tests-ime.patch`：基于焦点修复后源文件，再做窗口级IME隔离。
- `prepare.py`、`prepare-ime.py`：可重建补丁与身份。
- `ime-binding-review.json`：官方API链接、范围及补丁SHA。

以上测试与补丁在本子任务未编译执行；只有主任务实际命令日志能支持red/green或GUI结论。
