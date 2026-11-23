# Chain Captain · C160 运行记录

`m5stack-c160`（ESP32-S3R8，16 MB Flash / 8 MB PSRAM 规格，1.54" 240×240 JD9853A）已建立独立板型 `chain-captain`，并用未改动的官方出厂镜像完成一次真实启动验收。当前状态 `partial`，不代表整机或任意固件可用。

## 板型实现了什么

`esp32s3/src/board/chain_captain.rs`：240×240 显示窗口（GPSPI2 经 M5IOE1 电源/复位门控）、G1/G4/G5 三只用户键、M5PM1 电源键单击/双击/长按时序、M5IOE1 PWM 红绿蓝、BMI270 I2C 子集、ES8311 控制寄存器子集与音频子开关门控。板型测试 10 项通过：`cargo test -p esp32s3 chain_captain`。

`runtime.js` 的板级按键白名单已加入 `chain-captain: ['power']`，电源键走 `{t:'board-button',id:'power',pressed:1}` 派发到 `bus.board().button_at()`。

## 官方固件验收

镜像 `C160-M5Chain-Captain-UserDemo-V0.2_0x0.bin`（2 571 488 B，SHA-256 `fe8404ad…c67b0`，发布于 2026-10-12），未打补丁，页面 worker 的实际 WASM JIT 后端 ROM 启动：guest 打印 `Project name: ChainCaptain-UserDemo`、`App version: V0.2`、`ESP-IDF v5.5.5`、`Found 8MB PSRAM device`，LVGL Launcher 与六个应用（Chain IoT、Pomodoro Timer、IMU Bubble Level、IR Copy、Recorder、Settings）出现，240×240 首帧非空、63 种颜色。M5PM1（0x6E @100 kHz）、M5IOE1、BMI270、RGB、IR、显示、ES8311 + 22 050 Hz mono I2S、`storage` 分区 FAT 挂载均由 guest 日志确认。

按键后果可观察：G4 打开 Chain IoT 并使 M5PM1 `Boost Enabled`；G5 触发 `HAL-CHAIN` 两路扫描，`Port A TX=7 RX=6`、`Port B TX=18 RX=17`，各 `received=0 bytes`；G1 只改变面板帧。未建模外设以 guest 自身错误暴露：`HAL-RTC: RX8130 flag/time read failed: ESP_ERR_INVALID_STATE`。

复现命令和板级限制见[运行证据](evidence/chain-captain/README.md)。合并后的 Chromium 工作台复验了依次操作 3D A 键和键盘 B 键，客体输出 `Boost Enabled`，并可在 mini 播放器打开同一配置。结果见[浏览器回执](evidence/chain-captain/browser-v02.json)。

## 保留的冲突与未知

- 官网 PinMap 记 `PORT.A` 黄 G6 (UART2_TX) / 白 G7 (UART2_RX)、`PORT.CHAIN` 黄 G18 (UART1_TX) / 白 G17 (UART1_RX)。出厂固件运行期用 `TX=7 RX=6` 驱动 Port A、`TX=18 RX=17` 驱动 Port B：Port B 与官网一致，Port A 方向相反。目录按官网登记，两侧说法都不改写。
- 镜像头声明 `SPI Flash Size : 8MB`，而 `spi_flash` 报“实际 16 384 k 大于头声明的 8 192 k，按头声明使用”。产品规格仍按厂家记 16 MB。
- Grove 口数量（原理图出现 GROVEA/GROVEB）、USB-C 数据脚归属、UART0 是否引出到 Ext.Port、JD9853A 的 GRAM 行数仍属未知，记录在 `web/platform/knowledge/m5stack-c160.json` 的 unknowns。
- 固件分发：GitHub release 资产没有 `Access-Control-Allow-Origin`，浏览器直载被 CORS 拒绝，只能本地导入。M5Burner 已登记 CHAIN_CAPTAIN 与 UserDemo V0.2、UIFlow2.0 Chain Captain，但下载端点返回 400（早前 401），按 `binFileName` 猜测的 CDN 路径 404，本轮未取得可直载的 Burner 文件。

## 3D 外观

厂家 [CAD](https://github.com/m5stack/M5_Hardware/blob/8231e759560ce63d864c8987b8ca12f9c09c825b/Products/C160_Chain_Captain/Structures/Chain_Captain.stl) 中选取原位的一对壳体和四枚键帽，移到场景中心。模型尺寸为 56 × 40 × 17 mm，正面和背面与厂家照片的轮廓交并比均约 0.99。屏幕单独显示固件的 240 × 240 像素输出。四枚键帽正面命中各自的输入区域，背面射线无法触发。外壳可分开查看，内部 PCB 以 `UNKNOWN` 占位。

玻璃边界、紧固件外观、插座深度、背面标签底板和内部尺寸来自照片或壳体空间估算。端口只有展示用途，Port A 保留收发方向冲突。六个视角的比较和尺寸记录见 [外观验收](evidence/m5stack-c160/verification.json)，离线复现使用 `python tools/rebuild-product-model.py build m5stack-c160 --scratch <目录>`。
