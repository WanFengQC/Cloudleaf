# Round 23 语言模型与输入性能

**partial**。仅 Windows 无窗口机制，沿用一线程真实离线 worker 与固定 171 按键轨迹。实际推理窗口按同机时间戳筛选。按模式重复结果一致，实验新模式的输出允许不同于旧排序。

| 评分 / worker | 样本数 | 墙钟 P50 / P95 / P99 / max（毫秒） | 周期 P50 / P95 / P99 / max |
| --- | ---: | --- | --- |
| lexical/idle | 513 | 0.86 / 5.22 / 7.62 / 9.44 | 2211495 / 12583124 / 19455033 / 24181287 |
| word/idle | 513 | 0.43 / 2.09 / 3.09 / 4.21 | 1098749 / 5186593 / 7061645 / 10730360 |
| lexical/infer | 513 | 1.29 / 8.54 / 18.14 / 22.66 | 3274993 / 21523191 / 39312348 / 56278281 |
| word/infer | 513 | 0.47 / 2.72 / 4.35 / 5.62 | 1207353 / 6693839 / 9523527 / 14267404 |

使用 QueryThreadCycleTime 记录线程周期。周期不换算成毫秒，也不据此推断 ETW 调度等待。本轮未采集 ETW 或 Linux 线程周期。旧 128 MiB 输入内部账本独立，研究模式的语言索引上限 16 MiB。进程工作集和私有内存见采样回执。新模型不可默认启用。正式 5 ms P95、10 ms P99 门槛按实际负载继续验收。[Microsoft Win32 周期计数](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-querythreadcycletime)

