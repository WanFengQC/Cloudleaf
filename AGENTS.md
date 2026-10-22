# 协作约定

- 所有文件读写使用 UTF-8，修改时保持原有编码。PowerShell 读取中文前执行 `chcp 65001` 并设置 UTF-8 输出，读取使用 `Get-Content -Encoding UTF8`。
- 不使用 sed/awk 处理含中文的文件，使用 Python 或 Node.js；不用 PowerShell here-string 管道给 Node 写入中文脚本。
- 代码注释使用中文。
- 先阅读 `docs/roadmap.md` 和对应行为文档；接口、格式和用户可见行为变化先更新设计约定。
- 核心不访问网络，不引入 Windows 专属业务依赖，不在实时输入路径读文件、运行模型或调用 Python。
- 完成代码后运行 README 中的格式、静态检查、测试与资源审计；原型性能不得表述为生产词库或真实 Windows 输入法性能。
