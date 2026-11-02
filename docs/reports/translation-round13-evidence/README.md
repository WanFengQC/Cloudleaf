# Round 13 原始证据

`index.json`绑定原始文件字节。目录属性禁用Git换行改写，许可证/日志不手工编辑。`runs`保留每次实际命令、返回码、stdout/stderr、源码/工具指纹；原始完整工作区diff仍在本机`.local/round13-runs`，其SHA在result中。

`tools`是执行时的开发runner快照，原执行位置为仓库`.local`。重放时先复制回自己的`.local`，核对路径和工具版本，再选择新的输出目录；这些脚本默认拒绝覆盖已存在的运行。模型管理脚本会在其新建沙盒内真正损坏、恢复、导入和卸载自有副本，不可替换成用户原件目录。原件/权重/转换环境不在Git中，需按固定锁和已授权范围单独准备。

`model-source-review/sources.json`只描述早期29份来源采集，未声称当时下载权重；后来的真实权重取得和转换见`models`。`conversion-tools/review-draft.md`是实际只读审查稿原文，最终采用结论与局限见上一级模型运行时审查。开发Python工具未进行生态漏洞扫描，不随产品分发。

`gui`区分旧窗口、失败方式、工具误启动与最终正常观察；不能把所有launch都计作产品正常退出。`inference`的三个独立采样各自有二进制和数据SHA，整进程峰值不称单模型内存。

最终报告Git HEAD及自己的push CI不能写入它自身，另见本机`.local/round13-delivery-receipt.json`和Draft PR5正文；旧实现CI不能替代最终报告提交。
