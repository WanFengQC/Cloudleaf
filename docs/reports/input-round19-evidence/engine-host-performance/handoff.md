# Round19 默认 Engine 与无窗口 Host 实测

源码HEAD：`84187386ea36e8fdb3cf88300c0d35aa86c91cb6`；旧版来自R18最终CI `cf54c84d400b627dfa9b246b3800a82d65f061b8` 的固定 `input_trace.exe`，ZIP CRC与下载清单成员SHA已核验。当前二进制/源码测量前后不变。

这组是默认用户层关闭，不是用户层开启或新方案性能。全部42个进程返回0；无GUI、私人词库、TSF、heldout或正式准确率。普通共享Windows桌面，不是专用实时验收。

## 默认 Engine K10 配对

每版本/模型171事件×5个新进程=855，交替旧新和模型顺序，保留首次与全部长尾；合计5130事件。15组完整稳定语义输出相同，含EVENT的handled/error/preedit及FRAME、CANDIDATE、COMMIT、SEARCH、PATH、SUMMARY。仅去掉EVENT耗时时钟列和新增加且严格等于0的user_preference_units；没有移除正文、ID、评分、edge、来源、解释成本或统计。LOAD/MODEL容量身份原样保留但不作为候选语义相等断言。

|版本/模型|n|P50 ms|P95 ms|P99 ms|max ms|
|---|---:|---:|---:|---:|---:|
|old/lexical|855|0.7142|3.5406|5.1016|8.5387|
|new/lexical|855|0.6861|3.6303|5.0104|6.2172|
|old/addone|855|0.6553|5.2241|11.8447|16.7537|
|new/addone|855|0.6670|5.2034|11.0996|15.1255|
|old/witten|855|0.7153|4.9312|12.8144|15.4050|
|new/witten|855|0.7264|5.1823|10.8990|14.1390|

默认P95变化：Lexical +2.53%，add-one -0.40%，Witten–Bell +5.09%。两个当前字符模型仍超过5ms，不宣称零开销或门禁通过。

## K5 双引擎 Host

既有四条件各3个新进程×171事件=513；共2052事件。无HWND、内存文档sink，SIMULATED Applied不持久学习。两引擎+比较/事务的total不能与上面的单Engine K10混比。

|模型/阶段|n|P50 ms|P95 ms|P99 ms|max ms|
|---|---:|---:|---:|---:|---:|
|lexical/core|513|0.4847|2.0910|2.7919|4.0238|
|lexical/projection|513|0.3975|1.8180|2.7127|6.4426|
|lexical/compare|513|0.0013|0.0027|0.0092|0.0182|
|lexical/transaction|513|0.0000|0.0003|0.0041|0.0076|
|lexical/frame|513|0.0005|0.0011|0.0020|0.0161|
|lexical/total|513|0.8928|3.7353|5.7476|9.8789|
|addone/core|513|0.6005|3.9006|6.8172|11.4286|
|addone/projection|513|0.5363|3.8264|6.6787|14.4425|
|addone/compare|513|0.0013|0.0024|0.0049|0.0185|
|addone/transaction|513|0.0001|0.0004|0.0042|0.0309|
|addone/frame|513|0.0005|0.0010|0.0026|0.0166|
|addone/total|513|1.1489|7.7418|13.4384|25.8784|
|witten/core|513|0.6035|3.6631|6.1854|10.7881|
|witten/projection|513|0.5370|3.6700|6.2819|11.3172|
|witten/compare|513|0.0013|0.0023|0.0041|0.0172|
|witten/transaction|513|0.0001|0.0004|0.0042|0.0185|
|witten/frame|513|0.0005|0.0010|0.0022|0.0133|
|witten/total|513|1.1525|7.3416|13.0478|20.9661|
|supplement-addone/core|513|0.8187|6.9617|11.9746|17.0888|
|supplement-addone/projection|513|0.7639|7.5170|11.1887|19.8318|
|supplement-addone/compare|513|0.0013|0.0024|0.0054|0.0159|
|supplement-addone/transaction|513|0.0001|0.0005|0.0048|0.0302|
|supplement-addone/frame|513|0.0005|0.0012|0.0026|0.0173|
|supplement-addone/total|513|1.6411|14.1440|22.7151|35.5062|

## 50ms进程资源观测

包含原件读取缓冲、不可变词典/模型、查询及输出诊断。这里是进程采样和采样时读取的OS峰值计数，不是单算法分配、COM、Host内部128MiB预算或物理内存硬上界；退出前最后一段可能未采样。

|条件|进程数|采样数|working最大MiB|private最大MiB|采样OS peak最大MiB|
|---|---:|---:|---:|---:|---:|
|engine-old-lexical|5|79|185.30|205.43|185.56|
|engine-new-lexical|5|79|184.02|205.57|185.68|
|engine-old-addone|5|110|210.39|212.70|211.73|
|engine-new-addone|5|111|208.84|212.75|211.27|
|engine-old-witten|5|111|211.18|212.70|211.25|
|engine-new-witten|5|118|210.88|212.76|211.27|
|host-lexical|3|48|188.42|206.12|189.63|
|host-addone|3|74|214.00|213.93|215.72|
|host-witten|3|75|212.31|213.20|215.57|
|host-supplement-addone|3|88|216.12|216.95|218.36|

## 身份与原始材料

`summary.json`记录全部实际命令、退出码、二进制/模型/词典/事件SHA、全部资源样本与源码前后SHA。`runs/*/stdout.txt.gz`为原始输出，compression.json逐项验证gzip CRC和解压原字节；stderr与result完整保留。

旧input_trace SHA `97958b951dfa295e947104b6bdca194186e1315930267acd1e5f4523596d979f`；新 `10dc723f1c4e66f2792182cfffb1577ce4b0f8162fd2f67baf564a880ecfefdf`；Host `cb751c039598c1738f7159638c6b300c0b2bf80919ffd33b4761cb13ac4d07aa`。

没有使用该默认性能结果声称简拼/用户词库/真实Windows输入法达到正式产品质量。
