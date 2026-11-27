# Round 28 隔离发行

`tools/release_stage.py` 沿用封装、校验、安装、升级、回滚、卸载和中断恢复。锁定依赖、许可和来源版本随集成 ZIP 提供，内容经清单或 CI 产物成员校验。

- `signature-request RELEASE` 核验已封装内容后，输出清单原始字节的 SHA-256、字节数、版本与提交。签名输入字节和用途独立标识，状态为 `unsigned`。正式证书、签名实施、信任验证和系统安装器为 `blocked`。
- `migrate-user SOURCE_ROOT TARGET_ROOT` 使用两个已初始化的临时隔离根。独立暂存并校验当前、备份设置以及已发布的用户词库代次，包括代次中的用户词条和学习数据。迁移仅将目录发布到空白目标，已有目标或复制失败时保全原始资料。端点引用随设置文件保留，凭据、模型权重、待提交代次和其他目录留在原处。迁移调用要求源隔离宿主已退出。设置格式兼容由既有 Rust reader 的 schema 迁移测试覆盖。
- 专项以两个临时发行根及已有可追溯 Windows CI ZIP 验证。真实系统安装、TSF、证书签名和物理断电为 `not-run`。

使用 Release 的真实 `cloudleaf-user` 和 `cloudleaf-settings` 生成一份隔离用用户资料，包含设置 revision 2、用户词库 generation 1 及一条受控学习记录。迁移后用同一 reader 成功重开目标并核对两份文件 SHA，源资料字节亦相同。命令 `python -X utf8 .local/r28_migration_real.py` 返回 0。原始记录在 `.local/round28-migration-real`，源码/二进制 SHA-256 分别为 `b161f8c2dfd04c74460c9a3454d11aa7a775c75fa643229fb08bc9a12bcfda5f` 和 `7a421e941c05723faa3fe029d2906d3562c340db0711f0e30a86ac9aaa4fa404`。测试只写两个独立的临时目录。

命令 `python -X utf8 .local/r28_release_archived.py` 返回 0。归档 Windows 26→27 演练每个包 480 个封装成员，验证重复安装、升级、损坏检出、回滚、卸载及资料保全。清单 SHA-256 分别为 `7082a5b23714c72126286cec2233b0a4b5dae5ea5d513ea06bd4f67d339c8cf1` 和 `56828093acfaa8664cd53f24e03fa0b7a74f9eb58be03a2eacbedfa772beef10`。两份签名请求均为 `unsigned`。原枚举与核验回执位于 `.local/round28-release-archived`。
