# LILYGO Spark 商品清单与标识审计

固定仓库提交 `0fd728e800d9ebaa5a15515b49aa6244f3453340` 的 `firmware_manifest_v2.json` 列出 **23 个分组、151 个商品键**。其中 **127 个商品明确包含 ESP32**，收录到静态产品目录的 **21 个分组**；其余 24 条保留排除原因。原先六款具体硬件配置及稳定本地 ID、模型、引脚、容量和出厂默认项保持原值，只补充系列和 provider 身份。新增 121 条仅完成商品收录，屏幕、容量、硬件版次、固件和运行状态均未冒充已核验。

这是一份固定版本的 Spark 商品清单，不能解释为 LILYGO 官网在抓取日的完整在售库存。manifest 的 `generated_at` 是 `2026-06-26T08:16:10.974Z`；仓库提交时间为 `2026-09-05T15:03:40Z`，实际抓取时间为 `2026-10-28T05:17:33Z`。三者分别保留，不用抓取时间覆盖资料年龄。

## 商品标识与系列

- 权威商品键是 `product_list[].products[].product_id`；系列键是 `product_list[].id`。本次取得的 manifest 与四份类型、schema、结构说明均未发现 UUID 字段，因此没有生成所谓产品 UUID。该结论只覆盖已取得的文件，不推断其他 Spark 服务的数据结构。
- `family: {id, name}` 用实际 group.id 分组，名称中的 Series 显示为“系列”。系列不是设备，也没有运行条目。
- `providers["lilygo-spark"]` 保存 `productId`、`groupId`、`mappingScope` 和固定 `catalogRevision`。`mappingScope: hardware-variant` 表示原六款本地配置比上游商品入口更具体；`product` 表示商品层级收录。两者都不证明固件兼容。
- 例如 Spark `t-beam` 映射到已有 `lilygo-t-beam-v1-2`，仍限 V1.2、AXP2101、SX1276 配置；`t-display-s3` 仍限本地非触摸版本。没有把商品页的其他选项并入这些配置。
- T-Deck 六个商品分别存在：原版、Plus、Pro，以及各自的 Meshtastic 预装商品版。预装版的 `firmwareEdition` 只是商品标签，不继承其他条目的硬件参数、镜像、模型或出厂默认值。T-Deck 原版的 SX1262 配置保持不变。
- `Watch-Keyboard-C3 V1.0` 明确含 ESP32-C3，以可编程键盘商品收录；这不表示已完成手表附件连接。`T-Pico` 等多 MCU 商品保留 MCU 原文，不假定哪个芯片是主控，也不合并成单芯片模拟器。

## 未知项与适配顺序

新增商品保留 `runtime: {board: null, status: "unavailable", flashMb: null, psramMb: null}`。`display: null` 配 `displayStatus: "unknown"` 表示屏幕待核对；空 controls/interfaces 表示尚未建立输入与引脚配置，不表示真实设备没有这些功能。新条目不设置 defaultFirmwareId，不借用同系列模型或通用固件。源码及商品页链接仅标记 linked，不能冒充已抓取或已核验硬件来源。

Spark 摘要存在已知错误，例如 T-Display-S3 的显示总线摘要与精确硬件资料不符。本轮仅保留其商品名、ID、MCU 声明与来源入口，不从营销摘要填入 Flash、PSRAM、尺寸、引脚或显示参数。知识记录明确逐项待核验，出厂固件调查目前止于商品目录入口。

后续先继续完成[原六款优先队列](lilygo-priority-2026-10-28.md)，再按其中的后续顺序逐款核验。目录原有六款保持先前的兴趣线索顺序；新增商品随后按固定 Spark 分组和商品顺序列出。该顺序不是销量或运行兼容性排名。

## 分组覆盖

