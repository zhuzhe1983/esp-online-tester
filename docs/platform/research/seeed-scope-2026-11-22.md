# Seeed HMI/Display 完整商品范围核对

核对日期：2026-11-22。此记录扩展第一阶段的 13 款设备，不把已有执行证据自动扩展到新商品。机器可读身份、原始资料收据与固件调查见 [范围表](seeed-expanded-scope-2026-11-22.json)、[完整固件调查](seeed-display-factory-audit-2026-11-22.json) 和 [新增镜像布局](seeed-expanded-firmware-layouts-2026-11-22.json)。

## 范围与计数

目录共 **48 个产品身份**：23 个独立整机或主控板商品，加上 25 个官网可配置套件的明确板屏选项。25 个选项分别属于 4 个真实套件父 SKU，目录没有为选屏组合杜撰制造商 SKU。

第一阶段是 13 个带屏设备。此次新增 10 个独立商品和 25 个明确套件配置。独立商品中的 EE02、EE03、EE04、EE05 都内置 XIAO ESP32-S3 Plus，不能作为“需要另行研究的附件”排除；其裸板 SKU 没有屏幕，故 `display: null`，不会自动获得帧缓冲、屏幕 BUSY 或假屏幕内容。[官方电子纸产品索引](https://wiki.seeedstudio.com/seeed_epaper_displays/) 和各板的官方资料共同确认其 MCU 与接口。

用户给定的商店搜索结果依赖动态页面和实时库存，未取得完整静态结果。此次同时核对 Seeed 官方显示品类索引、具体商品页、商品 PDF、PCB 和固定提交的官方板屏目录。暂时缺货或停产的现有设备仍保留适配身份。发布日期仅使用明确的官方发布证据；资料修改日期、PCB 日期和类似日期的 SKU 不用作发布日期。未知日期设备保留未知，并优先展示带屏配置。

## 新增独立商品

| 目录身份 | 精确 SKU | 与已有设备的关系 |
|---|---|---|
| `seeed-xiao-epaper-ee02` | [100067144](https://www.seeedstudio.com/XIAO-ePaper-Display-Board-ESP32-S3-EE02-p-6639.html) | 自带 ESP32-S3 Plus；60-pin 外接屏接口；裸板无屏 |
| `seeed-xiao-epaper-ee03` | [100080566](https://www.seeedstudio.com/XIAO-ePaper-Display-Board-ESP32-S3-EE03-p-6638.html) | 自带 ESP32-S3 Plus 和 IT8951 显示路径；40-pin 接口；裸板无屏 |
| `seeed-xiao-epaper-ee04` | [100075670](https://www.seeedstudio.com/XIAO-ePaper-Display-Board-EE04-p-6560.html) | 自带 ESP32-S3 Plus；24/50-pin 接口；与旧 TRMNL 套件分开保留商品身份 |
| `seeed-xiao-epaper-ee05` | [100057220](https://www.seeedstudio.com/XIAO-ePaper-Display-Board-EE05-p-6755.html) | 自带 ESP32-S3 Plus；24-pin 接口；按钮 3 和电池使能接线不同于 EE04 |
| `seeed-sensecap-watcher-w1-a` | [113991315](https://www.seeedstudio.com/SenseCAP-Watcher-W1-A-p-5979.html) | 透明外壳 AI Vision 版；与 W1-B 共用有来源的显示输入板型 |
| `seeed-sensecap-watcher-xiaozhi` | [100051523](https://www.seeedstudio.com/SenseCAP-Watcher-XIAOZHI-EN-p-6532.html) | 独立 XiaoZhi 商业版本；不继承 AI Vision 恢复镜像或验证结果 |
| `seeed-sensecap-indicator-d1s` | [114993069](https://www.seeedstudio.com/SenseCAP-Indicator-D1S-p-5645.html) | D1S：空气质量传感器，无 LoRa |
| `seeed-sensecap-indicator-d1l` | [114993070](https://www.seeedstudio.com/SenseCAP-Indicator-D1L-p-5646.html) | D1L：SX1262 LoRa，不含 D1S 的传感器组合 |
| `seeed-sensecap-indicator-d1pro` | [114993071](https://www.seeedstudio.com/SenseCAP-Indicator-D1Pro-p-5644.html) | D1Pro：传感器和 LoRa |
| `seeed-sensecap-indicator-meshtastic` | [114993532](https://www.seeedstudio.com/SenseCAP-Indicator-D1L-for-Meshtastic-p-6304.html) | 独立预装 Meshtastic 商品；不以普通 D1L 出厂固件代替 |

Indicator 的版本划分以[官方四型号用户手册](https://files.seeedstudio.com/products/SenseCAP/SenseCAP_Indicator/SenseCAP%20Indicator%20User%20Manual_v2023.4.20.01.pdf)的明确表格为准。通用商品介绍里的可选功能不代表每个版本都配齐。D1S 保留无无线器件的显示输入板型；D1L、D1Pro 和 Meshtastic 版使用各自独立板型，并经实际扩展器与 SPI 接线提供 SX1262 数字子集。RP2040、空气质量传感器与物理 RF 仍未模拟，具体执行结果见 [LoRa 专项报告](../seeed-indicator-lora-runtime.md)。[官方原生固件说明](https://wiki.seeedstudio.com/SenseCAP_Indicator_Native_Firmware/)明确四个版本共用原生 UI，无对应传感器的版本显示 N/A。

原有条目也根据实际商品 PDF 核正精确 SKU：Watcher W1-B 为 113991275，Indicator D1 为 114993068，TRMNL 7.5 DIY Kit 为 104991005，XIAO ePaper 7.5 为 114993635，reTerminal E1003 为 100090602，E1004 为 100064156。它们的目录 ID 和历史验证范围保持不变。

## 25 种官方套件选项

父套件 SKU 分别为 EE02 的 [E26010501](https://www.seeedstudio.com/XIAO-ePaper-DIY-Kit-EE02-for-13-3-Spectratm-6-E-Ink.html)、EE03 的 [E26012301](https://www.seeedstudio.com/XIAO-ePaper-DIY-Kit-EE03-for-10-3-Monochrome-ePaper-Display.html)、EE04 的 [E25102101](https://www.seeedstudio.com/XIAO-ePaper-EE04-DIY-Bundle-Kit.html)、EE05 的 [E26042001](https://www.seeedstudio.com/XIAO-ePaper-DIY-Kit-ESP32-S3-EE05.html)。商品选项与[固定提交的官方 Hub 板屏目录](https://raw.githubusercontent.com/Seeed-Projects/OSHW-reTerminal-Series-E-D/8e03627d41f63b4133e606cb25c5c81b9d251494/web/js/firmwares.js)交叉核对如下。

| 面板选项 | 面板 SKU | 原生显示尺寸 | 确认的套件主控 | SenseCraft 1.1.5 完整恢复段 |
|---|---|---:|---|---|
| P133_SP6 | 100088646 | 1200 × 1600 | EE02 | 有 |
| P103_MONO | 100071485 | 1872 × 1404 | EE03 | 有 |
| P073_SP6 | 100064541 | 800 × 480 | EE04 | 有 |
| P075_MONO | 104990861 | 800 × 480 | EE04、EE05 | 有 |
| P0583_MONO | 104990858 | 648 × 480 | EE04、EE05 | 未取得 |
| P0426_MONO | 114993631 | 800 × 480 | EE04、EE05 | 有 |
| P042_MONO | 104990857 | 400 × 300 | EE04、EE05 | 未取得 |
| P029_MONO | 104990853 | 128 × 296 | EE04、EE05 | 有 |
| P029_QUAD | 104990855 | 128 × 296 | EE04、EE05 | 有 |
| P029_FLEX | 104990852 | 128 × 296 | EE04、EE05 | 未取得 |
| P0213_MONO | 104990845 | 122 × 250，可寻址行跨度 128 | EE04、EE05 | 有 |
| P0213_QUAD | 104990846 | 122 × 250，可寻址行跨度 128 | EE04、EE05 | 有 |
| P0213_FLEX | 104990847 | 104 × 212 | EE04、EE05 | 未取得 |
| P0154_MONO | 104990843 | 200 × 200 | EE04、EE05 | 有 |

EE04 有 12 种明确选屏组合，EE05 有 11 种，EE02 和 EE03 各 1 种。通用选型表中出现 10.3 或 13.3 英寸，不代表 EE04/EE05 实际商品选项或电气接口支持它们。原生传输方向与商品横竖描述分别记录，例如 P103 的 IT8951 原生尺寸是 1872 × 1404，不套用带整机安装方向的 E1003 帧旋转。

这 25 项明确标记为 `commercial-kit-selection`，分别保存父 SKU、主控板 SKU、面板 SKU 和来源 URL。屏幕模型为单独标识的外接调试屏；没有证据支持的壳体或整机装配不予虚构。

## 固件调查与边界

完整 48 项调查中，32 项有固定 SHA 的官方恢复镜像或恢复清单，8 项仅取得出厂软件资料，另 8 个明确屏幕组合未找到可取得的完整出厂镜像。这个计数是下载与身份核对结果，不是 32 项均已通过模拟器或实机验证。

新增 17 个 SenseCraft 板屏恢复清单均包含四段：`0x0` bootloader、`0x8000` 分区表、`0xe000` boot_app0、`0x90000` 应用。所有 68 段均实际下载、校验长度与 SHA，并解析应用芯片与分区布局。官方清单未提供 SPIFFS 内容分区，因此不能承诺资源挂载、联网配置或全部页面完成。各板与屏幕的完整身份必须匹配，不能只按 ESP32-S3 推断兼容。

余下 8 个组合在官方 Hub 有 Hello ePaper 选项，但实际解析出的 1.0.0 manifest 请求返回 404 或 503。目录将其记录为官方示例资料，不捏造下载链接、SHA 或应用镜像，也不改用其他面板程序。

Watcher XiaoZhi 的官方英文恢复镜像已实际取得，应用描述为 `xiaozhi 2.0.4`、ESP-IDF `v5.5.1`、芯片 ID 9。完整 merged 文件从 `0x0` 加载，应用位于 `0x200000`，与 AI Vision 1.1.7 的 `0x110000` 不同。厂商独有身份分区为空。该镜像作为有固定 SHA 的恢复项目入口；所有出厂批次的具体版本仍未知，不把恢复版号当作统一出厂版号。

Meshtastic 商品的预装软件家族由[专用官方说明](https://wiki.seeedstudio.com/sensecap_indicator_meshtastic/)确认，其中 Keyboard 段明确记录出厂版本为 **v2.5.11**；精确构建后缀、镜像哈希及后续各批次版本仍未核定。普通 D1L 的恢复镜像没有分配给这个商品。官方 Flasher 页面和脚本可以取得，脚本指向的版本 API、当前指定版本 manifest 本次公开 HTTP 请求返回 403。随后通过已连接 GitHub 的只读发布 API 取得官方资产链接，并实际下载了 2.5.11.8e2a3e5 的 ESP32-S3 发布包。目录将精确 Indicator 完整镜像、S3 OTA 辅助镜像、LittleFS 分别固定在 `0x0`、`0x260000`、`0x300000`，作为与文档版号相同的上游恢复项目默认；不声称等同每个出厂批次的构建。旧通用安装脚本漏了 Indicator 的 S3 文件名判断，此缺陷与实际芯片 ID、分区依据分别记录。公开 HTTP 失败仍保留，不能将连接器取得元信息描述成匿名 API 全面恢复。

每个新增商品同时记录 UIFlow1、UIFlow2 和 XiaoZhi 的查阅结果。泛用 XIAO ESP32-S3 的 UIFlow2 支持不证明所选显示屏或整机外设兼容；未找到精确目标的条目保留“未找到”，不会解释成绝不可能支持。独立 Watcher XiaoZhi 商业版则有明确的官方恢复镜像。

## 有意保留的资料冲突

- **EE05 电池使能。** 官方 V1.11 PCB 的 U3 pad 16 接 ADC_EN，符号引脚 D12 对应 GPIO39；GPIO6 是外露 D5/I²C 信号。当前 SenseCraft common config 写 GPIO6。板模型按 GPIO39 接线，不通过虚构连线掩盖出厂固件行为。
- **2.13 英寸柔性屏。** SKU104990847 对应 GDEW0213I5FD、104 × 212、UC8151D。官方 PCN 宣布 2025-12-18 停产且没有替代；商店仍列选项，目录保留已有设备身份，不声称仍在生产。
- **UC8151D 局部窗口。** [芯片数据手册](https://files.seeedstudio.com/wiki/Other_Display/213-flexible/IC%20Driver%20UC8151D.pdf)第 26 页规定 `0x90` 参数为 7 字节，固定版本 GFX2 `setAddrWindow` 当前发送 9 字节。模拟器按真实 7 字节协议实现；官方全刷路径不经过该局部命令。
- **2.13 英寸四色控制器名。** GDEY0213F51 在 GFX2 中使用 `JD79676` 兼容驱动，在 Seeed GxEPD2 中标为 `JD79661`。目录区分兼容驱动名称和待确认的硅片身份。
- **EE03 套件标题中的 Touch。** 独立面板 SKU100071485 与 EE03 电气资料未确证触控器件和引脚路径。物理触控保留待核实，当前运行配置不提供触摸输入；不能把 E1003 的触控板型直接套入。

## 实际排除理由

Wio Terminal 的 SAMD51、原版 reTerminal/DM 的 Raspberry Pi/CM4，以及 nRF52840 IPS/EN/OpenDisplay 产品，主控不属于 ESP32。WirelessTag、Waveshare、M5Stack、LilyGO 虽可能在 Seeed 商店出售，品牌不属于本次 Seeed 目标。

Round Display SKU104030087 和不含 MCU 的 ePaper breakout/XIAO eInk 扩展附件自身没有 ESP32 主控。Round Display 的[官方入门说明](https://wiki.seeedstudio.com/get_start_round_display/)要求另购 XIAO；本次官方目录核对未找到固定出售的 Seeed ESP32 + Round Display 整机套件。此排除针对该无 MCU 商品，不否认用户可以自行连接 ESP32。纯 XIAO 处理器开发板属于 MCU/XIAO 品类，已明确作为 EE 主控板或显示套件组成部分的商品则全部收录。

## 证据与验收

每个已知知识事实至少关联一份实际取得的官方原文。收据保留最终 URL、UTC、字节数和 SHA-256；失败或仅链接来源不伪造抓取时间和原文哈希。源码固定提交、制造商 PDF、原始 PCB ZIP、固件二进制均按各自真实下载内容登记。初次扩展检查核对 265 份不同原始 HTTP 内容；新增精确无线板型的来源后，[最终目录检查](../evidence/seeed-final-integration-20261122/catalog-and-sources.json)已逐份核验 **267 份原始 HTTP 内容**。

本扩展的结构检查覆盖 48 项身份、4 项无屏裸板、25 项允许的组合、17 个精确恢复布局、共享板型与 manifest 一致性、版本间固件隔离以及 EE05 接线冲突。全局知识结构校验通过 275 个记录；Seeed 原文校验与通用全品牌原文资料目录是两个范围。新增 35 项的英文名称标签已加入，现有控制与接口翻译测试通过。

29 个新增 EE 板型以及六个 SenseCAP 商业版本均以部分适配声明，实际运行结果由后续执行收据单独记录。原有 13 款的历史 WASM、镜像哈希、帧和输入证据只对其明确设备及版本有效；新增目录身份不继承别的 SKU 的验证标记。没有进行实机验证。

恢复包共有 56 个成员，下载为 62,741,576 字节，目录声明展开总量为 99,880,682 字节。ZIP 合同因此明确分离为下载不超过 64 MiB、成员不超过 64、目录声明展开总量不超过 128 MiB；实际仅解压清单指定成员，仍执行板级大小、CRC、SHA 与路径等检查。三个所选原始成员均通过实际解包与 SHA 核验。
