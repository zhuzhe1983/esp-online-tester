# CoreMatrix C158 功能覆盖审查

CoreMatrix 的板载核心功能已经能运行官方固件，但不能宣称“全部兼容”。本报告把产品硬件能力、模拟器实现和固件执行证据分开记录。审查基线为集成提交 `0768d8d4ec3254e27c2802373b0d0fef16921804`；内部系统实测使用 C61 提交 `5d67a6ae` 的 release 构建，后续新增功能应以自己的执行记录更新此表。

硬件依据是 [M5Stack C158 产品文档](https://docs.m5stack.com/zh_CN/core/CoreMatrix)、[ESP32-C61 数据手册](https://documentation.espressif.com/esp32-c61_datasheet_en.html)和 [ESP-IDF v5.5.4 C61 能力定义](https://github.com/espressif/esp-idf/blob/v5.5.4/components/soc/esp32c61/include/soc/soc_caps.h)。官方固件报告的 SDK 为 `v5.5.4-dirty`，不能把 C6、S3 或其他 M5 主机的驱动与运行结果直接算到 C61 上。来源时间与哈希见 [sources.json](sources.json)及[固件来源记录](../corematrix-firmware-20261106/sources-and-image.json)。

“已验证”表示指定固件执行产生了对应输出；“已实现、未验证”表示只有代码或器件层测试；“未实现”表示当前产品运行路径缺少必要设备或路由。模拟输入和数字行为均不表示经过实物、电气或无线射频校准。

| 能力 | 当前状态 | 证据与边界 |
| --- | --- | --- |
| ESP32-C61、16 MB Flash、8 MB PSRAM 启动 | 已验证 | 真正 C61 ROM → bootloader → FreeRTOS → MicroPython；芯片 ID 20、revision 100。见 [C61 运行记录](../corematrix-runtime/c61-native.md)。 |
| 16×16 白光 LED 点阵、亮度、供电 | 已验证 | TM1680 0x72 的命令/RAM写入产生256灯状态；原版固件驱动，不能以直接设置浏览器像素代替。验收程序为 `tools/probe-corematrix-uiflow.mjs`；器件为 `esp-periph/src/i2c/tm1680.rs`。物理产品不是 RGB 矩阵。 |
| A/B/C、PWR 按键 | 已验证 | PM1 输入和真实 M5 API 采样；电源键复位/下载另见 C61 原生记录。按键事件排队本身不算验收。 |
| BMI270 加速度/角速度 | 已验证 | 固件读实际 I²C 寄存器；M5Unified 固定版本坐标变换为 `[rawY,-rawX,rawZ]`。场景旋转和显式注入的校准必须分开。 |
| PWM 蜂鸣器 | 已实现；固件专项验收 | `CoreMatrix::capture_audio` 由 IOE1 PWM 频率、占空比与 IO2 供电产生48 kHz方波。直接 PWM 必须先 `M5.Speaker.end()`，配置 IO2/IO11 输出，避免原库后台任务清零；恢复 `M5.Speaker.begin()` 后验证原库 tone。光有串口成功返回不证明有非零 PCM。 |
| microSD SPI、供电/插卡检测 | 已实现；固件专项验收 | `esp32c61/src/board/corematrix.rs` 实际 SPI 接线 G25/27/26/28，IOE1 IO1 供电、IO10检测；SD 文件读写及浏览器重载保存由产品主验收程序负责，不能用内部 Flash 结果替代。 |
| 内部 Flash 文件系统 | 已验证 | 原版 MicroPython 写/flush/rename/sync 16,420字节，逐字节读回，guest `machine.reset()` 后重新 ROM 引导并逐字节确认保持。见 [receipt.json](receipt.json)。本检查未验证浏览器进程退出后的持久化或断电中断写入。 |
| RTC 日期、活动状态计时、硬复位保持 | 已验证 | `machine.RTC` 设置日期，`sleep_ms(1250)` 后 ticks 差1250 ms，日期推进；硬复位后仍保持。C61 `LpSys` 慢时钟/锁存及复位保留参与运行。不是外置 RTC 芯片，也不证明闹钟/睡眠唤醒。 |
| 四个板载 I²C 设备 | 已验证 | 原版 `machine.I2C(0,sda=Pin(0),scl=Pin(1)).scan()` 返回 `[0x4f,0x68,0x6e,0x72]`，分别为 IOE1、BMI270、PM1、TM1680。 |
| PM1 电源开关、定时/按键/BMI 唤醒 | 已实现；固件专项验收 | `CoreMatrix::power_state`、PM1/BMI 器件模型、板级电源事件控制实际 CPU 停止/重启。PM1 的外部断电/唤醒和 C61 SoC 内部 sleep 是不同路径。电池/VIN 为显式虚拟电压，不是电量或电流物理模型。 |
| C61 Light-sleep / Deep-sleep / 调制解调器省电 | 未实现 | `esp32c61/src/soc.rs::core_state` 只有板级供电导致的 Running/Held；`periph.rs` 没有 PMU 睡眠状态机，LP_TIMER 只有计数与锁存、没有 alarm IRQ。不能因 `time.sleep_ms` 或 PM1 断电成功就宣称芯片低功耗兼容。 |
| Bluetooth LE | 未实现，有负执行证据 | C61 硬件支持 BLE，官方构建也包含 `bluetooth`；但设备表缺少 C61 BT/BB 控制器。原版 `ble.active(True)` 在15亿脚本指令内未返回，访问未实现的0x600a1xxx/2xxx/7xxx窗口，见 C61 记录的 BLE 负结果（`e2beebcd`）。未验证扫描、连接、GATT 或配网。 |
| Wi-Fi STA、SoftAP、TCP/UDP/TLS | 独立网络验收范围 | C61 MAC/BB 和虚拟网络后端已有实现。必须逐项引用官方固件的 DHCP、DNS、socket、加密协商与应用数据测试；不能从“连上虚拟AP”推导 WPA2、SoftAP、TLS 或外网云服务可用。 |
| Grove 的 G0/G1 与5V使能 | 部分实现 | 内置I²C共用 G0/G1，IOE1 IO5电源位有状态；`hardware_diagnostics` 可报告5V使能。 |
| Grove 外接 Unit / Hub / 传感器 | 未实现产品路由 | `web/platform/accessory-routing.js::supportedHost` 仅接受 StopWatch；CoreMatrix 板类没有该平台附件接入路径。四个板载器件成功不等于外接模块可用，电源位变化也不等于已给外部模型供电。 |
| USB Type-C / MX1.25 USB 数据 | 串口 REPL 已验证 | C61 固定 USB Serial/JTAG 的 Serial 路径真实承载启动日志和 raw REPL。物理 D−/D+ 是 G12/G13；两种插座对应相同信号，不是两个独立 USB 控制器。JTAG 调试协议未验证。 |
| USB Host、HID、UAC、MSC 设备类 | 非本机已实现能力 | C61 的固定 USB Serial/JTAG 不是 S3 USB-OTG；不能仅凭 Type-C 插座加入这些能力。外置控制器/软件特殊实现不在本适配范围。 |
| UART0/1/2 与 M-Bus 路由 | 已实现、客体收发未验证 | `periph.rs::uart_route` 和 `soc.rs::uart_input` 有 C61 引脚/时钟/波特率路由；`hardware.rs::c61_uart_matrix_native_pins_and_source_clock_are_not_c6_values` 验证正确与错误波特率。尚无原版固件配置UART后经物理引脚双向数据交换的产品验收。USB REPL 不能替代该项。 |
| M-Bus GPIO/ADC/SPI/PWM 与任意外设 | 基础部分实现，产品扩展未验证 | GPIO/ADC/SPI/LEDC 可供已实现的板载路径使用；没有全引脚/所有外设协议组合的资格证明，附件平台路由也未开放。 |
| UIFlow 云端自动启动 | 已确认官方分发缺件 | 原版2.5.2/2.5.3/2.5.4的 `M5Things` 均缺失，见 [版本比较](../corematrix-firmware-20261106/startup-comparison.json)。REPL 和板载 API 可用不表示官方云端启动流程完整；不能写一个假模块来消除错误。 |
| 原厂出厂固件 / 小智固件 | 未找到精确官方匹配 | UIFlow 是明确标识的官方替代固件，不冒称 factory。产品、芯片与构建身份应完全匹配，不能套用其他 ESP32-C61 产品二进制。见 [出厂固件审计](../corematrix-firmware-20261106/factory-audit.json)。 |

仍需独立接入的主要功能是 C61 BLE 控制器、芯片内部睡眠状态机、CoreMatrix Grove/M-Bus 附件路由；UART 客体回环和扩展引脚属于下一批验证工作。PM1、网络、SD和蜂鸣器的专项结果应持续合入，不能用“全部功能适配完成”替代未完成项。

Espressif 的 [C61 sleep_modes 页面](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c61/api-reference/system/sleep_modes.html)在本次抓取时明确提示内容尚未针对 C61 更新。因此本报告只用它说明文档限制，不采用其中跨芯片示例的寄存器、RTC GPIO或唤醒引脚值。[官方 BLE 概述](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c61/api-guides/ble/overview.html)确认硬件 BLE 能力；不同官方页面的认证版本号不一致，本报告不据此宣称特定 Bluetooth 版本的完整协议认证。