| Spark group.id | 分组原名 | 源商品 | 收录 | 排除 |
| --- | --- | ---: | ---: | ---: |
| `t-display-series` | T-Display Series | 14 | 13 | 1 |
| `t-deck-series` | T-Deck Series | 6 | 6 | 0 |
| `t-beam-series` | T-Beam Series | 9 | 8 | 1 |
| `t3-series` | T3 (LoRa) Series | 9 | 8 | 1 |
| `t-lora-series` | T-Lora Series | 4 | 4 | 0 |
| `t-watch-series` | T-Watch Series | 4 | 4 | 0 |
| `t-embed-series` | T-Embed Series | 4 | 4 | 0 |
| `t-camera-series` | T-Camera Series | 2 | 2 | 0 |
| `t-connect-series` | T-Connect Series | 2 | 2 | 0 |
| `t-echo-series` | T-Echo Series | 5 | 0 | 5 |
| `t-twr-series` | T-TWR Series | 3 | 3 | 0 |
| `t-sim-series` | T-SIM / T-PCIE Series | 11 | 10 | 1 |
| `t-relay-series` | T-Relay Series | 3 | 3 | 0 |
| `t-dongle-series` | T-Dongle Series | 3 | 3 | 0 |
| `t-eth-series` | T-ETH Series | 2 | 2 | 0 |
| `t-encoder-series` | T-Encoder Series | 2 | 2 | 0 |
| `t7-series` | T7 Series | 4 | 4 | 0 |
| `t5-series` | T5 Series | 5 | 5 | 0 |
| `t8-series` | T8 Series | 1 | 1 | 0 |
| `t4-series` | T4 Series | 1 | 1 | 0 |
| `other-t-series` | Other T- Products | 39 | 35 | 4 |
| `other-products` | Other Products | 10 | 7 | 3 |
| `general-esp32-series` | General ESP32 (Espressif) | 8 | 0 | 8 |

## 已收录商品

完整机器映射见 [mapping JSON](lilygo-spark-product-mapping-2026-10-28.json)。表中的本地 ID 用于保存、分享和配置；Spark ID 用于上游资料查询。

