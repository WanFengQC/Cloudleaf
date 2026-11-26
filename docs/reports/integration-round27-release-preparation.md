# Round 27 隔离发行准备

`python -m tools.release_stage` 为 Windows/Linux 可重放的目录发行演练。其操作根必须是系统临时目录下的显式子目录，初始化限定空目录，后续认根标识。版本清单记录源码提交、角色、包内路径、文件大小与 SHA-256。预设目录为 `bin`、`licenses`、`provenance` 和 `resources`。演练材料包含固定来源锁、catalog 模板、许可证原件及 Cargo.lock，模型权重和用户配置无法进入包。

复用仓库原 `publish_lock`、版本目录校验、原子指针、进程身份和显式恢复。重复安装、版本冲突、升级、已装损坏、回退、进程退出和重复卸载均有实际测试。卸载原子撤销活动指针，留下内容寻址缓存及原件用于审计和显式重装。现有用户设置及词库字节保持，发行器无任何系统凭据写入路径。实际进程退出验证的是进程级恢复。

四个 Python 专项测试此前均通过。两份已核验 Windows CI ZIP（Round 25 与 Round 26）各选 480 个真实构建／许可／溯源／资源文件进行同一临时目录升级和回退。升级、篡改拒绝、回退、重复卸载和用户设置保全通过。清单及 SHA 收录在 `.local/round27-release-archived-longpath/public-safe.json`。首次路径过滤拒绝合法依赖名中的加号；第二次 Windows 长路径读回出错。实际 ZIP 再验证成功，首次异常记录继续保留。命令日志在 `.local/round21-runs/round27-release-archive-longpath` 和 `.local/round21-runs/round27-release-stage-final`。

正式签名安装器、用户可接受的缓存回收政策、真实覆盖升级、TSF 注册和卸载等待未来的授权环境验收。REQ-QA-05 为部分实现。
