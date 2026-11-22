# LILYGO 全目录适配记录 · 2026-11-22

本轮处理现有 LILYGO 类目的 **127 个商品条目**：保留原有 5 个部分支持的运行器，新增 **121 个有明确范围的诊断配置**，当前 **126 个条目可进入芯片／板级调试**。全部条目都有可检查的 3D 预览。这里的范围是项目已经收录的 Spark 商品 ID，不代表厂家所有历史硬件选项。

**诊断适配的含义：** 执行真实 CPU 指令，按所选硬件资料接入已经实现的外设路径；它不表示任意出厂固件、全部通信功能或实物电气行为已经验证。每款的所选版次、引脚、未实现功能、来源冲突和测试范围保存在独立知识页中。

## 发布时间顺序

工作台、搜索和下表共用 `compareProductChronology`：已确认的官方硬件版次／公告日期优先采用；缺少首发资料时，明确标注官方上架时间作为排序代用。日期不明的设备置后。同名不同预装软件条目保留原 ID；仓库最近提交时间不被当成产品发布日。

下表日期标记：**硬件**＝官方硬件版次日期，**公告**＝官方公告，**上架代用**＝官方商品页发布时间，**仓库代用**＝最早官方记录。硬件版次日期也不自动等于零售首发日。

当前日期依据：**24 项硬件版次日期、100 项上架日期代用、3 项未知**。上架日期可能反映重新上架或预装软件版本，不能据此确认首次零售发布日。

## 运行与模型覆盖

| 项目 | 当前范围 |
| --- | --- |
| 原有运行器 | 5；原有固件目标与默认映射保留 |
| 新增诊断配置 | 121；esp32: 39、esp32c3: 4、esp32c5: 2、esp32c6: 5、esp32p4: 1、esp32s2: 3、esp32s3: 67 |
| 新增模型 | 122；官方 CAD 部件 44，官方照片轮廓模型 78 |
| 全目录模型 | 原有 4 个官方 CAD、1 个有限照片模型与本轮新增模型；均保留 partial 状态 |
| 新增出厂固件兼容声明 | 0；本轮的协议／自编程序测试不提升为原厂固件验证 |
| 实机／机械装配校准 | 未进行；CAD 导出精度与实物精度分别记录 |

### 有意保留的边界

- **T-Radar**：官方资料描述的是 A121 雷达扩展，需要外部主机；目录已纠正主控标签，保留该商品模型与资料，不为它虚构独立 ESP32-S3。
- **S2 与 C5**：独立芯片后端的 SRAM app-direct 调试；普通 ROM、Flash/MMU、ESP-IDF／UIFlow 启动不在已实现范围内。S2 的 IRAM 和 DRAM 是同一 320 KiB SRAM 的别名，加载器拒绝物理重叠、越界和错误入口。
- **P4**：所选 T-Display-P4 V1.0 的 GPIO、XL9535 I2C0 和原生 SD 路径；HI8561 屏幕、C6 协处理器和完整出厂启动没有被协议测试证明可用。
- **多处理器和未知内存**：T-Pico 的 C6、T-BAO 的 ESP32、Watch Keyboard C3 的 MCU 子集与其他处理器／键盘／屏幕分开。模拟 Flash/PSRAM 容量不替代尚未知的物理容量。
- **不同屏幕和版次**：T-Touch Bar 选择 H621 双 JD9613 AMOLED；T-Keyboard S3 Pro 选择所述 STM32 固件与 4 片 GC9107；T-Connect Pro 选择 LoRa+屏幕版；T-RGB 选择 2.1 英寸 Full Circle。各自资料中未选的硬件选项不继承适配声明。
- **电子纸**：SSD1680、SSD1681、UC81xx、IL91874 和 ED047TC1 保留各自的显存、尺寸与刷新规则。原始 ED047TC1 屏幕当前为黑白数字子集，模拟电压、完整灰阶与实物刷新时间未验证。
- **通信与电源**：已实现的数字 LoRa、SD 传输和按键状态单独声明；不能由主芯片可执行推断蜂窝网络、GNSS、RF、摄像头、模拟电池、USB 或音频已经可用。

### 3D 来源选择

优先使用匹配所选型号的官方 STEP/STL，保留原始文件哈希、固定 Git 版本、单位说明和刚体变换。外壳零件、PCB、支架与分散打印布局按实际内容标注；不把部件边界写成已测整机尺寸。STL 本身不编码单位，未标定的毫米解释保持明确。

没有确认适合所选版次的 CAD 时，使用保留的官方主要视图建立投影轮廓粗模，其他角度用于比对；个别商品只有一张正反面合成图。照片模型不宣称为摄影测量重建：厚度、绝对尺度、被遮挡结构和屏幕位置仍可能未知。实时屏幕与尚未校准位置的按钮使用独立标记的调试面板；照片中的演示画面不会伪装成固件输出。

官方仓库候选也逐个区分：T-Circle 的 `.OBJ` 是编译器目标文件，KiCad 封装不等于整机模型，其他尺寸的 T-RGB、旧版 T-Embed、TWR Plus 外壳不能自动用于当前变体。详见 [CAD 选择审阅](research/lilygo-cad-selection-review-2026-11-22.json)、[92 个仓库快照](research/lilygo-cad-discovery-2026-11-22.json)及[模型导出报告](research/lilygo-model-expansion-2026-11-22.json)。

