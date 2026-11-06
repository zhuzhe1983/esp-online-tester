# CoreMatrix C158 功能覆盖审查

CoreMatrix 的板载核心功能已经能运行官方固件，但不能宣称“全部兼容”。本报告把产品硬件能力、模拟器实现和固件执行证据分开记录。审查基线为集成提交 `0768d8d4ec3254e27c2802373b0d0fef16921804`；内部系统实测使用 C61 提交 `5d67a6ae` 的 release 构建，内部系统结果保留该历史基线；下表已根据后续独立原生及生产 Worker 执行记录更新，汇总见[运行报告](../../corematrix-runtime.md)。

硬件依据是 [M5Stack C158 产品文档](https://docs.m5stack.com/zh_CN/core/CoreMatrix)、[ESP32-C61 数据手册](https://documentation.espressif.com/esp32-c61_datasheet_en.html)和 [ESP-IDF v5.5.4 C61 能力定义](https://github.com/espressif/esp-idf/blob/v5.5.4/components/soc/esp32c61/include/soc/soc_caps.h)。官方固件报告的 SDK 为 `v5.5.4-dirty`，不能把 C6、S3 或其他 M5 主机的驱动与运行结果直接算到 C61 上。来源时间与哈希见 [sources.json](sources.json)及[固件来源记录](../corematrix-firmware-20261106/sources-and-image.json)。

“已验证”表示指定固件执行产生了对应输出；“已实现、未验证”表示只有代码或器件层测试；“未实现”表示当前产品运行路径缺少必要设备或路由。模拟输入和数字行为均不表示经过实物、电气或无线射频校准。

| 能力 | 当前状态 | 证据与边界 |
| --- | --- | --- |
| ESP32-C61、16 MB Flash、8 MB PSRAM 启动 | 已验证 | 真正 C61 ROM → bootloader → FreeRTOS → MicroPython；芯片 ID 20、revision 100。见 [C61 运行记录](../corematrix-runtime/c61-native.md)。 |
| 16×16 白光 LED 点阵、亮度、供电 | 已验证 | TM1680 0x72 的命令/RAM写入产生256灯状态；原版固件驱动，不能以直接设置浏览器像素代替。验收程序为 `tools/probe-corematrix-uiflow.mjs`；器件为 `esp-periph/src/i2c/tm1680.rs`。物理产品不是 RGB 矩阵。 |
| A/B/C、PWR 按键 | 已验证 | PM1 输入和真实 M5 API 采样；电源键复位/下载另见 C61 原生记录。按键事件排队本身不算验收。 |
| BMI270 加速度/角速度 | 已验证 | 固件读实际 I²C 寄存器；M5Unified 固定版本坐标变换为 `[rawY,-rawX,rawZ]`。场景旋转和显式注入的校准必须分开。 |
| PWM 蜂鸣器 | 已验证 | 原版 `M5.Speaker.tone` 和 IOE1 PWM 产生非零48 kHz PCM，1200 Hz频率与供电静音通过生产 Worker 检查。直接 PWM 先停止原库后台任务，设置 IO2 push-pull 与 IO11/channel2。数字波形不等于实物音色。 |
| microSD SPI、供电/插卡检测 | 已验证 | G25/27/26/28 与 GDMA v2 实际双向传输；原版 MicroPython FAT 文件写读、平台 reset、固件重载后读回。真实浏览器另验证 IndexedDB 保存和页面刷新后读回。见[主运行报告](../../corematrix-runtime.md)。 |
| 内部 Flash 文件系统 | 已验证 | 原版 MicroPython 写/flush/rename/sync 16,420字节，逐字节读回，guest `machine.reset()` 后重新 ROM 引导并逐字节确认保持。见 [receipt.json](receipt.json)。本检查未验证浏览器进程退出后的持久化或断电中断写入。 |
| RTC 日期、活动状态计时、硬复位保持 | 已验证 | `machine.RTC` 设置日期，`sleep_ms(1250)` 后 ticks 差1250 ms，日期推进；硬复位后仍保持。C61 `LpSys` 慢时钟/锁存及复位保留参与运行。不是外置 RTC 芯片，也不证明闹钟/睡眠唤醒。 |
| 四个板载 I²C 设备 | 已验证 | 原版 `machine.I2C(0,sda=Pin(0),scl=Pin(1)).scan()` 返回 `[0x4f,0x68,0x6e,0x72]`，分别为 IOE1、BMI270、PM1、TM1680。 |
| PM1 电源开关、按键/BMI 唤醒 | 已验证 | 双击关机时 CPU 停止退休指令，单击原 ROM 启动；L1保留时 BMI INT1 经 PM1 GPIO4 唤醒，关闭 L1 不唤醒。电压值需等待电池检测稳定，见[电源回执](../corematrix-runtime/power-native.md)。定时PM1唤醒不在此执行验收范围。 |
| C61 定时 Light-sleep / Deep-sleep | 已验证；其他低功耗行为未覆盖 | 原版 `lightsleep(200)` 返回199 ms；CPU/寄存器/SYSTIMER保持，`deepsleep(200)` 经原 ROM 重启并读回深睡复位原因。原生和生产 Worker 通过；GPIO/UART/USB/BLE唤醒、REGDMA和模拟功耗未覆盖。见[C61睡眠模型](../../../esp32c61-sleep.md)。 |
| Bluetooth LE | legacy 数字广播/扫描专项验收 | 独立C61安全引擎、1MHz时钟、调度描述符、DMA与BT_MAC IRQ4执行原版控制器；广播37/38/39信道字节与间隔已验。接收与平台API结果见[BLE模型报告](../../../esp32c61-ble.md)。连接/GATT/配对/加密/RPA/扩展广播不在当前承诺内。 |
| Wi-Fi STA、SoftAP、TCP/UDP/TLS | 指定路径已验证 | open/WPA2 STA、DHCP/DNS/NTP、HTTP上传/下载和重连；SoftAP ARP/UDP，生产Worker另完成SoftAP DHCP/TCP/HTTP；TLS证书/主机名验证和4096字节响应、错密码/错主机名拒绝。UIFlow2.5.4的TLS日期缺陷需要明确guest RTC workaround。共享AP的MIC校验不完整，数字边界非RF/安全认证。见[网络回执](../../../evidence/c61-wifi/runtime.json)。 |
| Grove 的 G0/G1 与5V使能 | 已验证 | 外接I²C与内置器件共用G0/G1，IOE1 IO5实际控制外接模型供电；错线、错地址、断电NACK及恢复须新转换已由原版固件验证。 |
| Grove 外接 Unit / Hub / 传感器 | 指定模块已验证 | ENV-IV(SHT40/BMP280)、DLight(BH1750)和一层U006被动Hub：原生与生产Worker实际CRC/补偿/积分读数通过；原生另验叶子/上游拔插及CPU reset。其他GPIO/ADC/RMT配件、嵌套Hub及重复地址未开放。见[Grove回执](../corematrix-grove/runtime.json)。 |
| USB Type-C / MX1.25 USB 数据 | 串口 REPL 已验证 | C61 固定 USB Serial/JTAG 的 Serial 路径真实承载启动日志和 raw REPL。物理 D−/D+ 是 G12/G13；两种插座对应相同信号，不是两个独立 USB 控制器。JTAG 调试协议未验证。 |
| USB Host、HID、UAC、MSC 设备类 | 非本机已实现能力 | C61 的固定 USB Serial/JTAG 不是 S3 USB-OTG；不能仅凭 Type-C 插座加入这些能力。外置控制器/软件特殊实现不在本适配范围。 |
| UART0/1/2 与 M-Bus 路由 | 已实现、客体收发未验证 | `periph.rs::uart_route` 和 `soc.rs::uart_input` 有 C61 引脚/时钟/波特率路由；`hardware.rs::c61_uart_matrix_native_pins_and_source_clock_are_not_c6_values` 验证正确与错误波特率。尚无原版固件配置UART后经物理引脚双向数据交换的产品验收。USB REPL 不能替代该项。 |
| M-Bus GPIO/ADC/SPI/PWM 与任意外设 | 基础部分实现，产品扩展未验证 | GPIO/ADC/SPI/LEDC 可供已实现的板载路径使用；没有全引脚/所有外设协议组合的资格证明，附件平台路由也未开放。 |
| UIFlow 云端自动启动 | 已确认官方分发缺件 | 原版2.5.2/2.5.3/2.5.4的 `M5Things` 均缺失，见 [版本比较](../corematrix-firmware-20261106/startup-comparison.json)。REPL 和板载 API 可用不表示官方云端启动流程完整；不能写一个假模块来消除错误。 |
| 原厂出厂固件 / 小智固件 | 未找到精确官方匹配 | UIFlow 是明确标识的官方替代固件，不冒称 factory。产品、芯片与构建身份应完全匹配，不能套用其他 ESP32-C61 产品二进制。见 [出厂固件审计](../corematrix-firmware-20261106/factory-audit.json)。 |

历史负结果仍保留：`e2beebcd` 中 BLE 激活未返回；后续安全引擎、调度和DMA接入修复了该路径。睡眠首次错误使用旧ROM枚举bit3，后按C61 PMU bit4与eFuse idle状态修正。生产Worker的Grove输入最初被S3-only守卫拒绝，开放C61指定I²C输入后实际客体读数通过。

仍未覆盖BLE连接/GATT、其他SoC唤醒源、UART客体回环、任意M-Bus/扩展引脚组合、BMI FIFO/计步及官方云端自动启动。各项实现范围和原版固件缺陷应分别判断，不能用“全部功能适配完成”替代未验证项。

Espressif 的 [C61 sleep_modes 页面](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c61/api-reference/system/sleep_modes.html)在本次抓取时明确提示内容尚未针对 C61 更新。因此本报告只用它说明文档限制，不采用其中跨芯片示例的寄存器、RTC GPIO或唤醒引脚值。[官方 BLE 概述](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c61/api-guides/ble/overview.html)确认硬件 BLE 能力；不同官方页面的认证版本号不一致，本报告不据此宣称特定 Bluetooth 版本的完整协议认证。
