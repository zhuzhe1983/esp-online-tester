# Atom-Lite C008 研究检查点

原版 C008 为经典 ESP32-PICO-D4，4 MB Flash。官网引脚表与原理图均给出 G39 用户键、G27 单颗 SK6812 3535、G12 红外和 Grove G26/G32；按键由 4.7 kΩ 外部上拉，按下接地。不能套用 AtomS3-Lite 的 G41/G35 或 Stamp-S3 的 G0/G21。

已找到并固定官方 Factory Test v1.0，完整镜像 1,048,576 字节、SHA-256 `1c8fa292e31819c4a3ca36c714e93d08f7a8a58f16918f8fdf1df0210e546ce4`。官方 M5Atom `ebbf736396f9459bf1adc7b588a7f1825b366844` 的 `FactoryTest/ATOM_LITE` 源码使用 Adafruit NeoPixel、G27、GRB 800 kHz，循环彩虹，按住按键暂停；这只是来源线索，尚未证明分发 BIN 与该源码一致。当前公开目录保持 unavailable，待真实 ROM/客体输出及输入验收。

经典 ESP32 已有独立 RMT 控制器与板级路由，先实现产品 GPIO/RMT 数字子集，不新增逐 tick 软件色彩生成。若固件用 GPIO 位发送，需核对真实边沿与公开器件时序再决定支持，不能由前端伪造彩虹。无屏产品不增加虚构屏幕。红外、物理 Wi-Fi/BLE、外接 Grove 器件和模拟供电先明确未实现。

来源实际抓取时间、地址及摘要见 [sources.json](evidence/atom-lite/sources.json)。官网当前图的 USB 芯片为 CH552T，而教程 USB 驱动段介绍 FTDI，记录版本来源差异，不由驱动段推导当前板上芯片。
