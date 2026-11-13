# 只读探针操作错误

- 首次exec将尚未创建的`r19-search/source`作为cwd，工具在命令执行前拒绝：`Failed to create unified exec process: 目录名称无效。(os error 267)`。设置和构建均未执行；改在已存在的临时父目录执行setup，再使用生成的source目录。
- 首次`python run_probe.py baseline`两个实际模型进程已运行，stdout/stderr保留；汇总工具行22 `sha(__file__)`传str给仅支持Path的辅助表达式，在行10抛出`AttributeError: 'str' object has no attribute 'read_bytes'`，工具exit1。将sha入口统一为Path，不覆盖baseline原始日志，重新运行使用baseline-final目录。
- 首次`capture.py sparse-build cargo build ...`在Python subprocess创建进程时`FileNotFoundError: [WinError 2]`，编译未启动；Windows CreateProcess的可执行文件查找未采用child env PATH。辅助工具先按该PATH解析为实际绝对路径，新目录重跑，不覆盖失败目录。
- `capture.py sparse-r18-diff target/release/diff.exe ...`同样因Windows可执行文件查找不按child cwd相对解析而抛出`FileNotFoundError: [WinError 2]`，探针未执行。后续使用已生成exe的绝对路径，失败目录不覆盖。
# 2026-11-13 复跑工具错误

尝试以PowerShell `python -c` 内嵌f-string打印量化表时引号转义失败：SyntaxError `'[' was never closed`；它只打印汇总，未执行测量或修改结果。改用UTF8独立summarize_finish.py，原数据不改。

`python replay_r18_finish.py` 首次返回1，line20的Python条件表达式误写`else20`，SyntaxError: expected 'else' after 'if' expression。脚本解析前即失败，没有执行查询；改为空格分隔的`else 20`，保留此错误记录，不改分母/生产实现。