| Spark product_id | 商品名 | 本地 catalog ID | 层级 |
| --- | --- | --- | --- |
| `t-display` | T-Display | `lilygo-ttgo-t-display` | 已核验具体硬件配置 |
| `t-display-amoled-lite` | T-Display AMOLED Lite | `lilygo-t-display-amoled-lite` | 商品，参数待核验 |
| `t-display-bar` | T-Display Bar | `lilygo-t-display-bar` | 商品，参数待核验 |
| `t-display-keyboard` | T-Display keyboard | `lilygo-t-display-keyboard` | 商品，参数待核验 |
| `t-display-p4` | T-Display P4 | `lilygo-t-display-p4` | 商品，参数待核验 |
| `t-display-s3` | T-Display S3 | `lilygo-t-display-s3` | 已核验具体硬件配置 |
| `t-display-s3-amoled` | T-Display S3 AMOLED | `lilygo-t-display-s3-amoled` | 商品，参数待核验 |
| `t-display-s3-amoled-1-64` | T-Display S3 AMOLED (DS) | `lilygo-t-display-s3-amoled-1-64` | 商品，参数待核验 |
| `t-display-s3-amoled-plus` | T-Display S3 AMOLED Plus | `lilygo-t-display-s3-amoled-plus` | 商品，参数待核验 |
| `t-display-s3-long` | T-Display S3 Long | `lilygo-t-display-s3-long` | 商品，参数待核验 |
| `t-display-s3-pro` | T-Display S3 Pro | `lilygo-t-display-s3-pro` | 商品，参数待核验 |
| `t-display-s3-pro-external` | T-Display S3 Pro External | `lilygo-t-display-s3-pro-external` | 商品，参数待核验 |
| `t-display-s3-pro-lr1121` | T-Display S3 Pro LR1121 | `lilygo-t-display-s3-pro-lr1121` | 商品，参数待核验 |
| `t-deck` | T-Deck | `lilygo-t-deck` | 已核验具体硬件配置 |
| `t-deck-meshtastic` | T-Deck Meshtastic | `lilygo-t-deck-meshtastic` | Meshtastic 预装商品版 |
| `t-deck-plus-1` | T-Deck Plus | `lilygo-t-deck-plus-1` | 商品，参数待核验 |
| `t-deck-plus-meshtastic` | T-Deck Plus Meshtastic | `lilygo-t-deck-plus-meshtastic` | Meshtastic 预装商品版 |
| `t-deck-pro` | T-Deck Pro | `lilygo-t-deck-pro` | 商品，参数待核验 |
| `t-deck-pro-meshtastic` | T-Deck Pro Meshtastic | `lilygo-t-deck-pro-meshtastic` | Meshtastic 预装商品版 |
| `t-beam` | T-Beam | `lilygo-t-beam-v1-2` | 已核验具体硬件配置 |
| `t-beam-1w` | T-Beam 1W | `lilygo-t-beam-1w` | 商品，参数待核验 |
| `t-beam-bpf` | T-Beam BPF | `lilygo-t-beam-bpf` | 商品，参数待核验 |
| `helium-t-beam` | T-Beam Helium | `lilygo-helium-t-beam` | 商品，参数待核验 |
| `t-beam-meshtastic` | T-Beam Meshtastic | `lilygo-t-beam-meshtastic` | Meshtastic 预装商品版 |
| `t-beam-softrf` | T-Beam SoftRF | `lilygo-t-beam-softrf` | SoftRF 预装商品版 |
| `t-beam-supreme` | T-Beam SUPREME | `lilygo-t-beam-supreme` | 商品，参数待核验 |
| `t-beam-supreme-meshtastic` | T-Beam SUPREME Meshtastic | `lilygo-t-beam-supreme-meshtastic` | Meshtastic 预装商品版 |
| `lora3` | T3 LoRa32 V1.6.1 | `lilygo-lora3` | 商品，参数待核验 |
| `t3s3-v1-0` | T3 S3 | `lilygo-t3s3-v1-0` | 商品，参数待核验 |
| `ts-s3-epaper` | T3 S3 E-paper | `lilygo-ts-s3-epaper` | 商品，参数待核验 |
| `t3-s3-lr1121` | T3 S3 LR1121 | `lilygo-t3-s3-lr1121` | 商品，参数待核验 |
| `t3-s3-meshtastic` | T3 S3 Meshtastic | `lilygo-t3-s3-meshtastic` | Meshtastic 预装商品版 |
| `t3-s3-v1-3` | T3 S3 V1.3 | `lilygo-t3-s3-v1-3` | 商品，参数待核验 |
| `t3-s3-mvsr` | T3-S3 MVSR | `lilygo-t3-s3-mvsr` | 商品，参数待核验 |
| `t3-tcxo` | T3 TCXO | `lilygo-t3-tcxo` | 商品，参数待核验 |
| `t-lora-c6` | T-Lora C6 | `lilygo-t-lora-c6` | 商品，参数待核验 |
| `t-lora-dual` | T-Lora Dual | `lilygo-t-lora-dual` | 商品，参数待核验 |
| `t-lora-pager` | T-Lora Pager | `lilygo-t-lora-pager` | 商品，参数待核验 |
| `t-lora-pager-meshtastic` | T-Lora Pager Meshtastic | `lilygo-t-lora-pager-meshtastic` | Meshtastic / MeshCore 预装商品版 |
| `t-watch-2021` | T-Watch 2021 | `lilygo-t-watch-2021` | 商品，参数待核验 |
| `t-watch-s3` | T-Watch S3 | `lilygo-t-watch-s3` | 已核验具体硬件配置 |
| `t-watch-s3-plus` | T-Watch S3 Plus | `lilygo-t-watch-s3-plus` | 商品，参数待核验 |
| `t-watch-ultra` | T-Watch Ultra | `lilygo-t-watch-ultra` | 商品，参数待核验 |
| `t-embed` | T-Embed | `lilygo-t-embed` | 商品，参数待核验 |
| `t-embed-cc1101` | T-Embed CC1101 | `lilygo-t-embed-cc1101` | 已核验具体硬件配置 |
| `t-embed-cc1101-plus` | T-Embed CC1101 Plus | `lilygo-t-embed-cc1101-plus` | 商品，参数待核验 |
| `t-embed-si4732` | T-Embed SI4732 | `lilygo-t-embed-si4732` | 商品，参数待核验 |
| `t-camera-plus-s3` | T-Camera Plus S3 | `lilygo-t-camera-plus-s3` | 商品，参数待核验 |
| `t-camera-s3` | T-Camera S3 | `lilygo-t-camera-s3` | 商品，参数待核验 |
| `t-connect` | T-Connect | `lilygo-t-connect` | 商品，参数待核验 |
| `t-connect-pro` | T-Connect Pro | `lilygo-t-connect-pro` | 商品，参数待核验 |
| `t-twr` | T-TWR/ SOLD OUT | `lilygo-t-twr` | 商品，参数待核验 |
| `t-twr-plus` | T-TWR Plus | `lilygo-t-twr-plus` | 商品，参数待核验 |
| `t-twr-rev2-1` | T-TWR REV2.1 | `lilygo-t-twr-rev2-1` | 商品，参数待核验 |
| `a-t-pcie` | T-PCIE | `lilygo-a-t-pcie` | 商品，参数待核验 |
| `t-sim-a7670e` | T-A7670E/G/SA R2 | `lilygo-t-sim-a7670e` | 商品，参数待核验 |
| `t-sim7600e` | T-SIM7000E | `lilygo-t-sim7600e` | 商品，参数待核验 |
| `t-sim7000g` | T-SIM7000G | `lilygo-t-sim7000g` | 商品，参数待核验 |
| `t-sim7070g` | T-SIM7070G | `lilygo-t-sim7070g` | 商品，参数待核验 |
| `t-sim7080-s3` | T-SIM7080G S3 | `lilygo-t-sim7080-s3` | 商品，参数待核验 |
| `t-sim7600e-l1` | T-SIM7600E-L1 | `lilygo-t-sim7600e-l1` | 商品，参数待核验 |
| `t-sim7600` | T-SIM7600E/G/SA-H | `lilygo-t-sim7600` | 商品，参数待核验 |
| `t-sim-7670g-s3` | T-SIM7670G S3 | `lilygo-t-sim-7670g-s3` | 商品，参数待核验 |
| `t-simcam` | T-SIMCAM | `lilygo-t-simcam` | 商品，参数待核验 |
| `t-relay` | T-Relay | `lilygo-t-relay` | 商品，参数待核验 |
| `t-relay-5v-8-channel-relay` | T-Relay 5V 8 Channel Relay | `lilygo-t-relay-5v-8-channel-relay` | 商品，参数待核验 |
| `t-relay-s3` | T-Relay S3 | `lilygo-t-relay-s3` | 商品，参数待核验 |
| `t-dongle-esp32-s2-1-14-inch-lcd-board` | T-Dongle | `lilygo-t-dongle-esp32-s2-1-14-inch-lcd-board` | 商品，参数待核验 |
| `t-dongle-c5` | T-Dongle C5 | `lilygo-t-dongle-c5` | 商品，参数待核验 |
| `t-dongle-s3` | T-Dongle S3 | `lilygo-t-dongle-s3` | 商品，参数待核验 |
| `t-eth-elite-1` | T-ETH ELite | `lilygo-t-eth-elite-1` | 商品，参数待核验 |
| `t-eth-lite` | T-ETH-Lite | `lilygo-t-eth-lite` | 商品，参数待核验 |
| `t-encoder-esp32` | T-Encoder | `lilygo-t-encoder-esp32` | 商品，参数待核验 |
| `t-encoder-pro` | T-Encoder Pro | `lilygo-t-encoder-pro` | 商品，参数待核验 |
| `t7-s3` | T7 S3 | `lilygo-t7-s3` | 商品，参数待核验 |
| `t7-v1-3-mini-32-esp32` | T7 V1.3 MINI 32 ESP32 | `lilygo-t7-v1-3-mini-32-esp32` | 商品，参数待核验 |
| `t7-c5` | T7-C5 | `lilygo-t7-c5` | 商品，参数待核验 |
| `t7-c6` | T7-C6 | `lilygo-t7-c6` | 商品，参数待核验 |
| `t5-4-7-inch-e-paper-v2-3` | T5 e-Paper | `lilygo-t5-4-7-inch-e-paper-v2-3` | 商品，参数待核验 |
| `t5-e-paper-s3-pro` | T5 E-Paper S3 Pro | `lilygo-t5-e-paper-s3-pro` | 商品，参数待核验 |
| `t5-v2-3-1` | T5 V2.3.1 | `lilygo-t5-v2-3-1` | 商品，参数待核验 |
| `t5-2-13inch-e-paper` | T5-2.13inch E-paper | `lilygo-t5-2-13inch-e-paper` | 商品，参数待核验 |
| `t5s-2-7inch-e-paper` | T5s 2.7 inch E-Paper | `lilygo-t5s-2-7inch-e-paper` | 商品，参数待核验 |
| `t8-c3` | T8 | `lilygo-t8-c3` | 商品，参数待核验 |
| `t4-s3` | T4 S3 | `lilygo-t4-s3` | 商品，参数待核验 |
| `t-01c3` | T-01 C3 | `lilygo-t-01c3` | 商品，参数待核验 |
| `t-2can` | T-2CAN | `lilygo-t-2can` | 商品，参数待核验 |
| `t-a7608e-h` | T-A7608 | `lilygo-t-a7608e-h` | 商品，参数待核验 |
| `t-bao` | T-Bao | `lilygo-t-bao` | 商品，参数待核验 |
| `t-call-v1-4` | T-Call | `lilygo-t-call-v1-4` | 商品，参数待核验 |
| `t-can485` | T-CAN485 | `lilygo-t-can485` | 商品，参数待核验 |
| `t-circle-s3` | T-Circle | `lilygo-t-circle-s3` | 商品，参数待核验 |
| `t-energy-s3` | T-Energy S3 | `lilygo-t-energy-s3` | 商品，参数待核验 |
| `t-fpga` | T-FPGA | `lilygo-t-fpga` | 商品，参数待核验 |
| `t-glass` | T-Glass | `lilygo-t-glass` | 商品，参数待核验 |
| `t-higrow` | T-Higrow | `lilygo-t-higrow` | 商品，参数待核验 |
| `t-hmi` | T-HMI | `lilygo-t-hmi` | 商品，参数待核验 |
| `t-internet-com` | T-Internet-COM | `lilygo-t-internet-com` | 商品，参数待核验 |
| `t-journal` | T-Journal | `lilygo-t-journal` | 商品，参数待核验 |
| `t-keyboard-s3-pro` | T-Keyboard S3 Pro | `lilygo-t-keyboard-s3-pro` | 商品，参数待核验 |
| `t-knob` | T-Knob | `lilygo-t-knob` | 商品，参数待核验 |
| `t-lion` | T-Lion | `lilygo-t-lion` | 商品，参数待核验 |
| `t-lite-w5500` | T-Lite W5500 | `lilygo-t-lite-w5500` | 商品，参数待核验 |
| `t-micro32-s3` | T-Micro32 S3 | `lilygo-t-micro32-s3` | 商品，参数待核验 |
| `t-mini-epaper-s3` | T-Mini E paper S3 | `lilygo-t-mini-epaper-s3` | 商品，参数待核验 |
| `t-panel-s3` | T-Panel S3 | `lilygo-t-panel-s3` | 商品，参数待核验 |
| `t-pico` | T-Pico | `lilygo-t-pico` | 商品，参数待核验 |
| `t-poe-pro` | T-POE Pro | `lilygo-t-poe-pro` | 商品，参数待核验 |
| `t-qt-c6` | T-QT C6 | `lilygo-t-qt-c6` | 商品，参数待核验 |
| `t-qt-pro` | T-QT Pro | `lilygo-t-qt-pro` | 商品，参数待核验 |
| `t-radar` | T-Radar | `lilygo-t-radar` | 商品，参数待核验 |
| `t-rgb` | T-RGB | `lilygo-t-rgb` | 商品，参数待核验 |
| `t-rs-s3` | T-RSS3 | `lilygo-t-rs-s3` | 商品，参数待核验 |
| `t-touch-bar` | T-Touch Bar | `lilygo-t-touch-bar` | 商品，参数待核验 |
| `t-track` | T-Track | `lilygo-t-track` | 商品，参数待核验 |
| `t-tv` | T-TV | `lilygo-t-tv` | 商品，参数待核验 |
| `t-vending` | T-Vending | `lilygo-t-vending` | 商品，参数待核验 |
| `t-weigh` | T-Weigh | `lilygo-t-weigh` | 商品，参数待核验 |
| `t-wrist-e-paper-1-54-inch-display` | T-Wrist E-paper 1.54 Inch Display | `lilygo-t-wrist-e-paper-1-54-inch-display` | 商品，参数待核验 |
| `t-zigbee-esp32-c3-tlsr8258` | T-Zigbee | `lilygo-t-zigbee-esp32-c3-tlsr8258` | 商品，参数待核验 |
| `esp32-oled-v3-0` | ESP32 OLED V3.0 | `lilygo-esp32-oled-v3-0` | 商品，参数待核验 |
| `esp32-s2` | ESP32 S2 | `lilygo-esp32-s2` | 商品，参数待核验 |
| `esp32-s2-woor` | ESP32 S2 WOOR | `lilygo-esp32-s2-woor` | 商品，参数待核验 |
| `lora-v1-3` | LoRa V1.3 | `lilygo-lora-v1-3` | 商品，参数待核验 |
| `lora-v1-0-kit` | LoRa32 V1.0 Kit/ Sold Out | `lilygo-lora-v1-0-kit` | 商品，参数待核验 |
| `mini-e-paper-core` | Mini E-Paper | `lilygo-mini-e-paper-core` | 商品，参数待核验 |
| `watch-keyboard-c3-v1-0` | Watch-Keyboard-C3 V1.0 | `lilygo-watch-keyboard-c3-v1-0` | 商品，参数待核验 |

