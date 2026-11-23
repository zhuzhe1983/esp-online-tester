# M5Stack Core 系列固件来源复核

2026-11-23 核对了 Core / Basic / FIRE / M5GO 系列中此前待适配的 15 个型号及硬件版次。其中 9 个型号已有固定 CDN 出厂镜像，5 个型号由厂家提供 EasyLoader 恢复入口，Tough 未找到明确标为出厂恢复的发布项。已复核的镜像布局、来源、官方 UIFlow 和小智适配范围见下列记录。本轮验证范围是来源、版本、镜像格式和目录选择，固件运行与实机结果由各板型验收记录提供。 [逐型号记录](firmware-audit.json) · [来源收据](sources.json) · [镜像布局](layouts.json)

| 设备 | 出厂固件结论 | 官方替代项 |
| --- | --- | --- |
| Basic / Basic v2.7 | Core Factory Test v0.4。镜像内使用 4 MiB 布局，型号资料列出 16 MiB Flash | Core UIFlow1、UIFlow2 |
| FIRE v2.7 | 官方 EasyLoader 内嵌 16 MiB 布局镜像，网页目录提供厂家恢复入口 | Fire 专用 UIFlow1 v1.15.5、UIFlow2 v2.5.5，均为固定 CDN |
| M5GO IoT Kit v2.7 | 官方说明预装 UIFlow 示例。User Demo EasyLoader 内嵌 4 MiB 布局镜像，型号资料列出 16 MiB Flash | 厂家 Core UIFlow1、UIFlow2 适用 16 MiB 版 |
| Faces Kit | 官方键盘测试 EasyLoader 内嵌 4 MiB 布局镜像 | 套装的 Gray 主机选择匹配容量的 Core UIFlow。当前目录映射 16 MiB 主机 |
| Core2 / Core2 v1.1 | Core2FactoryTest v2.2，发布方明确列出这两版 | Core2 UIFlow1、UIFlow2 |
| Core2 v1.3 / Core2 For AWS v1.3 | 两个独立的官方 EasyLoader 均内嵌可校验的 ESP32 镜像。AWS v1.3 另有明确型号的 M5Burner v1.1 元信息，匿名下载要求验证码 | 厂家文档链接通用教程，项目版本仍需独立核对 |
| Core2 For AWS 原版 | Core2_For_AWS v0.0.1，固定 CDN | 厂家文档链接通用教程 |
| Tough | Tough_Tools v1.0.2 属于厂家测试工具。所查厂家产品页、M5Tools 和 M5Burner 目录未标注出厂恢复镜像 | Tough UIFlow1、UIFlow2 |
| CoreS3 / CoreS3-Lite | CoreS3 UserDemo v0.12，Lite 厂家页链接相同源码、安装器和恢复教程 | 厂家 CoreS3 UIFlow2、小智教程 |
| CoreS3-SE | CoreS3SE UserDemo v0.6，固定 CDN。较新的 v0.15 需核对实际内容 | 厂家 CoreS3 UIFlow2、小智教程 |
| CoreS3 Thread BR | 独立官方 v1.0，应用位于 0x20000 | 所查渠道未找到该型号的 UIFlow 或小智镜像 |

FIRE 和 Faces 的 UIFlow 条目跟踪具体 M5Burner 项目、设备分类和版本。已有可直连资产沿用 SHA-256 固定的厂家 CDN。各型号目录仍优先选择其原有出厂或恢复入口。4 MiB 版 Core UIFlow2 单独核验，Faces 新增的试运行项对应当前目录配置的 16 MiB Gray 主机。FIRE 专用项目与 Basic、Gray、M5GO 的 Core 项目各自使用独立映射。 [Fire UiFlow1](https://docs.m5stack.com/en/uiflow/m5fire/program) · [Fire UiFlow2](https://docs.m5stack.com/en/uiflow2/m5fire/program) · [Faces Kit](https://docs.m5stack.com/en/core/Faces_Kit)

## 复核方法

1. 按来源收据访问官网、恢复指南和发布清单。M5Burner 新接口按项目 ID 复核 `OFFICIAL`、设备分类和 `PUBLIC/PUBLISHED` 版本。旧接口的发布清单给出已公开 CDN 路径。两处记录的项目名、版本、目标容量和厂商发布身份均核对。小智检查厂家分发条目、上游板型目录和五个近期发布的资产名称。 [小智上游板型](https://github.com/78/xiaozhi-esp32/tree/main/main/boards/m5stack) · [小智上游发布](https://github.com/78/xiaozhi-esp32/releases)
2. 对公开 CDN 直链执行 GET，用 `shasum -a 256` 对比报告。EasyLoader 的本地复核按 `resourcePayloadOffset` 读取 `.NET` ByteArray，检查其前一个类型字节 `0x20` 和后续四字节小端长度。使用该长度提取资源，避免将安装器尾部或完整分区容量误算进固件。Faces 资源自 Flash `0x1000` 开始，验证用的本地合并缓冲区前缀填入 4096 字节的 `0xff`。分别记录容器、原始资源与本地合并缓冲区的 SHA-256。 [.NET 资源读取实现](https://github.com/dotnet/runtime/blob/main/src/libraries/System.Private.CoreLib/src/System/Resources/ResourceReader.cs)
3. 从报告中的偏移分别取 bootloader 和实际提供的应用分区，用 `esptool image-info` 5.6.0 检查芯片、校验和与镜像尾部摘要。用 `web/platform/runtime.js` 的 `validateMergedImage` 核对 17 份完整布局与配置容量。Basic 的空闲 OTA 槽不随镜像下发。FIRE 容器资源截至 `0x5fe000`，后续配置分区空间由设备 Flash 提供。
4. `tools/validate-platform-catalog.py` 与相关 Node 测试全部通过。额外确认 Basic v2.7、FIRE v2.7、M5GO v2.7、Faces 的默认 ID 和首项角色，以及 FIRE 与 Faces 新增项的固件和 watchlist 目标一致。 在 `a3c2f4d8` 的无头 Chromium 工作台切换这四款设备，默认项、备选列表和板型未接入时的运行入口行为均符合预期，无页面错误。

收据保留响应时间、地址、字节数和摘要。原始响应正文、下载请求标识和账号字段不随记录发布。授权门控只留下 HTTP 状态与公开错误码。所有 EXE、BIN 和临时提取物均保存在本地忽略目录。
