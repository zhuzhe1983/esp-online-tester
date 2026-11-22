# LilyGO S3 显示与控制适配

本批新增 19 个独立产品 ID 的 ESP32-S3 板级配置，按官方资料核实每块板的屏幕接口、GPIO、可见窗口和控制输入。实现位于 `esp32s3/src/board/lilygo_display.rs`。完整接线、硬件选择、来源版本、每个已抓取来源的 SHA-256 和已知限制见 [适配资料](research/lilygo-runtime-profiles-2026-11-22.json)。实现不把可渲染的模型当成固件运行证据。

## 产品范围与顺序

下表沿用全量来源审计的最新优先顺序，并采用两处新查到的硬件版本日期：Pro LR1121 所用 MVSRLora 背板 V1.0 为 2024-12-14，AMOLED 1.64 V1.0 为 2024-04-08。仅有商城上架日期的产品保留低置信度标记；它们不等同于已确认的硬件首发日期。

| 产品 | 模拟器原生画面 | 控制器 / 接口 | 本次输入范围 |
|---|---:|---|---|
| T-Display S3 Pro External | 222 × 480 | ST7796 / SPI | 三个按钮、CST226 单点触摸 |
| T-LoRa Pager Meshtastic | 222 × 480 | ST7796 / SPI | BOOT、旋钮与旋钮按压 |
| T-Embed CC1101 Plus | 170 × 320 | ST7789 / SPI | BOOT / 旋钮按压、用户按钮、旋钮 |
| T-Watch Ultra | 410 × 502 | CO5300 / QSPI | BOOT；XL9555 显示电源门控 |
| T-Display Bar | 76 × 284 | ST7789 / SPI | 两个按钮、CST816 兼容单点报告 |
| T-Embed SI4732 | 170 × 320 | ST7789 / SPI | BOOT / 旋钮按压、旋钮 |
| T-Display S3 Pro LR1121 | 222 × 480 | ST7796 / SPI | 主板按钮、CST226 单点触摸 |
| T-LoRa Pager | 222 × 480 | ST7796 / SPI | BOOT、旋钮与旋钮按压 |
| T-Display S3 AMOLED 1.64 | 280 × 456 | CO5300 / QSPI | BOOT |
| T-Watch S3 Plus | 240 × 240 | ST7789 / SPI | 带 BOOT / RST 外壳版本的 BOOT、FT6336 单点触摸 |
| T-Display S3 AMOLED Plus | 240 × 536 | RM67162 / SPI | BOOT、CST816 兼容单点报告 |
| T4 S3 | 450 × 600 | RM690B0 / QSPI | BOOT、CST226 单点触摸 |
| T-Display S3 Pro | 222 × 480 | ST7796 / SPI | 三个按钮、CST226 单点触摸 |
| T-Watch S3 | 240 × 240 | ST7789 / SPI | FT6336 单点触摸 |
| T-Display S3 AMOLED | 240 × 536 | RM67162 / QSPI | 带 GPIO38 显示使能的触摸版本，BOOT、CST816T 单点报告 |
| T-Display AMOLED Lite | 368 × 194 | SH8501 / QSPI | 两个按钮 |
| T-Dongle S3 | 80 × 160 | ST7735 / SPI | BOOT |
| T-Embed | 170 × 320 | ST7789 / SPI | BOOT / 旋钮按压、旋钮 |
| T-QT Pro | 128 × 128 | GC9A01 / SPI | 两个按钮；FN4R2 / 当前面板版本 |

Pager、Embed 和 Dongle 的板体通常横向使用；表中列出控制器的原生画面方向。AMOLED Lite 则按官方库的 368 × 194 横向寻址输出。前端模型应分别处理板体朝向，不能通过修改固件写入的像素来伪造方向。

## 实现行为

像素来自实际 SPI 数据及 DCS 地址窗口，不生成静态画面，也不按固件或产品名称预设结果。SPI 接收要求正确的 SCLK、MOSI / D0 与 CS 路由。普通 SPI 还要求有效的 D/C GPIO；具有 RESET 或显示使能脚的板必须先由固件配置并驱动对应 GPIO。显示关闭、休眠、复位、背光关闭时画面变黑，已有 GRAM 数据不作为新的显示初始化。

已实现 RGB565 地址写入、MADCTL 变换、RAMWR / RAMWRC、显示开关与休眠、复位，以及背光和 AMOLED 亮度的开关门控。18 / 24 位像素格式不会按 RGB565 强行解释。PWM 仅在具有该控制方式的配置中启用；AW9364 只支持稳态开关，未实现 16 级脉冲调光。

QSPI 解析厂商使用的 `02/32, 00, command, 00` 头部，支持软件 CS 下跨传输拆分的头部和像素流。CS 未释放时，即使像素数据恰好包含相同字节模式，也继续当作像素处理。共享 `SpiPins` 接口尚无 D1、D2、D3 的掩码，因此这些通道的接线和电气时序仍未核验。不同 AMOLED 控制器的模拟参数、厂商寄存器页、电源时序和 TE 同步不在本次范围内。

触摸只返回已实现协议的单点坐标与按压 / 释放状态，按照各板自己的 I²C 地址、SDA / SCL、IRQ 和 RESET 接线运行。没有伪造实测的触摸固件或校准版本。Bar 使用 CST816S 兼容报告子集；AMOLED 及 Plus 按官方声明选择 CST816T。两者都不证明所有 CST816 芯片变体兼容。Bar README 的 I²C 表标注有重复，采用 Factory/board.h 与实际 Factory.ino 的 SDA3 / SCL2 接线。Ultra 的 CST9217、AMOLED Lite 的 CHSC5816 和 AMOLED 1.64 的 FT3168 尚未实现。

Pro 官方 wiki 的 ST7789V2 描述与官方 `Setup214`、ST7796 驱动及商城描述冲突。本配置选择有明确引脚和裁剪窗口的 ST7796 驱动，不把另一种控制器标成已验证。Pro External、LR1121 背板、Pager Meshtastic 和 Embed SI4732 的共用部分均有厂商原文说明；只共用显示与输入，未沿用另一产品的无线、音频或电源能力。

## 验证证据与边界

运行以下检查：

```sh
cargo test -p esp32s3 lilygo_display --lib
cargo test -p esp32s3 --test lilygo_display
cargo clippy -p esp32s3 --lib --tests -- -D warnings
```

板级测试检查裁剪窗口、错误路由拒绝、复位和休眠、低电平有效背光、PWM 解绑、QSPI 拆分头部与连续像素、XL9555 门控、触摸报告、坐标边界、释放和触摸复位。它们还检查每个独立产品配置没有自动获得 SD、无线、音频、IMU 等外围能力。

额外的 Xtensa 指令探针使用真实 `s32i` 指令写 IO_MUX、GPIO 矩阵及 GP-SPI 寄存器。External Pro、Bar 和 AMOLED 1.64 的红色像素通过完整路径进入真实 framebuffer，错误 MOSI 路由不会改写画面，硬件复位后必须重新初始化。另一个探针通过网页输入队列按下 Pro 三个按钮，再用真实 `l32i` 指令读取 GPIO；暂停期间释放同样生效。

这些探针是本项目编写的最小指令诊断，不是厂商完整固件。尚未证明任何新增配置能完整运行出厂镜像，未提供对应完整固件兼容承诺。SD / SDMMC、LoRa / CC1101 / nRF24 / SI4732、充电及电池、音频、NFC、传感器、摄像头、GNSS 和 haptics 仍需独立适配。原始第三方固件未加入仓库。
