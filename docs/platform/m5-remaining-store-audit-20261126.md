# M5Stack 全商店主控角色复核

固定的三页官方Shopify响应共有687个唯一商品，原始SHA-256和实际抓取时间全部校验。本轮先阅读363条原未审商品，再重新阅读原先已分类的324条主介绍、芯片/存储规格及真正的Includes范围。结果是资料角色筛查，并非逐块PCB/BOM、完整3D、外设协议或固件验收。

累计发现的五个主控/SKU配置缺口为 **U037、U138、U109-B、K115、K014-B**：

| SKU | 本体/配置事实 | 原误判或缺口 |
| --- | --- | --- |
| U037 | M5Camera-F，ESP32、4MB Flash、4MB PSRAM、OV2640、160° | 未审 |
| U138 | Unit PoESP32，ESP32-WROOM-32U、4MB Flash、IP101G | 未审；UART使用方式不能抹去本体ESP32 |
| U109-B | 带壳Unit Camera，ESP32-WROOM-32E、4MB、OV2640、65° | 标题Enclosure曾被当作纯壳配件 |
| K115 | Atom-PSRAM，ESP32-PICO-V3-02、8MB Flash、2MB PSRAM | 与C008 Atom-Lite的4MB/无PSRAM配置不同 |
| K014-B | 旧FIRE主机，16MB Flash、4MB PSRAM、ILI9341 | 当前K007系列目录均为ILI9342C，不应继承8MB配置 |

原95项非ESP控制器组中还找回7个明确包含ESP32主机的组合：K137、K033-C、K038、K038-B、K014-C、K014-E、K017-C。其STM32/Base-MCU规格属于电机或输入协处理器，不能覆盖主描述及Includes中明确供应的ESP32主机。原机械组的有源无线/定位/音频产品也按主介绍纠正，不再仅凭Antenna标题判断为机械件。K143是AX630C主平台并含ESP32-C6网卡，单列协处理器存在，不计为独立ESP32主控，也不称完全没有ESP32。

下表是44个增补含主机SKU映射（原23+363复核新增14+324复核新增7）。37条具名/同配置映射、5条主控版次待核、2条独立配置。SKU身份可查不代表外围附件已模拟；没有继承父条目的固件、出厂身份或执行结论。它们不是44种全新的芯片设计。历史 `missingConfirmedDeviceSkus` 是相对于最早91条目录的缺口队列，不是当前未完成数。

| 套装SKU | 包含主控 | 父SKU或候选 | 边界 |
| --- | --- | --- | --- |
| K137-V11 | AtomS3R | C126 | 主控配置映射；外围另审 |
| A145-V033 | Stamp-S3A | S007-PIN127-V033 | 照片确认1.27排针；完整父SKU为对应推断，套装壳为黑色 |
| A145 | Stamp-S3 PIN1.27 | S007-PIN127 | 主控配置映射；外围另审 |
| K059-B | AtomS3-Lite | C124 | 主控配置映射；外围另审 |
| K016-H2 | StickC Plus2 | K016-P2 | 主控配置映射；外围另审 |
| K041-B | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K023-B | Basic | K001/K001-V26/K001-V27 | 主控存在确证；版次/存储待核 |
| K010-SI | Core2 for AWS | K010-AWS | 主控配置映射；外围另审 |
| K115-B | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K119 | Basic | K001/K001-V26/K001-V27 | 主控存在确证；版次/存储待核 |
| K118 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K115 | Atom-PSRAM | 未知 | 独立配置；不可默继承父主控 |
| K016-H | StickC Plus | K016-P | 主控配置映射；外围另审 |
| K065 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K063 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K061 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K059 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K057 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K055 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K054 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K053 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K052 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K050 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K042-D | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K047 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K040 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K042 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K046 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K045 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K044 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K043 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K041 | Atom-Lite | C008 | 主控配置映射；外围另审 |
| K029 | StickC | K016-C | 主控配置映射；外围另审 |
| K016-F | StickC | K016-C | 主控配置映射；外围另审 |
| K023 | Basic | K001/K001-V26/K001-V27 | 主控存在确证；版次/存储待核 |
| K017 | M5Core | 未知 | 主控存在确证；版次/存储待核 |
| K014-B | FIRE | K007/K007-V26/K007-V27 | 独立配置；不可默继承父主控 |
| K137 | AtomS3 | C123 | 主控配置映射；外围另审 |
| K033-C | StickC Plus2 | K016-P2 | 主控配置映射；外围另审 |
| K014-E | FIRE | K007 | 命名主控+配置匹配；采购版次未锁定 |
| K038-B | StickC Plus | K016-P | 主控配置映射；外围另审 |
| K017-C | M5Core | 未知 | 主控存在确证；版次/存储待核 |
| K014-C | Gray | K002 | 命名主控+配置匹配；采购版次未锁定 |
| K038 | StickC | K016-C | 主控配置映射；外围另审 |

深查结论见 [主控版次复核](m5-host-revision-followup-20261126.md)。A145-V033精确套装高清图确认预焊1.27mm排针，对应S007-PIN127-V033配置；不能继承单卖橙色壳。Basic三套K023/K023-B/K119的照片、原理图与固定官方例程均未锁定供货Flash/LCD/主板版次。K017/K017-C的机器人和遥控器两个M5Core角色有固定源码支持，具体内存仍未声明。K014-C实际规格是Gray16MB/ILI9342C，未写4MB PSRAM或ILI9341；K014-E为FIRE16MB/8MB/ILI9342C，与旧K014-B分开。

A072商城标题写with-core，但官方正文、Includes与M5-Bus引脚仍只证实继电器基座和端子，保留来源身份冲突。A133/A133-B的ESP32在外接Atom；M039/M039-V11的ESP32在叠加M5主机；S006/A105中的ESP32是编程对象；K024/K024-B实验板、A106-V21 DTU基座、K039 Proto件不含已证实主机；K121没有StickC而K029主描述明确包含StickC。

机器记录：[363条](research/m5-remaining-store-audit-20261126.json)、[其余324条](research/m5-prior-store-audit-20261126.json)、[44条主控映射](research/m5-host-kit-mapping-20261126.json)、[687条全范围](research/m5-full-store-scope-20261126.json)。原始来源和补查的成功/失败收据均保留，无厂商已锁定供货版次的公开证据时明确阻断继承，而不从例程板型默认值或封闭外壳猜容量。

复现依次运行 `tools/audit-m5-store-remaining.py` 与 `tools/audit-m5-store-prior.py`，输入三份hash匹配的官方响应并采用记录的UTC。两工具均不联网、不下载固件、不修改目录或runtime。687条已做来源角色筛查不表示687款已完成适配。
