# 原厂 Photo：TF JPEG 与缓存未对齐读取

固定原厂 FactoryOnly（cf10a435…）通过实际 ROM、bootloader、FreeRTOS 与明确选择的离线 Wi-Fi 驱动兼容模式，读取 FAT32 卡中的 JPEG，并经 SPI 向 ILI9488 显示棋格和 `< / Back / >` 控件。固件、ROM 和文件字节未改，没有 host 绘图或 JPEG API 替代。

| 输入或输出 | 字节 | SHA-256 |
| --- | ---: | --- |
| 原 FactoryOnly | 3346992 | cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942 |
| rev100 ROM ELF | 472300 | 49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a |
| 独立 FAT32 测试卡 | 67108864 | f29e47c7fca119caf71353f311cfb7bcbb4ccff2c26f6afcf9c80c9805dba80f |
| /photo/test.jpg | 5200 | 6af0bed529e583dc696a07c3d01088667efff5c4bb0d99932e7d71f42c6b53b6 |
| 实际 RGB565_LE 帧 | 204800 | 81c3850416e159a29fb669a0b58ac59490b5f81444de92f4dd284c8253ae9b03 |
| 按平台 round 扩展的 RGBA | 409600 | 668d3017ab8ba7806c87b1f6e5e1ddb5e42446566a09c7a205bfe816f77e5f5a |

[测试卡生成与独立 FAT/JPEG 回验](../c5-photo-fixture-2026-11-25/README.md)和[两轮真实浏览器证据](../c5-factory-browser-2026-11-25/README.md)分别记录输入与实际消费。原生和浏览器帧逐字一致，90个未被控件遮挡的格中心主色全部匹配。它们与 host JPEG 解码参考不是逐像素一致；RGB565、RGB666转换和图层覆盖均保留，中心最大 RGB 差15/7/15。

## 原失败与有界修复

原失败在 `4201ed26` 执行原字节 `83 57 27 00`，即普通 `LHU a5,2(a4)`。原生记录 `a4=42318109`、READ `4231810b` width2；旧实现仅允许内部 SRAM/LP/ROM 的未对齐数据访问，在进入权限/cache/MMU检查前返回 ModelLimit。旧 trap CSR 属前序中断，不是这条 LHU 的架构异常。[isa.json](isa.json)保存原 BIN 段映射、精确调用点及与官方 esp_new_jpeg1.0.2 对象的48字节匹配；没有匹配厂家 ELF/map，不把参考 ELF 冒充原符号。

[ESP32-C5 TRM1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf) HP2.12.1.2（印刷123页）允许普通未对齐拆分；6.3.3.1/2（242页）描述外部 Flash/RAM 的 cache/MMU 路径。固定 PDF 与 IDF5.5.3/5.5.4 原源、解释边界见 [memory-contract.json](memory-contract.json)。

修复只扩展 validated-ROM 的普通2/4B数据访问：完整 span 留在当前配置大小的同一 MMU 页，每个字节目标一致、物理偏移连续且容量完整。原地址和完整 width 的 APM/PMA/PMP、cache、MSPI 与 PSRAM gates 保留，最后一次 slice 完成；不用递归 read8/write8 分片授权。Flash写入、跨页、MMIO拆分仍拒绝；aligned路径、取指和 LR/SC/AMO 对齐不变。这是功能子集，不声明物理拆分原子性、cache填充或总线错误时序。规则与 Photo PC、文件内容及 Wi-Fi profile 无关。独立源码审查未发现 P1/P2，范围见 [review.json](review.json)。

最终 observer 在原奇地址 LHU 前观察64KiB页/MMU49/entry600，地址映到 PSRAM810b；cache与psram_access资格成立。`jpeg_dec_io_t`在408558ec，输入423180b0/len5200；parse入口和该 LHU 前已有 PSRAM5200B 的 SHA 均等于卡内 JPEG。显式 die/clock/pad 数字来自另列的最终冷快照，不倒填为每项都在该 PC 采样。邻字节含 DQT `ff db 00 43`，长度读后换序为67。

原生达到984800000调度槽/名义周期、393868634 architectural retired，零异常、无first_fault。HLE hookCalls10、virtualSlots15、真实guest eventPosts5、connectAttempts4单列；WFI/虚拟槽不能当物理CPU指令。SD54次内部命令/20次backing load/零写只是辅助计数，真实输入hash与实际成图才证明本例文件消费。

第一次正向已出图，但 observer先捕获同PC的偶地址JFIF长度；该阴性诊断保留。后续仅修改 native示例的一行条件捕获firstodd，两次的最终帧/UART/计数逐字一致。[native.json](native.json)保留各自源码、日志、输入和阶段，不能把后继快照回填旧运行。

## 复现与范围

调用者提供匹配原样输入及不存在的新输出目录：

```sh
cargo build --release -p esp32c5 --example factory_photo_boundary
target/release/examples/factory_photo_boundary \
  "$ROM_ELF" "$ORIGINAL_FACTORY_BIN" "$PHOTO_CARD_IMG" "$FRESH_OUTPUT_DIR"
```

专用示例显式选择离线profile，从零启动后用实际GPIO3两次、GPIO28一次进入Photo；不调用应用内部函数。14项标量合同测试、C5全包597项与Clippy收据在native.json；完整3513项工作区、生产WASM及CPU比较在[整合记录](../c5-final-integration-2026-11-25/README.md)。

本例只覆盖一个固定原厂镜像、FAT32卡与baseline444 JPEG。未证明全部JPEG类型、卡写入、其他型号、射频或物理校准。独立浏览器正常重载保留卡的结论另列，不从原生零写计数推出。原始二进制、日志与帧留在私有ignored存储，公开的是必要来源、参数、hash与结果；没有声称原始捕获已提供公开下载。