## 排除记录

| Spark product_id | 源 MCU | 原因 |
| --- | --- | --- |
| `t-display-k230` | K230 | Spark 标注 K230，不是 ESP32 系列产品。 |
| `t-beam-shiled-v1-0` | 未列 MCU | Spark 未列独立 ESP32 MCU；附件保留审计，不进入设备目录。 |
| `t3-stm32` | STM32WL55CCU6 | Spark 标注 STM32WL55CCU6，不是 ESP32 系列产品。 |
| `t-echo-lilygo` | nRF52840 | Spark 标注 nRF52840，不是 ESP32 系列产品。 |
| `t-echo-lite` | nRF52840 | Spark 标注 nRF52840，不是 ESP32 系列产品。 |
| `t-echo-meshtastic` | nRF52840 | Spark 标注 nRF52840，不是 ESP32 系列产品。 |
| `t-echo-plus` | nRF52840 | Spark 标注 nRF52840，不是 ESP32 系列产品。 |
| `t-echo-softrf-firmware` | nRF52840 | Spark 标注 nRF52840，不是 ESP32 系列产品。 |
| `lilygo--t-simhat-can-rs485-relay-5v` | 未列 MCU | Spark 未列独立 ESP32 MCU；附件保留审计，不进入设备目录。 |
| `t-0-99-inch-lcd` | 未列 MCU | Spark 未列独立 ESP32 MCU；附件保留审计，不进入设备目录。 |
| `t-bat` | 未列 MCU | Spark 未列独立 ESP32 MCU；附件保留审计，不进入设备目录。 |
| `t-bhi260ap` | 未列 MCU | Spark 未列独立 ESP32 MCU；附件保留审计，不进入设备目录。 |
| `t-icm-20948` | 未列 MCU | Spark 未列独立 ESP32 MCU；附件保留审计，不进入设备目录。 |
| `fs2112-speaker` | 未列 MCU | Spark 未列独立 ESP32 MCU；附件保留审计，不进入设备目录。 |
| `module17-revision-0-1e` | STM32 | Spark 标注 STM32，不是 ESP32 系列产品。 |
| `to-esp8266-oled-sh1106` | ESP8266 | Spark 标注 ESP8266，不是 ESP32 系列产品。 |
| `general-esp32` | ESP32 | 通用 Espressif 芯片入口，不是 LILYGO 商品。 |
| `general-esp32s2` | ESP32-S2 | 通用 Espressif 芯片入口，不是 LILYGO 商品。 |
| `general-esp32s3` | ESP32-S3 | 通用 Espressif 芯片入口，不是 LILYGO 商品。 |
| `general-esp32c3` | ESP32-C3 | 通用 Espressif 芯片入口，不是 LILYGO 商品。 |
| `general-esp32c6` | ESP32-C6 | 通用 Espressif 芯片入口，不是 LILYGO 商品。 |
| `general-esp32h2` | ESP32-H2 | 通用 Espressif 芯片入口，不是 LILYGO 商品。 |
| `general-esp32c2` | ESP32-C2 | 通用 Espressif 芯片入口，不是 LILYGO 商品。 |
| `general-esp32p4` | ESP32-P4 | 通用 Espressif 芯片入口，不是 LILYGO 商品。 |

