# 依赖与资源审计扩展设计

旧脚本只允许6个成员、零第三方依赖和3份自编资源。本轮保留这些门禁事实，改为可合并的精确政策，而不是删除检查。

政策目录 `config/audits/*.json`，schema_version=1，各文件只声明本方向新增项：

- `workspace_packages`：准确crate名称列表；metadata实际成员必须与所有政策并集完全一致。
- `external_packages`：name/version/license/source准确组合；metadata的每个非workspace依赖必须逐项相符，拒绝未审核的更新、Git来源或新增传递依赖。
- `resources`：path/source/license/license_file准确组合；`data/manifests/*.json`的实际资源与并集完全一致。每份资源检查UTF-8、SHA-256、条数、格式与仓库内授权文件；支持两行表头TSV及显式format=jsonl的富记录。

`core.json`固定现有6个crate、3份MIT自编资源；A/B/C分别增加windows/search/data政策，不同时编辑同一清单。重复声明、路径越界、许可/来源冲突、缺失或多余条目均失败。新政策仍须人工/独立review，上游许可证证据登记在THIRD_PARTY或本方向报告。

Windows绑定实际锁文件包含的全部传递依赖先审查准确版本、维护/许可与漏洞情况，再锁入政策。CI先`cargo fetch --locked`获得已锁依赖，后续fmt/clippy/test/metadata仍offline运行；这不允许实时按键联网。零依赖基线不再误称后续Windows模块零依赖。

资源format缺省为tsv，header_lines=2；jsonl为每个非空行一个有效JSON对象且不允许坏行静默跳过。规范化LF确保Git存储与工作区哈希一致。运行时格式本身还须对应导入器/核心解析器的语义校验，通用哈希审计不能代替数据质量验收。