## 全部 127 项，按当前日期顺序

“诊断”只表示本表所链接资料中已实现的范围；“原有部分支持”沿用之前单独记录的验证。屏幕尺寸是所选运行画布或明确标注的实体预览，不是其他硬件选项的兼容承诺。

| # | 设备／逐款资料 | 日期及依据 | 运行配置 | 屏幕／调试画布 | 模型来源 |
| ---: | --- | --- | --- | --- | --- |
| 1 | [LILYGO T-Dongle C5](../../web/platform/knowledge/lilygo-t-dongle-c5.json) | 2026-01-28 · 上架代用 | 诊断 · esp32c5 | 80×160 ST7735 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-dongle-c5/model.json) |
| 2 | [LILYGO T7-C5](../../web/platform/knowledge/lilygo-t7-c5.json) | 2026-01-28 · 上架代用 | 诊断 · esp32c5 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t7-c5/model.json) |
| 3 | [LILYGO T-Display S3 Pro External](../../web/platform/knowledge/lilygo-t-display-s3-pro-external.json) | 2025-11-28 · 上架代用 | 诊断 · esp32s3 | 222×480 ST7796 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-display-s3-pro-external/model.json) |
| 4 | [LILYGO T-Lora Pager Meshtastic](../../web/platform/knowledge/lilygo-t-lora-pager-meshtastic.json) | 2025-10-10 · 上架代用 | 诊断 · esp32s3 | 222×480 ST7796 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-lora-pager-meshtastic/model.json) |
| 5 | [LILYGO T-Deck Pro Meshtastic](../../web/platform/knowledge/lilygo-t-deck-pro-meshtastic.json) | 2025-08-19 · 上架代用 | 诊断 · esp32s3 | 240×320 UC8253 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-deck-pro-meshtastic/model.json) |
| 6 | [LILYGO T-Embed CC1101 Plus](../../web/platform/knowledge/lilygo-t-embed-cc1101-plus.json) | 2025-08-08 · 上架代用 | 诊断 · esp32s3 | 170×320 ST7789 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-embed-cc1101-plus/model.json) |
| 7 | [LILYGO T-Beam BPF](../../web/platform/knowledge/lilygo-t-beam-bpf.json) | 2025-07-22 · 上架代用 | 诊断 · esp32s3 | 128×64 SH1106 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-beam-bpf/model.json) |
| 8 | [LILYGO T-Display Bar](../../web/platform/knowledge/lilygo-t-display-bar.json) | 2025-07-18 · 上架代用 | 诊断 · esp32s3 | 76×284 ST7789 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-display-bar/model.json) |
| 9 | [LILYGO T-Watch Ultra](../../web/platform/knowledge/lilygo-t-watch-ultra.json) | 2025-07-18 · 上架代用 | 诊断 · esp32s3 | 410×502 CO5300 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-watch-ultra/model.json) |
| 10 | [LILYGO T3 S3 V1.3](../../web/platform/knowledge/lilygo-t3-s3-v1-3.json) | 2025-07-18 · 上架代用 | 诊断 · esp32s3 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t3-s3-v1-3/model.json) |
| 11 | [LILYGO T-2CAN](../../web/platform/knowledge/lilygo-t-2can.json) | 2025-07-07 · 硬件 | 诊断 · esp32s3 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-2can/model.json) |
| 12 | [LILYGO T-Radar](../../web/platform/knowledge/lilygo-t-radar.json) | 2025-06-24 · 上架代用 | 扩展模块模型／资料 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-radar/model.json) |
| 13 | [LILYGO T-Display P4](../../web/platform/knowledge/lilygo-t-display-p4.json) | 2025-06-13 · 硬件 | 诊断 · esp32p4 | 540×1168 HI8561 / H0405S002T002-V0（实体屏幕预览，尚无帧输出） | [官方 CAD 部件](../../web/platform/assets/lilygo-t-display-p4/model.json) |
| 14 | [LILYGO T-Embed SI4732](../../web/platform/knowledge/lilygo-t-embed-si4732.json) | 2025-05-12 · 上架代用 | 诊断 · esp32s3 | 170×320 ST7789 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-embed-si4732/model.json) |
| 15 | [LILYGO T-Beam Meshtastic](../../web/platform/knowledge/lilygo-t-beam-meshtastic.json) | 2025-05-05 · 上架代用 | 诊断 · esp32 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-beam-meshtastic/model.json) |
| 16 | [LILYGO T-Deck Meshtastic](../../web/platform/knowledge/lilygo-t-deck-meshtastic.json) | 2025-05-05 · 上架代用 | 诊断 · esp32s3 | 320×240 ST7789 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-deck-meshtastic/model.json) |
| 17 | [LILYGO T-Beam SUPREME Meshtastic](../../web/platform/knowledge/lilygo-t-beam-supreme-meshtastic.json) | 2025-04-22 · 上架代用 | 诊断 · esp32s3 | 128×64 SH1106 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-beam-supreme-meshtastic/model.json) |
| 18 | [LILYGO T-Deck Plus Meshtastic](../../web/platform/knowledge/lilygo-t-deck-plus-meshtastic.json) | 2025-04-22 · 上架代用 | 诊断 · esp32s3 | 320×240 ST7789 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-deck-plus-meshtastic/model.json) |
| 19 | [LILYGO T-Deck Pro](../../web/platform/knowledge/lilygo-t-deck-pro.json) | 2025-03-26 · 上架代用 | 诊断 · esp32s3 | 240×320 UC8253 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-deck-pro/model.json) |
| 20 | [LILYGO T-Lora Dual](../../web/platform/knowledge/lilygo-t-lora-dual.json) | 2025-03-15 · 硬件 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-lora-dual/model.json) |
| 21 | [LILYGO T-Connect Pro](../../web/platform/knowledge/lilygo-t-connect-pro.json) | 2025-02-05 · 硬件 | 诊断 · esp32s3 | 222×480 ST7796 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-connect-pro/model.json) |
| 22 | [LILYGO T-Display S3 Pro LR1121](../../web/platform/knowledge/lilygo-t-display-s3-pro-lr1121.json) | 2024-12-14 · 硬件 | 诊断 · esp32s3 | 222×480 ST7796 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-display-s3-pro-lr1121/model.json) |
| 23 | [LILYGO T-Deck Plus](../../web/platform/knowledge/lilygo-t-deck-plus-1.json) | 2024-12-09 · 上架代用 | 诊断 · esp32s3 | 320×240 ST7789 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-deck-plus-1/model.json) |
| 24 | [LILYGO T3 TCXO](../../web/platform/knowledge/lilygo-t3-tcxo.json) | 2024-11-27 · 上架代用 | 诊断 · esp32 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t3-tcxo/model.json) |
| 25 | [LILYGO T5 E-Paper S3 Pro](../../web/platform/knowledge/lilygo-t5-e-paper-s3-pro.json) | 2024-11-14 · 上架代用 | 诊断 · esp32s3 | 960×540 ED047TC1 raw source/gate panel | [官方 CAD 部件](../../web/platform/assets/lilygo-t5-e-paper-s3-pro/model.json) |
| 26 | [LILYGO T-Bao](../../web/platform/knowledge/lilygo-t-bao.json) | 2024-11-13 · 上架代用 | 诊断 · esp32 | 240×240 ST7789（实体屏幕预览，尚无帧输出） | [照片轮廓粗模](../../web/platform/assets/lilygo-t-bao/model.json) |
| 27 | [LILYGO T3-S3 MVSR](../../web/platform/knowledge/lilygo-t3-s3-mvsr.json) | 2024-11-06 · 硬件 | 诊断 · esp32s3 | 128×64 SSD1306 | [官方 CAD 部件](../../web/platform/assets/lilygo-t3-s3-mvsr/model.json) |
| 28 | [LILYGO T-Keyboard S3 Pro](../../web/platform/knowledge/lilygo-t-keyboard-s3-pro.json) | 2024-09-05 · 硬件 | 诊断 · esp32s3 | 512×128 4 × GC9107；4 片合成调试画布 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-keyboard-s3-pro/model.json) |
| 29 | [LILYGO T-Circle](../../web/platform/knowledge/lilygo-t-circle-s3.json) | 2024-08-15 · 硬件 | 诊断 · esp32s3 | 160×160 GC9D01N | [照片轮廓粗模](../../web/platform/assets/lilygo-t-circle-s3/model.json) |
| 30 | [LILYGO T-Connect](../../web/platform/knowledge/lilygo-t-connect.json) | 2024-08-12 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-connect/model.json) |
| 31 | [LILYGO T-Lora Pager](../../web/platform/knowledge/lilygo-t-lora-pager.json) | 2024-08-05 · 硬件 | 诊断 · esp32s3 | 222×480 ST7796 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-lora-pager/model.json) |
| 32 | [LILYGO T-Embed CC1101 · V1.0](../../web/platform/knowledge/lilygo-t-embed-cc1101.json) | 2024-07-29 · 硬件 | 原有部分支持 · esp32s3 | 320×170 ST7789V | [官方 CAD 部件](../../web/platform/assets/lilygo-t-embed-cc1101/model.json) |
| 33 | [LILYGO T-TWR REV2.1](../../web/platform/knowledge/lilygo-t-twr-rev2-1.json) | 2024-07-29 · 上架代用 | 诊断 · esp32s3 | 128×64 SH1106 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-twr-rev2-1/model.json) |
| 34 | [LILYGO T-ETH ELite](../../web/platform/knowledge/lilygo-t-eth-elite-1.json) | 2024-07-25 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-eth-elite-1/model.json) |
| 35 | [LILYGO T-Knob](../../web/platform/knowledge/lilygo-t-knob.json) | 2024-07-13 · 上架代用 | 诊断 · esp32c6 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-knob/model.json) |
| 36 | [LILYGO T-Beam 1W](../../web/platform/knowledge/lilygo-t-beam-1w.json) | 2024-06-15 · 硬件 | 诊断 · esp32s3 | 128×64 SH1106 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-beam-1w/model.json) |
| 37 | [LILYGO T-Energy S3](../../web/platform/knowledge/lilygo-t-energy-s3.json) | 2024-06-13 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-energy-s3/model.json) |
| 38 | [LILYGO T-Glass](../../web/platform/knowledge/lilygo-t-glass.json) | 2024-06-04 · 上架代用 | 诊断 · esp32s3 | 126×126 JD9613 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-glass/model.json) |
| 39 | [LILYGO T-SIM7670G S3](../../web/platform/knowledge/lilygo-t-sim-7670g-s3.json) | 2024-04-12 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-sim-7670g-s3/model.json) |
| 40 | [LILYGO T-Display S3 AMOLED (DS)](../../web/platform/knowledge/lilygo-t-display-s3-amoled-1-64.json) | 2024-04-08 · 硬件 | 诊断 · esp32s3 | 280×456 CO5300 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-display-s3-amoled-1-64/model.json) |
| 41 | [LILYGO T3 S3 LR1121](../../web/platform/knowledge/lilygo-t3-s3-lr1121.json) | 2024-03-25 · 硬件 | 诊断 · esp32s3 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t3-s3-lr1121/model.json) |
| 42 | [LILYGO T-Lora C6](../../web/platform/knowledge/lilygo-t-lora-c6.json) | 2024-03-15 · 硬件 | 诊断 · esp32c6 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-lora-c6/model.json) |
| 43 | [LILYGO T-Micro32 S3](../../web/platform/knowledge/lilygo-t-micro32-s3.json) | 2024-03-11 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-micro32-s3/model.json) |
| 44 | [LILYGO T7-C6](../../web/platform/knowledge/lilygo-t7-c6.json) | 2024-02-11 · 硬件 | 诊断 · esp32c6 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t7-c6/model.json) |
| 45 | [LILYGO T-Encoder Pro](../../web/platform/knowledge/lilygo-t-encoder-pro.json) | 2024-02-02 · 硬件 | 诊断 · esp32s3 | 390×390 CO5300 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-encoder-pro/model.json) |
| 46 | [LILYGO T-Display S3 AMOLED Plus](../../web/platform/knowledge/lilygo-t-display-s3-amoled-plus.json) | 2024-01-01 · 硬件 | 诊断 · esp32s3 | 240×536 RM67162 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-display-s3-amoled-plus/model.json) |
| 47 | [LILYGO T-Watch S3 Plus](../../web/platform/knowledge/lilygo-t-watch-s3-plus.json) | 2024-01-01 · 硬件 | 诊断 · esp32s3 | 240×240 ST7789 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-watch-s3-plus/model.json) |
| 48 | [LILYGO T-QT C6](../../web/platform/knowledge/lilygo-t-qt-c6.json) | 2023-12-20 · 硬件 | 诊断 · esp32c6 | 128×128 GC9107 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-qt-c6/model.json) |
| 49 | [LILYGO T3 S3 E-paper](../../web/platform/knowledge/lilygo-ts-s3-epaper.json) | 2023-12-16 · 硬件 | 诊断 · esp32s3 | 122×250 SSD1680 | [照片轮廓粗模](../../web/platform/assets/lilygo-ts-s3-epaper/model.json) |
| 50 | [LILYGO ESP32 S2](../../web/platform/knowledge/lilygo-esp32-s2.json) | 2023-11-25 · 上架代用 | 诊断 · esp32s2 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-esp32-s2/model.json) |
| 51 | [LILYGO ESP32 S2 WOOR](../../web/platform/knowledge/lilygo-esp32-s2-woor.json) | 2023-11-25 · 上架代用 | 诊断 · esp32s2 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-esp32-s2-woor/model.json) |
| 52 | [LILYGO T-Panel S3](../../web/platform/knowledge/lilygo-t-panel-s3.json) | 2023-11-23 · 硬件 | 诊断 · esp32s3 | 480×480 ST7701S | [官方 CAD 部件](../../web/platform/assets/lilygo-t-panel-s3/model.json) |
| 53 | [LILYGO LoRa V1.3](../../web/platform/knowledge/lilygo-lora-v1-3.json) | 2023-11-18 · 上架代用 | 诊断 · esp32 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-lora-v1-3/model.json) |
| 54 | [LILYGO T-RSS3](../../web/platform/knowledge/lilygo-t-rs-s3.json) | 2023-11-14 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-rs-s3/model.json) |
| 55 | [LILYGO T-TV](../../web/platform/knowledge/lilygo-t-tv.json) | 2023-11-13 · 上架代用 | 诊断 · esp32 | 128×64 SSD1309 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-tv/model.json) |
| 56 | [LILYGO T-Mini E paper S3](../../web/platform/knowledge/lilygo-t-mini-epaper-s3.json) | 2023-11-10 · 硬件 | 诊断 · esp32s3 | 80×128 UC8175 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-mini-epaper-s3/model.json) |
| 57 | [LILYGO T4 S3](../../web/platform/knowledge/lilygo-t4-s3.json) | 2023-11-03 · 上架代用 | 诊断 · esp32s3 | 450×600 RM690B0 | [官方 CAD 部件](../../web/platform/assets/lilygo-t4-s3/model.json) |
| 58 | [LILYGO T-Display S3 Long](../../web/platform/knowledge/lilygo-t-display-s3-long.json) | 2023-10-27 · 上架代用 | 诊断 · esp32s3 | 180×640 AXS15231B | [照片轮廓粗模](../../web/platform/assets/lilygo-t-display-s3-long/model.json) |
| 59 | [LILYGO T-Camera Plus S3](../../web/platform/knowledge/lilygo-t-camera-plus-s3.json) | 2023-10-23 · 硬件 | 诊断 · esp32s3 | 240×240 ST7789 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-camera-plus-s3/model.json) |
| 60 | [LILYGO T-POE Pro](../../web/platform/knowledge/lilygo-t-poe-pro.json) | 2023-10-19 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-poe-pro/model.json) |
| 61 | [LILYGO T-Beam SoftRF](../../web/platform/knowledge/lilygo-t-beam-softrf.json) | 2023-10-13 · 上架代用 | 诊断 · esp32 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-beam-softrf/model.json) |
| 62 | [LILYGO T-ETH-Lite](../../web/platform/knowledge/lilygo-t-eth-lite.json) | 2023-09-09 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-eth-lite/model.json) |
| 63 | [LILYGO TTGO T-Display · 4MB](../../web/platform/knowledge/lilygo-ttgo-t-display.json) | 2023-08-04 · 上架代用 | 原有部分支持 · esp32 | 135×240 ST7789V | [官方 CAD 部件](../../web/platform/assets/lilygo-ttgo-t-display/model.json) |
| 64 | [LILYGO T-Beam SUPREME](../../web/platform/knowledge/lilygo-t-beam-supreme.json) | 2023-08-01 · 上架代用 | 诊断 · esp32s3 | 128×64 SH1106 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-beam-supreme/model.json) |
| 65 | [LILYGO T-Display S3 Pro](../../web/platform/knowledge/lilygo-t-display-s3-pro.json) | 2023-08-01 · 硬件 | 诊断 · esp32s3 | 222×480 ST7796 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-display-s3-pro/model.json) |
| 66 | [LILYGO T-Beam Helium](../../web/platform/knowledge/lilygo-helium-t-beam.json) | 2023-07-25 · 上架代用 | 诊断 · esp32 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-helium-t-beam/model.json) |
| 67 | [LILYGO T-Vending](../../web/platform/knowledge/lilygo-t-vending.json) | 2023-06-23 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-vending/model.json) |
| 68 | [LILYGO T-TWR Plus](../../web/platform/knowledge/lilygo-t-twr-plus.json) | 2023-06-16 · 上架代用 | 诊断 · esp32s3 | 128×64 SH1106 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-twr-plus/model.json) |
| 69 | [LILYGO T-Watch S3 · SX1262](../../web/platform/knowledge/lilygo-t-watch-s3.json) | 2023-05-31 · 上架代用 | 诊断 · esp32s3 | 240×240 ST7789 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-watch-s3/model.json) |
| 70 | [LILYGO T-Deck · SX1262](../../web/platform/knowledge/lilygo-t-deck.json) | 2023-05-26 · 上架代用 | 原有部分支持 · esp32s3 | 320×240 ST7789 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-deck/model.json) |
| 71 | [LILYGO T-Display keyboard](../../web/platform/knowledge/lilygo-t-display-keyboard.json) | 2023-05-26 · 上架代用 | 诊断 · esp32 | 135×240 ST7789 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-display-keyboard/model.json) |
| 72 | [LILYGO T-A7608](../../web/platform/knowledge/lilygo-t-a7608e-h.json) | 2023-05-24 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-a7608e-h/model.json) |
| 73 | [LILYGO T-Relay S3](../../web/platform/knowledge/lilygo-t-relay-s3.json) | 2023-05-13 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-relay-s3/model.json) |
| 74 | [LILYGO T-Track](../../web/platform/knowledge/lilygo-t-track.json) | 2023-04-28 · 上架代用 | 诊断 · esp32s3 | 294×126 JD9613 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-track/model.json) |
| 75 | [LILYGO T3 S3](../../web/platform/knowledge/lilygo-t3s3-v1-0.json) | 2023-04-18 · 上架代用 | 诊断 · esp32s3 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t3s3-v1-0/model.json) |
| 76 | [LILYGO T-RGB](../../web/platform/knowledge/lilygo-t-rgb.json) | 2023-04-17 · 上架代用 | 诊断 · esp32s3 | 480×480 ST7701S | [照片轮廓粗模](../../web/platform/assets/lilygo-t-rgb/model.json) |
| 77 | [LILYGO T-Display S3 AMOLED](../../web/platform/knowledge/lilygo-t-display-s3-amoled.json) | 2023-04-14 · 上架代用 | 诊断 · esp32s3 | 240×536 RM67162 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-display-s3-amoled/model.json) |
| 78 | [LILYGO T-FPGA](../../web/platform/knowledge/lilygo-t-fpga.json) | 2023-04-14 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-fpga/model.json) |
| 79 | [LILYGO T-SIM7080G S3](../../web/platform/knowledge/lilygo-t-sim7080-s3.json) | 2023-02-09 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-sim7080-s3/model.json) |
| 80 | [LILYGO T-HMI](../../web/platform/knowledge/lilygo-t-hmi.json) | 2023-02-02 · 上架代用 | 诊断 · esp32s3 | 240×320 ST7789 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-hmi/model.json) |
| 81 | [LILYGO T-Touch Bar](../../web/platform/knowledge/lilygo-t-touch-bar.json) | 2023-02-02 · 上架代用 | 诊断 · esp32s3 | 588×126 JD9613 × 2；2 片合成调试画布 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-touch-bar/model.json) |
| 82 | [LILYGO T-TWR/ SOLD OUT](../../web/platform/knowledge/lilygo-t-twr.json) | 2023-01-12 · 上架代用 | 诊断 · esp32s3 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-twr/model.json) |
| 83 | [LILYGO T-Camera S3](../../web/platform/knowledge/lilygo-t-camera-s3.json) | 2022-12-28 · 上架代用 | 诊断 · esp32s3 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-camera-s3/model.json) |
| 84 | [LILYGO T-A7670E/G/SA R2](../../web/platform/knowledge/lilygo-t-sim-a7670e.json) | 2022-12-02 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-sim-a7670e/model.json) |
| 85 | [LILYGO T5-2.13inch E-paper](../../web/platform/knowledge/lilygo-t5-2-13inch-e-paper.json) | 2022-11-25 · 上架代用 | 诊断 · esp32 | 104×212 GDEW0213T5 manufacturer four-gray command set | [照片轮廓粗模](../../web/platform/assets/lilygo-t5-2-13inch-e-paper/model.json) |
| 86 | [LILYGO Watch-Keyboard-C3 V1.0](../../web/platform/knowledge/lilygo-watch-keyboard-c3-v1-0.json) | 2022-11-25 · 上架代用 | 诊断 · esp32c3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-watch-keyboard-c3-v1-0/model.json) |
| 87 | [LILYGO T-CAN485](../../web/platform/knowledge/lilygo-t-can485.json) | 2022-11-23 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-can485/model.json) |
| 88 | [LILYGO T-SIM7000E](../../web/platform/knowledge/lilygo-t-sim7600e.json) | 2022-11-22 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-sim7600e/model.json) |
| 89 | [LILYGO T-PCIE](../../web/platform/knowledge/lilygo-a-t-pcie.json) | 2022-11-16 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-a-t-pcie/model.json) |
| 90 | [LILYGO T-Watch 2021](../../web/platform/knowledge/lilygo-t-watch-2021.json) | 2022-11-16 · 上架代用 | 诊断 · esp32 | 240×240 GC9A01 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-watch-2021/model.json) |
| 91 | [LILYGO T-Weigh](../../web/platform/knowledge/lilygo-t-weigh.json) | 2022-11-16 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-weigh/model.json) |
| 92 | [LILYGO T-Wrist E-paper 1.54 Inch Display](../../web/platform/knowledge/lilygo-t-wrist-e-paper-1-54-inch-display.json) | 2022-11-16 · 上架代用 | 诊断 · esp32 | 200×200 SSD1681 / DEPG0150BN | [官方 CAD 部件](../../web/platform/assets/lilygo-t-wrist-e-paper-1-54-inch-display/model.json) |
| 93 | [LILYGO T5s 2.7 inch E-Paper](../../web/platform/knowledge/lilygo-t5s-2-7inch-e-paper.json) | 2022-11-16 · 上架代用 | 诊断 · esp32 | 176×264 IL91874 / GDEW027W3 | [照片轮廓粗模](../../web/platform/assets/lilygo-t5s-2-7inch-e-paper/model.json) |
| 94 | [LILYGO T7 V1.3 MINI 32 ESP32](../../web/platform/knowledge/lilygo-t7-v1-3-mini-32-esp32.json) | 2022-11-16 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t7-v1-3-mini-32-esp32/model.json) |
| 95 | [LILYGO T-Display AMOLED Lite](../../web/platform/knowledge/lilygo-t-display-amoled-lite.json) | 2022-11-12 · 上架代用 | 诊断 · esp32s3 | 368×194 SH8501 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-display-amoled-lite/model.json) |
| 96 | [LILYGO T7 S3](../../web/platform/knowledge/lilygo-t7-s3.json) | 2022-11-03 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t7-s3/model.json) |
| 97 | [LILYGO ESP32 OLED V3.0](../../web/platform/knowledge/lilygo-esp32-oled-v3-0.json) | 2022-10-25 · 上架代用 | 诊断 · esp32 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-esp32-oled-v3-0/model.json) |
| 98 | [LILYGO Mini E-Paper](../../web/platform/knowledge/lilygo-mini-e-paper-core.json) | 2022-10-25 · 上架代用 | 诊断 · esp32 | 80×128 UC8175 / GDEW0102T4 | [官方 CAD 部件](../../web/platform/assets/lilygo-mini-e-paper-core/model.json) |
| 99 | [LILYGO T-Journal](../../web/platform/knowledge/lilygo-t-journal.json) | 2022-10-25 · 上架代用 | 诊断 · esp32 | 128×32 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-journal/model.json) |
| 100 | [LILYGO T-Lion](../../web/platform/knowledge/lilygo-t-lion.json) | 2022-10-25 · 上架代用 | 诊断 · esp32 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-lion/model.json) |
| 101 | [LILYGO T-Lite W5500](../../web/platform/knowledge/lilygo-t-lite-w5500.json) | 2022-10-25 · 上架代用 | 诊断 · esp32 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-lite-w5500/model.json) |
| 102 | [LILYGO T-Relay 5V 8 Channel Relay](../../web/platform/knowledge/lilygo-t-relay-5v-8-channel-relay.json) | 2022-10-25 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-relay-5v-8-channel-relay/model.json) |
| 103 | [LILYGO T-SIMCAM](../../web/platform/knowledge/lilygo-t-simcam.json) | 2022-10-25 · 上架代用 | 诊断 · esp32s3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-simcam/model.json) |
| 104 | [LILYGO T-Dongle S3](../../web/platform/knowledge/lilygo-t-dongle-s3.json) | 2022-10-20 · 上架代用 | 诊断 · esp32s3 | 80×160 ST7735 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-dongle-s3/model.json) |
| 105 | [LILYGO T5 e-Paper](../../web/platform/knowledge/lilygo-t5-4-7-inch-e-paper-v2-3.json) | 2022-10-07 · 上架代用 | 诊断 · esp32s3 | 960×540 ED047TC1 raw source/gate panel | [照片轮廓粗模](../../web/platform/assets/lilygo-t5-4-7-inch-e-paper-v2-3/model.json) |
| 106 | [LILYGO T-Embed](../../web/platform/knowledge/lilygo-t-embed.json) | 2022-09-27 · 上架代用 | 诊断 · esp32s3 | 170×320 ST7789 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-embed/model.json) |
| 107 | [LILYGO T-QT Pro](../../web/platform/knowledge/lilygo-t-qt-pro.json) | 2022-09-21 · 上架代用 | 诊断 · esp32s3 | 128×128 GC9A01 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-qt-pro/model.json) |
| 108 | [LILYGO T-Higrow](../../web/platform/knowledge/lilygo-t-higrow.json) | 2022-09-19 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-higrow/model.json) |
| 109 | [LILYGO T-SIM7600E-L1](../../web/platform/knowledge/lilygo-t-sim7600e-l1.json) | 2022-09-19 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-sim7600e-l1/model.json) |
| 110 | [LILYGO T-SIM7600E/G/SA-H](../../web/platform/knowledge/lilygo-t-sim7600.json) | 2022-09-19 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-sim7600/model.json) |
| 111 | [LILYGO T-Encoder](../../web/platform/knowledge/lilygo-t-encoder-esp32.json) | 2022-09-13 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-encoder-esp32/model.json) |
| 112 | [LILYGO T-01 C3](../../web/platform/knowledge/lilygo-t-01c3.json) | 2022-09-06 · 上架代用 | 诊断 · esp32c3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-01c3/model.json) |
| 113 | [LILYGO LoRa32 V1.0 Kit/ Sold Out](../../web/platform/knowledge/lilygo-lora-v1-0-kit.json) | 2022-08-24 · 上架代用 | 诊断 · esp32 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-lora-v1-0-kit/model.json) |
| 114 | [LILYGO T-Internet-COM](../../web/platform/knowledge/lilygo-t-internet-com.json) | 2022-08-24 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-internet-com/model.json) |
| 115 | [LILYGO T-SIM7070G](../../web/platform/knowledge/lilygo-t-sim7070g.json) | 2022-08-17 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-sim7070g/model.json) |
| 116 | [LILYGO T8](../../web/platform/knowledge/lilygo-t8-c3.json) | 2022-08-17 · 上架代用 | 诊断 · esp32c3 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t8-c3/model.json) |
| 117 | [LILYGO T-Zigbee](../../web/platform/knowledge/lilygo-t-zigbee-esp32-c3-tlsr8258.json) | 2022-08-15 · 上架代用 | 诊断 · esp32c3 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-zigbee-esp32-c3-tlsr8258/model.json) |
| 118 | [LILYGO T-Relay](../../web/platform/knowledge/lilygo-t-relay.json) | 2022-08-12 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-relay/model.json) |
| 119 | [LILYGO T-SIM7000G](../../web/platform/knowledge/lilygo-t-sim7000g.json) | 2022-07-25 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [官方 CAD 部件](../../web/platform/assets/lilygo-t-sim7000g/model.json) |
| 120 | [LILYGO T-Call](../../web/platform/knowledge/lilygo-t-call-v1-4.json) | 2022-07-22 · 上架代用 | 诊断 · esp32 | 无内置屏幕 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-call-v1-4/model.json) |
| 121 | [LILYGO T-Beam V1.2 · SX1276](../../web/platform/knowledge/lilygo-t-beam-v1-2.json) | 2022-07-18 · 上架代用 | 原有部分支持 · esp32 | 未确认 | [原有有限照片模型](../../web/platform/assets/lilygo-t-beam-v1-2/model.json) |
| 122 | [LILYGO T-Dongle](../../web/platform/knowledge/lilygo-t-dongle-esp32-s2-1-14-inch-lcd-board.json) | 2022-07-18 · 上架代用 | 诊断 · esp32s2 | 135×240 ST7789 | [照片轮廓粗模](../../web/platform/assets/lilygo-t-dongle-esp32-s2-1-14-inch-lcd-board/model.json) |
| 123 | [LILYGO T3 LoRa32 V1.6.1](../../web/platform/knowledge/lilygo-lora3.json) | 2022-07-18 · 上架代用 | 诊断 · esp32 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-lora3/model.json) |
| 124 | [LILYGO T-Display-S3 · 非触摸版](../../web/platform/knowledge/lilygo-t-display-s3.json) | 2022-07-15 · 上架代用 | 原有部分支持 · esp32s3 | 170×320 ST7789V | [官方 CAD 部件](../../web/platform/assets/lilygo-t-display-s3/model.json) |
| 125 | [LILYGO T-Pico](../../web/platform/knowledge/lilygo-t-pico.json) | 未知 | 诊断 · esp32c6 | 222×480 ST7796S（实体屏幕预览，尚无帧输出） | [照片轮廓粗模](../../web/platform/assets/lilygo-t-pico/model.json) |
| 126 | [LILYGO T3 S3 Meshtastic](../../web/platform/knowledge/lilygo-t3-s3-meshtastic.json) | 未知 | 诊断 · esp32s3 | 128×64 SSD1306 | [照片轮廓粗模](../../web/platform/assets/lilygo-t3-s3-meshtastic/model.json) |
| 127 | [LILYGO T5 V2.3.1](../../web/platform/knowledge/lilygo-t5-v2-3-1.json) | 未知 | 诊断 · esp32 | 122×250 SSD1680 / DEPG0213BN | [官方 CAD 部件](../../web/platform/assets/lilygo-t5-v2-3-1/model.json) |

