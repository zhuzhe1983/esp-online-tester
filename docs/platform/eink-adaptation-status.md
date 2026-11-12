# 电子墨水屏产品适配清单

2026-11-12。当前目录有 **20 项**电子墨水屏产品：8 项具有可执行的部分板级模型，12 项尚无可执行板型。`partial` 表示列明的固件和功能经过验证，不代表整机、任意固件或实物显示效果完全兼容。目录身份和运行入口以 `web/platform/catalog/products.json` 为准。

当前优先处理 **PaperS3、PaperMono、PaperMono-Lite、普通 INK4**；验收范围为基础功能、联网及显示正确性。PaperS3 另外指定 [EDC Book](https://edcbook.cn/)。其余产品保留现状，不继续扩展适配。

## 优先四款

| 产品 | 已有基础能力 | 本轮新增或补充验收 | 主要待完成项 |
| --- | --- | --- | --- |
| [PaperS3 C139](papers3-runtime.md) | 官方 UserDemo v0.5、540×960 黑白帧、GT911 单点触摸、BMI270 虚拟输入、SPI TF | 官方固件写出 `TEST.TXT` 并保持复位/重载数据，RTC 递增、图标逐点比较和 Wi-Fi 扫描通过；[EDCBook 3.1.2](papers3-edcbook.md) 原始 BIN 已启动，首页、触屏阅读和前后翻页通过 | EDCBook 返回同页仍有 388/518,400 字形像素差异，TF/联网未验收；应用联网、16 灰阶/光学校准、蜂鸣器、PMS150G 电源和 RTC 唤醒 |
| [PaperMono C153](papermono-runtime.md) | 官方 1.2.0、480×800 显示、A/B、触摸、TF、RGB；NFC 无卡扫描及 LoRa 显式虚拟对端 | BMI270 首次初始化、姿态/角速度读数及恢复；四灰阶全部 384,000 像素和局刷恢复吻合；关联、DHCP、双向 SNTP 与 TIME UPDATED 通过 | RX8130 自主走时、音频、深睡、实物坐标/光学标定；大 BMP 栈溢出待排查 |
| [PaperMono-Lite C153-Lite](papermono-runtime.md) | 与完整版独立的板型和存储身份；显示、A/B、触摸、TF、RGB | 独立验证 BMI270、四灰阶/局刷恢复、按键/触摸及 TF；实际关联、DHCP、双向 SNTP 与 TIME UPDATED 通过 | RX8130 自主走时、音频、深睡与实物标定；Lite 没有 NFC/LoRa；EDC Book 未独立验收 |
| [Metalio INK4](ink4-runtime.md) | 官方 2.0.53、480×800 黑白显示、触摸、主页/音量等语义键 | 开放网页 TF；官方挂载、读文件/写目录、复位和浏览器重载持久化；真实录制回放；修正连续局刷旧字/旧页残留，自动配网及实际 OTA 响应通过 | 账号激活与语音未完成；电量计、RTC、IMU、音频及 4G 缺失；普通版结论不适用于 Plus |

详细收据：[PaperS3 显示](papers3-display.md)、[PaperMono/Lite 显示和联网](evidence/papermono-network-display/README.md)、[INK4 运行范围](ink4-runtime.md)。四款均保持 `partial`。PaperMono/Lite 的完整灰阶比较基于包含 SSD1677 修复的构建；最终构建再次验收完整版灰阶，以及两型号的输入和联网，构建身份分别记录。

物理屏幕的电压波形、温度、残影和刷新时长仍缺少实机标定。验收区分固件真实像素、数字控制器语义与实际光学效果，不用“画面发生变化”代替正确性证明。联网同样区分助手连接、设备取得地址和应用收到响应。

## 其余四款有部分运行支持，本轮暂停

以下沿用既有产品证据，没有在本轮重新跑全部固件。

| 产品 | 已验证范围 | 主要缺项 |
| --- | --- | --- |
| [PaperDIY K159-B](paperdiy-runtime.md) | 官方 1.0 显示和 A/B；保留首次关机标志及再次启动行为 | 无触摸硬件；TF、完整电源流程等 |
| [PaperColor C151](papercolor-runtime.md) | 官方 1.0.1 彩色启动画面、重启后的 AP 二维码、C 键长按及 RGB | TF、RTC 计时、完整电源/音频/网络 |
| [Air Quality K131](airquality-runtime.md) | 官方 0.2.0、200×200 显示、A/B 设置/AP、虚拟 SCD40/SEN55 样本 | 真实传感过程、完整联网及整机电源；固件保持试运行标识 |
| [Air Quality v1.1 K131-V11](airquality-v11-runtime.md) | 独立验收官方 0.2.0 与浏览器输入回放 | RGB、完整电源/网络；不沿用原版硬件假设 |

## 尚无可执行板型的十二项

| 产品 | 当前程度 |
| --- | --- |
| M5Stack Paper v1.1 K049-B | 目录/外观资料；运行不可用 |
| M5Stack CoreInk K048 | 目录/外观资料；运行不可用 |
| M5Stack Xiaozhi Card Kit K146 | 目录/外观资料；运行不可用 |
| Metalio INK4 Plus | 外观资料及独立 S31 研究；尚未完成产品板型，运行不可用 |
| LILYGO T3 S3 E-paper | 产品身份资料；运行不可用 |
| LILYGO T5 e-Paper | 产品身份资料；运行不可用 |
| LILYGO T5 E-Paper S3 Pro | 产品身份资料；运行不可用 |
| LILYGO T5-2.13inch E-paper | 产品身份资料；运行不可用 |
| LILYGO T5s 2.7 inch E-Paper | 产品身份资料；运行不可用 |
| LILYGO T-Mini E paper S3 | 产品身份资料；运行不可用 |
| LILYGO T-Wrist E-paper 1.54 Inch Display | 产品身份资料；运行不可用 |
| LILYGO Mini E-Paper | 产品身份资料；运行不可用 |

这些条目的菜单、尺寸资料或模型预览不构成固件执行证据。S31 的两个旧工作树尚有未完成的架构迁移，保留原分支和目录，不以合并实验核心作为 INK4 Plus 已适配的依据。
