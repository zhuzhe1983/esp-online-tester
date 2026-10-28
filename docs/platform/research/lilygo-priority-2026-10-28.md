# LILYGO 首批设备与后续顺序

首批深度调查保留 6 款具体配置，继续作为硬件、模型与固件适配的优先队列。随后按用户要求先补齐 Spark 商品目录：127 个含 ESP32 的商品独立列入 21 个系列分组，见[产品清单与标识审计](lilygo-spark-product-inventory-2026-10-28.md)。这份首批调查不代表其余商品已经适配。排序使用 2026-10-28 抓取的官方 GitHub 仓库关注数作为社区兴趣线索，并兼顾显示板、LoRa、键盘终端、多功能手持设备和手表。Stars 不是销量；`LilyGo-LoRa-Series`、`LilyGoLib` 覆盖多种产品，不能解释为某个型号的独立热度。原始响应 hash、UTC、来源和逐设备结论见 [审计记录](lilygo-catalog-audit-2026-10-28.json)。

| 顺序 | 本轮配置 | 关注度来源 | Stars / forks |
| --- | --- | --- | --- |
| 1 | T-Display-S3 非触摸版 | [T-Display-S3](https://github.com/Xinyuan-LilyGO/T-Display-S3) | 1,206 / 274 |
| 2 | 原版 TTGO T-Display 4MB | [TTGO-T-Display](https://github.com/Xinyuan-LilyGO/TTGO-T-Display) | 1,200 / 361 |
| 3 | T-Beam V1.2，SX1276 868/915MHz | [LilyGo-LoRa-Series](https://github.com/Xinyuan-LilyGO/LilyGo-LoRa-Series) | 1,011 / 260，系列合计 |
| 4 | 原版 T-Deck，SX1262 868/915MHz | [T-Deck](https://github.com/Xinyuan-LilyGO/T-Deck) | 620 / 117 |
| 5 | 原版 T-Embed CC1101 V1.0 | [T-Embed-CC1101](https://github.com/Xinyuan-LilyGO/T-Embed-CC1101) | 531 / 72 |
| 6 | 原版 T-Watch S3，SX1262 | [LilyGoLib](https://github.com/Xinyuan-LilyGO/LilyGoLib) | 149 / 38，共享库；用作穿戴设备覆盖，非绝对热度排名 |

官网的 [T-Display 产品说明](https://wiki.lilygo.cc/products/t-display-series/t-display/) 与 [T-Beam 产品说明](https://wiki.lilygo.cc/products/t-beam-series/t-beam/) 分别介绍了显示仪表、便携 IoT 和 Meshtastic 等常见用途。参数仍以精确版本的一手仓库、原理图为准。T-Deck 的 wiki 混有 Plus 的 GNSS/电池描述并称无触摸，已采用明确适用于原版的 [硬件资料](https://github.com/Xinyuan-LilyGO/LilyGoLib/blob/57f6414e682ae124817ae0936f7b8251ac17d6c3/docs/hardware/lilygo-t-deck.md)；T-Beam V1.2 的 AXP2101 不沿用 wiki 的 AXP192。

## 后续候选队列

以下候选已按 Spark 商品标识收录为待核验条目；本表是后续深度调查次序，尚未完成热度量化和固件/模型验收。开始每款前重新比较具体仓库的关注度、厂家当前型号、精确硬件版本和可用 CAD，不将现有条目复制改名。

1. T-Deck Plus：键盘与 LoRa 生态延续，但要单独记录 GNSS 占用 Grove 的差异。
2. T-Embed CC1101 Plus：与原版有射频、接口及结构差异，固件和模型分别核对。
3. T-LoRa Pager：补充带屏 LoRa 手持设备，按 SX1262/LR1121 分版。
4. T-Display-S3 AMOLED：按屏幕尺寸、触摸和控制器分版，不能共用本轮非触摸 LCD 定义。
5. T-Beam Supreme：ESP32-S3、GNSS 与 PMU 拓扑重新核对，不能共用本轮 ESP32 V1.2 模型。
6. T5 4.7 / T5 e-Paper S3 Pro：先厘清芯片和屏幕版本，再找适用的出厂及阅读器固件。
7. T-Watch S3 Plus / Ultra：独立核验传感器、射频和外壳，保留批次差异。

## 固件来源与限制

每款均调查并固定了官方出厂测试/恢复镜像；六个出厂条目均优先于社区项目，作为各自默认项。另收录五个精确板型的 Bruce 1.16.1 和三个 Meshtastic 2.7.26 稳定版镜像。Bruce 使用同一个 `bruce` watchlist 条目，Meshtastic 扩展现有 `meshtastic` 条目，与 M5Stack 平铺共享。发布匹配须同时满足板型、芯片、格式与校验规则；不将 Meshtastic 文件名中的 `factory` 当成厂商出厂身份。

14 个镜像在入库审计阶段均只下载到进程内存，验证实际 SHA-256、芯片、容量、bootloader、分区和应用完整性，不提交二进制。后续集成检查尝试了原版 TTGO 出厂镜像，能显示启动 Logo，但发生持续看门狗复位，仍保留 trial；详见[集成结果](../lilygo-onboarding.md)。其余镜像未执行，未刷写真实硬件。元信息与布局审计可重复运行：

```sh
node tools/audit-lilygo-firmware.mjs
node --test tools/platform-config-test.mjs tools/platform-watchlist-test.mjs tools/platform-meshtastic-test.mjs tools/platform-firmware-sync-test.mjs
python tools/validate-platform-catalog.py
node tools/platform-knowledge-test.mjs
```

Bruce 的发布资产含 SHA-256，允许精确目标的后续元信息更新；Meshtastic 目标清单目前只提供 MD5，新版缺少 SHA-256 时保留本轮实测摘要和镜像，不把 MD5 伪装成 SHA-256。厂商仓库文件固定到提交，不因项目最新 Release 标签不同就替换。UIFlow 未找到本轮六款的官方精确映射；已检查小智 v2.5.2 发布资产，只有其他 LILYGO 变体，未借用相近型号。Launcher 留作来源调查，尚未加入未核验的可运行镜像。

本站板级模型、真实固件运行与视觉模型分别验收。原版 T-Display 可复用已有部分板级模型，其余五款当前明确为尚未适配；本记录不承诺任何新增固件的完整兼容性或实机可用性。
