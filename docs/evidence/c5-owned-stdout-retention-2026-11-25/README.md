# C5 owned stdout 保留清单

按[证据保留规范](../README.md)，BOD0、WiFi modem-map与COEX gate的6份完整测试/lint stdout从当前源码树移至ignored `target/`，共27208B。移动前后逐字和SHA256核对一致；没有改动capture内容，没有重写旧commit，没有上传release。

[manifest.json](manifest.json)保留每份原路径、bytes/SHA、Git blob、可取回原文件的artifact revision、实际被测source revision、结果和调用者输出目录命令。三份checks继续保留原始artifact hash/负面记录，另加retention位置；manifest记录原checks与更新checks的hash映射，原checks也逐字保存在ignored目录。原stdout和旧receipt副本均已确认可读后才移除跟踪文件。

私有输出根使用`${INTEGRATION_ROOT}/target/c5-owned-stdout-retention-20261125/from-4e1a8474/`，按原evidence目录分组；不记录本机身份。公开摘要中的结果、输入与source hash、命令和限制已足够复核组件结论，完整逐test stdout不增加正确性证明。原文件仍可从固定artifact revision按原路径取回并验hash；这次只减少当前tree，不回收旧历史字节。

后续重现时将harness的`--output-dir`指向调用者的ignored输出目录，不再复制完整stdout回Git。此清理没有运行组件测试或整机，也不提升原有BOD0/WiFi/COEX的功能验收范围。