## 来源、复现与验证

原始 manifest 的 SHA-256 为 `e2a6b0ee0c1531db400120b927b2b4f70b3e70fdb922814245fa173eaba8deb9`，共 436499 字节。固定 URL、UTC、最终 URL、字节数、SHA-256 和标识类型文件的独立收据均见[source receipt](lilygo-spark-product-source-receipt-2026-10-28.json)。[product source](lilygo-spark-product-source-2026-10-28.json) 仅保留商品标识、MCU 和入口链接；删除营销摘要、图片、固件记录、作者资料及 collections，并单独记录投影 hash。没有镜像第三方二进制或热链产品图片。

生成工具默认只创建缺少的商品与基础知识；既有硬件、模型和固件保持不动。使用固定投影离线复现；若要核对原文，先自行从收据中的固定 URL 下载 manifest，再传入路径。

```sh
python3 tools/import-lilygo-spark-catalog.py --check
python3 tools/import-lilygo-spark-catalog.py --check --verify-source /path/to/firmware_manifest_v2.json
python tools/validate-platform-catalog.py
node tools/platform-knowledge-test.mjs
node --test tools/platform-lilygo-catalog-test.mjs tools/platform-config-test.mjs tools/platform-catalog-loading-test.mjs tools/platform-lilygo-spark-test.mjs
node tools/check-evidence-privacy.mjs
```

目录检查覆盖 151 个来源键的完整核销、127 个唯一映射、24 个排除项、T-Deck 的六个独立商品、原六款的稳定身份及出厂默认项；所有 127 个设备均能创建、验证和编解码预览 scene。与变更前 `61e1808f` 的比较确认原六款完整 product 对象除 family/providers 外无变化。来源原文实际字节与投影 hash 已复核，生成器重复检查无差异。未对新增 121 个商品执行固件、模型或实机验证。