| 词典 / 召回 | 配置 | 组 | 搜索 P50 / P95 / P99 / max（毫秒） | 峰值活路径 |
| --- | --- | --- | --- | ---: |
| base-structural | lexical | Round 16 Tatoeba dev | 2.47 / 3.58 / 3.58 / 3.58 | 52 |
| base-structural | character-witten | Round 16 Tatoeba dev | 3.46 / 10.86 / 10.86 / 10.86 | 13604 |
| base-structural | word | Round 16 Tatoeba dev | 1.11 / 1.49 / 1.49 / 1.49 | 252 |
| base-structural | mixed | Round 16 Tatoeba dev | 1.11 / 1.50 / 1.50 / 1.50 | 252 |
| base-structural | lexical | Round 17 新闻 dev | 7.20 / 12.31 / 28.37 / 28.37 | 72 |
| base-structural | character-witten | Round 17 新闻 dev | 10.43 / 26.36 / 36.10 / 36.10 | 18949 |
| base-structural | word | Round 17 新闻 dev | 2.83 / 5.02 / 6.91 / 6.91 | 352 |
| base-structural | mixed | Round 17 新闻 dev | 2.82 / 5.01 / 6.91 / 6.91 | 352 |
| base-structural | lexical | Round 18 输入方案 | 0.56 / 35.22 / 48.19 / 48.19 | 64 |
| base-structural | character-witten | Round 18 输入方案 | 0.63 / 14.45 / 40.79 / 40.79 | 2917 |
| base-structural | word | Round 18 输入方案 | 0.29 / 6.64 / 9.95 / 9.95 | 304 |
| base-structural | mixed | Round 18 输入方案 | 0.28 / 6.64 / 9.95 / 9.95 | 304 |
| base-structural | lexical | Round 20 固定查询去重视图 | 0.48 / 22.18 / 74.71 / 74.71 | 64 |
| base-structural | character-witten | Round 20 固定查询去重视图 | 0.56 / 10.61 / 31.35 / 31.35 | 2917 |
| base-structural | word | Round 20 固定查询去重视图 | 0.22 / 4.82 / 11.90 / 11.90 | 304 |
| base-structural | mixed | Round 20 固定查询去重视图 | 0.25 / 4.82 / 11.90 / 11.90 | 304 |
| base-structural | lexical | Round 23 冻结研发验证 | 3.36 / 7.09 / 7.20 / 7.20 | 59 |
| base-structural | character-witten | Round 23 冻结研发验证 | 3.35 / 11.29 / 16.95 / 16.95 | 17281 |
| base-structural | word | Round 23 冻结研发验证 | 1.22 / 2.54 / 4.07 / 4.07 | 289 |
| base-structural | mixed | Round 23 冻结研发验证 | 1.24 / 2.55 / 4.07 / 4.07 | 289 |
| base-character | lexical | Round 16 Tatoeba dev | 2.37 / 5.25 / 5.25 / 5.25 | 52 |
| base-character | character-witten | Round 16 Tatoeba dev | 3.08 / 13.73 / 13.73 / 13.73 | 13604 |
| base-character | word | Round 16 Tatoeba dev | 7.83 / 64.82 / 64.82 / 64.82 | 49999 |
| base-character | mixed | Round 16 Tatoeba dev | 7.80 / 64.82 / 64.82 / 64.82 | 49999 |
| base-character | lexical | Round 17 新闻 dev | 7.13 / 13.55 / 14.12 / 14.12 | 72 |
| base-character | character-witten | Round 17 新闻 dev | 7.69 / 23.74 / 25.69 / 25.69 | 18949 |
| base-character | word | Round 17 新闻 dev | 30.01 / 64.19 / 74.20 / 74.20 | 50000 |
| base-character | mixed | Round 17 新闻 dev | 30.04 / 64.22 / 74.20 / 74.20 | 50000 |
| base-character | lexical | Round 18 输入方案 | 0.40 / 20.72 / 38.52 / 38.52 | 64 |
| base-character | character-witten | Round 18 输入方案 | 0.42 / 13.80 / 30.51 / 30.51 | 2917 |
| base-character | word | Round 18 输入方案 | 0.70 / 25.69 / 60.93 / 60.93 | 16107 |
| base-character | mixed | Round 18 输入方案 | 0.70 / 25.70 / 60.95 / 60.95 | 16107 |
| base-character | lexical | Round 20 固定查询去重视图 | 0.51 / 25.57 / 54.13 / 54.13 | 64 |
| base-character | character-witten | Round 20 固定查询去重视图 | 0.51 / 10.58 / 48.87 / 48.87 | 2917 |
| base-character | word | Round 20 固定查询去重视图 | 0.71 / 27.47 / 120.17 / 120.17 | 16107 |
| base-character | mixed | Round 20 固定查询去重视图 | 0.70 / 27.47 / 120.19 / 120.19 | 16107 |
| base-character | lexical | Round 23 冻结研发验证 | 3.23 / 6.77 / 7.02 / 7.02 | 59 |
| base-character | character-witten | Round 23 冻结研发验证 | 4.28 / 12.09 / 22.61 / 22.61 | 17281 |
| base-character | word | Round 23 冻结研发验证 | 9.80 / 40.74 / 44.66 / 44.66 | 49999 |
| base-character | mixed | Round 23 冻结研发验证 | 9.80 / 40.80 / 44.66 / 44.66 | 49999 |
| supplement-character | lexical | Round 16 Tatoeba dev | 9.44 / 92.83 / 92.83 / 92.83 | 52 |
| supplement-character | character-witten | Round 16 Tatoeba dev | 9.36 / 53.55 / 53.55 / 53.55 | 18014 |
| supplement-character | word | Round 16 Tatoeba dev | 46.39 / 197.70 / 197.70 / 197.70 | 50000 |
| supplement-character | mixed | Round 16 Tatoeba dev | 46.54 / 197.72 / 197.72 / 197.72 | 50000 |
| supplement-character | lexical | Round 17 新闻 dev | 26.08 / 49.29 / 59.55 / 59.55 | 72 |
| supplement-character | character-witten | Round 17 新闻 dev | 35.51 / 112.85 / 149.32 / 149.32 | 30201 |
| supplement-character | word | Round 17 新闻 dev | 121.82 / 212.48 / 261.52 / 261.52 | 50000 |
| supplement-character | mixed | Round 17 新闻 dev | 121.87 / 212.42 / 261.52 / 261.52 | 50000 |
| supplement-character | lexical | Round 18 输入方案 | 1.53 / 75.60 / 176.88 / 176.88 | 64 |
| supplement-character | character-witten | Round 18 输入方案 | 1.70 / 39.05 / 155.74 / 155.74 | 3862 |
| supplement-character | word | Round 18 输入方案 | 2.75 / 98.89 / 411.75 / 411.75 | 20749 |
| supplement-character | mixed | Round 18 输入方案 | 2.79 / 98.93 / 411.75 / 411.75 | 20749 |
| supplement-character | lexical | Round 20 固定查询去重视图 | 1.34 / 84.10 / 141.41 / 141.41 | 64 |
| supplement-character | character-witten | Round 20 固定查询去重视图 | 1.46 / 28.07 / 83.32 / 83.32 | 3862 |
| supplement-character | word | Round 20 固定查询去重视图 | 2.75 / 70.48 / 232.35 / 232.35 | 20749 |
| supplement-character | mixed | Round 20 固定查询去重视图 | 2.88 / 71.41 / 232.35 / 232.35 | 20749 |
| supplement-character | lexical | Round 23 冻结研发验证 | 7.60 / 36.40 / 52.02 / 52.02 | 61 |
| supplement-character | character-witten | Round 23 冻结研发验证 | 13.97 / 42.24 / 61.66 / 61.66 | 26902 |
| supplement-character | word | Round 23 冻结研发验证 | 41.61 / 155.58 / 201.17 / 201.17 | 50000 |
| supplement-character | mixed | Round 23 冻结研发验证 | 41.64 / 155.58 / 201.17 / 201.17 | 50000 |

输入原始样本、缓存及候选计时均来自开发机。单次搜索分配与 GEMM 内部逐任务并行规模没有独立硬件剖析。
[性能采样回执](integration-round23-evidence/round23-public/input-performance.json)
