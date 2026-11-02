# Round 14证据说明

原始命令、stdout/stderr和每次返回码保留，`index.json`核实际字节。Git属性禁止改写这些原始文件换行。原始工作区diff在本机`.local/round14-runs`，result保留其SHA，未把带未提交改动的运行冒充干净HEAD。

大词典、源ZIP、二进制和任何个人配置均不入本目录；它们的实际路径及SHA分别登记。`initial-reviews`为源方向只读核验，`search-review`为独立temp技术对照，不是人工gold。真实R14实现的专项以`runs`和`round14-search-comparison`为准。

`tools`是实际执行的开发runner快照，原位置在仓库`.local`，重放先复制到自有`.local`并选择新的输出目录，勿覆盖旧结果。固定包须按已有授权准备，脚本不负责授权或下载。`round14-real-input-final/stdout.tsv`包括真实错误首选及模拟提交，不是发布质量数据。

最终报告提交HEAD、自己的push CI/日志/产物校验和远程PR状态放在本机`.local/round14-delivery-receipt.json`，避免报告自引用SHA；本轮不创建或修改第六PR，也不以实现提交CI代表报告提交。
