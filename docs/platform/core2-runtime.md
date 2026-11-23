# M5Stack Core2 板级运行

| 型号 | 板型 | 原厂固件验收 |
|---|---|---|
| K010 | `core2`，AXP192、MPU6886 | 浏览器用官方 v2.2 完整镜像经 ROM 启动，显示 320×240 仪表盘。屏幕触摸使固件的坐标显示从 0,0 变为 160,120；默认 64 MiB FAT32 卡已被固件挂载。 |
| K010-V11 | `core2-v11`，AXP2101、INA3221、MPU6886 | 官网 v1.1 EasyLoader 内的镜像与 v2.2 一致。原厂程序读取 TF，当前电源模型中 ALDO2 保持关闭，原厂画面及触摸未通过。 |
| K010-V13 | `core2-v13`，AXP192、BMI270 | 官方 EasyLoader 内嵌镜像经本地 ROM 启动，能读取默认 FAT32 卡。启动自检报告 IMU 失败，触摸跳过后进入仪表盘，显示触摸坐标。 |
| K010-AWS | `core2-aws`，AXP192、MPU6886 | 官方 v0.0.1 在本地进入 AWS IoT EduKit 滑动首页。AWS 安全芯片和云流程未验收。 |
| K010-AWS-V13 | `core2-aws-v13`，AXP192、BMI270 | 官方 EasyLoader 内嵌 User Demo 经本地启动并初始化 BMI270；固件在 ATECC608 初始化失败后请求重启。 |

五款均为部分支持。固件仍由官方来源提供，EasyLoader 里的镜像可提取后以本地文件加载。每款的固件来源及失败复现见 [固件调查](evidence/m5-core2/sources.json)和[运行记录](evidence/m5-core2/runtime.json)。

## 已建模硬件

- LCD 的 SCK、MOSI、MISO、CS、D/C 分别是 GPIO18、23、38、5、15。使用 ILI9342C 指令的数字显示子集和原生横向 320×240 帧。后期 ILI9342E 批次的专用初始化、背光亮度曲线和光学表现未验收。
- FT6336U 通过 GPIO21/22 上的内部 I²C 与 GPIO39 触摸中断连接。屏下 A/B/C 与 LCD 共用触摸控制器。依据官方 M5Core2 库，用 y=260 的原始坐标模拟三枚触摸键；三维热点位置从各型号已有屏幕窗口换算，尚未做实物对轴。键盘 A/B/C 把同一事件送至触摸控制器。
- AXP192 板的 LDO2 为 LCD 逻辑和 TF 供电，GPIO4 释放触摸及屏幕复位，DC3 打开背光。K010-V11 分别使用 ALDO4、ALDO2、BLDO1，独立挂载 INA3221。电源键进入相应 PMIC 寄存器。经典 SoC 的实体断电、充电、温度和 RTC 闹钟唤醒不在当前范围内。
- TF 同用 SPI 的 GPIO18/23/38，对应 CS 为 GPIO4。供电中断时重置卡协议状态，保留持久数据。默认卡以 FAT32 准备，用户可继续导入已有卡，运行结果以固件是否实际挂载为准。
- MPU6886 或 BMI270 按硬件版独立挂接至 `0x68`。BM8563 提供客体时间驱动的基础日历。惯性数据采用虚拟采样，缺少物理校准。AWS 的 ATECC608、底座灯、麦克风及扬声器未建模。
- 16 MiB Flash。经典 ESP32 模拟器配置 4 MiB PSRAM；产品知识库保留厂家的 8 MiB 硬件规格。未以实物操作或刷机验收。

## 复现

```sh
cargo test --release -p esp-periph --lib i2c::axp192::
cargo test --release -p esp32 --lib board::core2::
cargo run --release -p esp32 --example core2_factory_probe -- <merged-bin> <esp32-rev300-rom-elf> <core2-board>
```

本地探针可以通过 `CORE2_PROBE_SD_IMAGE` 指定待挂载的 FAT32 镜像，以及通过 `CORE2_PROBE_PPM_DIR` 把受控场景的帧写到指定的空目录。镜像、原始捕获及构建产物留在本机。浏览器打开工作台后，使用 Playwright wrapper 的 `run-code --filename tools/probe-core2-browser.js` 复验原版 320×240 仪表盘、SD 以及触摸区域读数变化。