## 验证与发布

分开记录原生 CPU／外设测试、生产 WASM、公共 Worker 输入、目录／模型合约、真实 Chromium 渲染与性能比较。测试只证明所运行的程序和路径；新条目不会因此获得“出厂固件已验证”标记。

| 检查 | 结果 |
| --- | --- |
| 目录与知识资料 | passed · 219 products / 10 accessories / 178 firmware entries / 122 watchlist entries; knowledge tests passed |
| 工作区 Clippy | passed · final native source; no compiler or manifest warnings |
| WASM release Clippy | passed · final WASM source; warnings denied |
| 生产 WASM | passed · 6572710 bytes; no DWARF; name section retained |
| 真实 Worker / WASM 诊断 | passed · 8 tests passed; all 126 advertised boards plus bare S2 construct; actual CPU frames, input, UART, P4 XL9535 and S2 unsupported-instruction error propagation |
| 平台完整回归 | passed · 446 tests passed, 0 failed, 0 skipped; 66.79 seconds |
| 既有 WASM / Worker 场景 | passed · 14 CI manifest scenarios plus StopWatch passed; real worker pacing/channel/ACK/replacement/storage tests passed. These runs were concurrent with native compilation and are not performance measurements. |
| 补充 CI 契约与真实 ABI | passed · 126 unit tests, retained x5 receipt verification, timing/BLE ABIs, actual 18-event recording replay and StopWatch guest diagnostics passed; recording is not bit-exact. |
| 公开 Pages 目录打包 | passed · 2835 files / 773643448 bytes staged; seven internal StopWatch diagnostic artifacts excluded. Temporary staging copy removed after the check to reclaim build space. |
| P4 数字外设与复位 | passed · 5 tests passed; two full 8-bit I2C fractional clocks, BOOT state, XL9535 routes and SDMMC block DMA |
| 独立 S2 原生后端 | passed · 7 tests passed; ISA gate, aliased memory, image validation, polled UART, GPIO, software SPI, full frames and reset |
| 生产模型加载器 | passed · 122 new models; 44 CAD and 78 photograph outlines; no loader/source-hash failures |
| 模型独立重现 | passed · 122 models; 837/837 compared files byte-identical; 597 frozen input hashes unchanged; see lilygo-model-reproduction-2026-11-22.json |
| 证据隐私与来源审阅 | passed · Existing docs/evidence pattern gate: 2378 tracked files, 22 gzip, passed. Additional final review: 287 LilyGO JSON records / 170521 strings, plus 706 changed text files with 119 compressed files expanded, no private host paths, credentials or signed URLs found. Binary source provenance and model hashes were reviewed separately. |
| 原生 release 全工作区 | passed · 188 suites; 2358 passed, 0 failed, 0 ignored, 98 external-input cases filtered; release build 33m 18s after cache-space recovery |
| 原生可选调度器 | passed · 44 tests passed, 0 failed; explicit virtual-quantum mode rerun, not added to the full-workspace unique-test total. |
| WASM JIT 差分与波形 | passed · 111126 differential cases across 78 isolated groups and 24 separate GPIO waveform cases passed. The checked production web/wasm/esp32sim.wasm is unchanged. |
| WASM 分析配置编译 | passed · Release build with production flags and jit-profile,cpu-profile passed; production web/wasm/esp32sim.wasm unchanged. |
| 最终性能对照 | completed-with-known-increase · 80 measured runs plus 10 warmups match exact guest work/output. CPU median changes: S3 +5.348%, C3 -1.326%, C6 -2.452%, classic -8.453%, P4 +7.225%. P4 is slower in 8/8 pairs with disjoint ranges. Two scoped candidates failed to eliminate the increase and were reverted. Known native hello cost remains; no no-regression or browser-speed claim. |
| 真实 Chromium 渲染 | pending-ci · Pages CI must render all 127 models and execute actual C5/S2 input/frame probes before deployment. Local Chromium socket creation was denied and the escalation was rejected by automatic approval review; this is not recorded as a passed browser test. |

实际命令、产物哈希、性能条件和发布状态见 [集成验证回执](research/lilygo-integration-verification-2026-11-22.json)。

### 可复现维护入口

```sh
python3 tools/apply-lilygo-adaptations.py
python3 tools/sync-lilygo-model-runtime-hints.py
python3 tools/build-lilygo-expansion-models.py --output web/platform/assets
python3 tools/apply-lilygo-adaptations.py
python3 tools/report-lilygo-expansion-models.py
python3 tools/report-lilygo-adaptations.py
python3 tools/validate-platform-catalog.py
node tools/verify-lilygo-expansion-models.mjs
node --test tools/platform-*-test.mjs
```

模型重建的本次已验证依赖记录在 `tools/requirements-lilygo-expansion-models.txt`，逐文件重现结果见 [重现回执](research/lilygo-model-reproduction-2026-11-22.json)；以上流程不抓取或重分发新的出厂固件。最终 Rust、WASM、性能与浏览器命令以集成验证回执为准。
