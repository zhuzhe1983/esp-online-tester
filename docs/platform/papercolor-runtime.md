# PaperColor C151 适配记录

当前为研究检查点，目录仍未启用。产品使用 ESP32-S3R8、16 MB Flash / 8 MB OPI PSRAM，400×600 Spectra 6 ED2208/EL040EF1 彩色纸屏；不能复用 PaperMono 的 SSD1677 或 PaperDIY 的并行协议。

[来源收据](evidence/papercolor/sources.json) 固定官方产品页、出厂使用说明、V0.5 原理图和 M5PM1 v1.9 手册。官方 UserDemo 源码 `1ff998e0cf3b9ce916f2e0319561d043db21cc4a` 使用 IDF 5.5.1，M5GFX 子模块 `5268353cffabd2925b87ed9bcc05ded4e22cc3d1`、M5Unified `46aa9ede24016a70f397884e8fb3c0ccc3a05269`。已下载现有目录中的官方 v1.0.1 完整镜像（16,777,216 字节），SHA-256 与目录匹配：`0f9c9c0f429cfae2c4ab39ca842bbddf7b1e6f8eb29bbfd2826cb9903f7ee37e`；包含 IDF 5.5.1 字符串。当前源码未证明逐字节对应发行镜像。

ED2208 使用 G15 CLK、G13 MOSI、G44 CS、G43 DC、G11 BUSY（低有效）、G12 RESET，电源为 M5PM1 PYG0。驱动配置中出现 `pin_rst=43`，但此前显式复位 G12；物理接线以原理图 G12 为准，不让 DC 写入错误地反复复位纸屏。原生 4bpp 依次映射黑0、白1、黄2、红3、蓝5、绿6；按 M5GFX 的理想调色板显示，不声称校准实物色彩或 15–30 秒光学波形。

内部 I²C 为 SDA3/SCL2：M5PM1 0x6e、RX8130 0x32、SHT40 0x44。后二者是 M5GFX 自动识别的必要探测目标。A/B/C 分别为 G10/G9/G1，无触摸屏。RGB 是 G21 的两颗串联 WS2812，电源由 M5PM1 LDO3V3_EN（PWR_CFG 位2，HOLD_CFG 位5）控制。TF 使用 SPI G15/G13/G14/CS47，PMIC PYG1 检测卡、PYG3 供电、PYG4 开启检测；当前计划明确报告无卡，不伪装成 SDMMC 支持。

官方首次启动会显示引导图、写入 NVS `papercolor/first_boot_done` 并关机；后续启动会进入 AP 配置。按 A 五秒后固件会闪白灯、短亮绿灯并请求再次启用 AP。网络门户、音频、TF、RTC 持续走时和真实电源域不能由纸屏画面推出已兼容。先实现 SPI RAM/refresh/busy、电源和实际按键/灯输出，运行未修改官方镜像后再启用目录。

已查看开放 PR；没有 ED2208/PaperColor 的重复实现。相邻 GP-SPI service PR #312 和 SPI1 #313 不在本轮更改范围。本轮不是 CPU 加速实验；若修改共享路径仍按仓库规范做完整回归与同指令性能对照。现有厂家 CAD 会独立重建并核对按钮、RGB 光窗与材质；保留已有 UNKNOWN PCB 和外观估算边界。
