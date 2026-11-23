# CoreS3 系列板级模拟

| 型号 | 板型 | 默认原厂固件 | 主机差异 |
| --- | --- | --- | --- |
| CoreS3 K128 | `cores3` | UserDemo v0.12 | 与 DIN 底座一同销售，底座供电和外设暂未模拟 |
| CoreS3-SE K128-SE | `cores3-se` | UserDemo v0.6 | 无相机、接近传感器、BMI270 或 BMM150 |
| CoreS3-Lite K128-Lite | `cores3-lite` | UserDemo v0.12 | 厂商列出的固件与标准版使用同一镜像，平台分别保留型号和存储 |
| CoreS3 Thread BR K149 | `cores3-thread-br` | 官方 v1.0 | ESP32-S3 主机数字子集，未模拟 ESP32-H2 RCP 或 Thread/Zigbee 网络 |

K128、Lite 和 K149 的官方资料指向同一份 CoreS3 V1.0 主机原理图。SE 使用独立的 V1.0 图纸，保留相同的屏幕、触摸、RTC 和 TF 接线。固件版本、已核对的镜像摘要和图纸见[来源回执](evidence/cores3/sources.json)。

| 部件 | 客体路径 |
| --- | --- |
| ILI9342C 屏幕 | SCK36/MOSI37/CS3/D/C35，AW9523B P1_1 复位，AXP2101 DLDO1 背光，320×240 framebuffer |
| FT6336U 单点触摸 | SDA12/SCL11、`0x38`，AW9523B P0_0 复位，P1_2 中断经扩展器到 GPIO21 |
| BM8563 日历 | SDA12/SCL11、`0x51`，按客体时间推进 |
| BMI270 | SDA12/SCL11、`0x69`，固定虚拟采样，可由 Rust/WASM 板级接口输入传感器轴数据，SE 无此器件 |
| microSD | SCK36/MOSI37/MISO35/CS4，AW9523B P0_4 低电平检测插卡，AXP2101 ALDO4 供电 |

四款板型共用平台的 SPI SD 卡镜像通路。平台准备的 64 MiB FAT32 卡通过实际插卡状态和供电寄存器接入，断电时卡不回复 SPI 命令，重新供电需由固件初始化。定向 Rust 测试覆盖卡检测、错误针脚、掉电重置和 SE 不枚举缺少的 BMI270。

实际浏览器的 WASM worker 用原厂镜像启动四款主机。K128、SE 和 Lite 的 UserDemo 可从开机图进入主菜单，再进入 SD 页面，客体串口输出 `HomeMenu -> AppSD`。K128 的原厂程序从导入的 FAT32 镜像读取并显示了 `PROOF.TXT`。K149 显示 Espressif 标识并启动 `esp_ot_br`，随后报告 Spinel 响应超时。

SE 工作台通过 3D 屏幕点击打开主菜单，再从 2D 屏幕进入 SD 页面。mini 播放器在独立的空白存储环境中回放这四个触摸事件，客体再次进入 SD 页面，输出帧的 SHA-256 与原始录制一致。原录制的 Flash 数据保留。固件、ROM、WASM 与画面摘要见[浏览器回执](evidence/cores3/browser.json)。

复现时使用新建的本地浏览器会话，因为检查会替换该会话里 K128 的虚拟卡。构建 WASM 并准备 ROM 后，打开本地工作台，用 Playwright 包装器执行 `run-code --filename tools/fixtures/cores3-browser-check.js`。测试完成后关闭本次会话。

原厂镜像继续标为 `trial`。当前板型实现 ILI9342C 的数字命令，ILI9342E 批次、音频、相机、接近传感器、BMM150 辅助总线、真实温度与电量、RTC 唤醒、实体开关与 USB OTG 均未验收。未实机验证。
