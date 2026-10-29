# Round 5 固定词条隔离审计的权利与修改说明

本目录含 CC-CEDICT 原文及派生分类，数据适用 CC BY-SA 4.0；许可、贡献者/MDBG/固定镜像署名及原件 URL 逐字节封装在 upstream-documents.json。以 UTF-8 编码 documents[].utf8_text 即恢复对应 LICENSE.txt/NOTICE.txt，核对 bytes/sha256；工具代码的 MIT 不改变这些数据的许可。

2026-10-28 整理：使用实现 `6d3d4e1ac717ad96de8c839ee9857c3629ebecbc` 的只读审计工具，对固定 2026-06-04 原件中保留的 1,468 条隔离记录追加原行 SHA、token 表达形状、主类别、证据边界及未实施处置。没有改原读音、补词频、修改生产音节表或将隔离记录转为接收；分类不是普通话规范或语言准确率裁决。

原件 SHA-256 `fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a`；新包 manifest SHA-256 `5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5`。`summary.json` 的 output_sha256 指向原始审计器生成的 LICENSE/NOTICE/records，许可/署名是 JSON 中还原的原文字节而非 JSON 封装字节。本说明在证据发布时另附，不改本地审计输出、原文末尾空行或原始摘要。初次发布的空行检查失败见 round5-publication-check.json。
