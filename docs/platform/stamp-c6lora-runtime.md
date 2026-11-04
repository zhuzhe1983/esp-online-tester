# Stamp C6LoRa S012 数字运行子集

`stamp-c6lora` 对应裸 S012 模块：ESP32-C6、16 MiB 外部 Flash、无 PSRAM。板级模型复用已有 PI4IOE5V6408 和 SX1262 协议实现，接入该型号的真实 I²C、SPI、复位、BUSY 与 DIO1 路由。型号与引脚依据 [官方产品页](https://docs.m5stack.com/en/core/Stamp_C6LoRa)、[原理图 v0.2.3](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1223/S012-SCH_Sch_M5_C6_Lora_v0.2.3_2025_12_02_19_14_35.pdf) 和 [Arduino RadioLib 示例](https://docs.m5stack.com/en/arduino/m5stamp_c6lora/program)。抓取身份和逐脚记录见 [sources.json](evidence/stamp-c6lora-runtime/sources.json)。

| 连接 | 引脚 |
| --- | --- |
| PI4IOE5V6408 @ 0x43 | SDA G10、SCL G8、INT G3 |
| SX1262 SPI2 | MOSI G21、MISO G22、SCLK G20、NSS G23 |
| SX1262 状态 | BUSY G19、DIO1 G7 |
| PI4IO 控制输出 | P5 LNA_EN、P6 天线开关电源、P7 SX_NRST |

S012 的 C6 使用 QFN40 与外部 W25Q128；板型挂载时沿用已有外部 Flash eFuse 身份处理。G9 有下载上拉，但它是外露焊盘，未虚构板载按钮。模块没有已确认的用户显示屏或 RGB。

测试读到的 `SX1261` 是 RadioLib 使用的家族版本字符串。[固定版本 SX1262.h](https://github.com/jgromes/RadioLib/blob/b91c6af112e423dcb14529ade5827802a14e1015/src/modules/SX126x/SX1262.h#L13) 明确把 `RADIOLIB_SX1262_CHIP_TYPE` 定义为该字符串，并解释 SX1262 也会这样报告。这个结果证明寄存器协议链路，不是独立鉴别 SX1262 硅片型号或修订号的证据；具体型号来自 S012 厂家资料，未知 ROM 后缀保持零值。

运行时采用连续外部 3.3 V 供电。PI4IO 的方向、高阻、输出值与中断寄存器决定线路状态；只有 P7 实际驱动高电平才释放 SX1262 复位。P6 控制数字天线路径。P5 关闭时拒绝显式对端收包，并取消尚未完成的收包；重新启用不重放旧包。这是收包链的数字门控策略，不是 LNA 增益或关断衰减模型。SPI 接受正确物理路由上的 GPIO NSS 分段事务或外设 NSS 完整事务。

`readLoRa()` 返回实际客体配置、已发送数据和计数。`receiveLoRa()` 按同一信道/调制参数安排数字对端数据；只有客体通过 SPI 读回载荷并处理 DIO1 后才算输入验收。BUSY、超时、CRC 错误和空闲/接收状态沿用共享 SX1262 模型。没有 LoRaWAN 服务、射频传播、天线损耗、接收增益或距离模型；BAT/MODULE_EN、模拟电源、真实晶振与硅时序未验证。没有实机验证。

截至 2026-11-04，本次官方页和完整 M5Burner C6LoRa 分类调查没有找到本型号预编译出厂或 UIFlow 镜像，也未找到精确小智目标。官网示例是开发源码，不足以证明出厂用途。目录保留本地构建文件上传入口，内部协议测试固件不进入出厂固件列表。

可重建验证源位于 `tools/fixtures/stamp-c6lora-firmware`，使用 ESP-IDF 5.5.1。先激活该 SDK 的 Python 环境并 source `export.sh`，然后：

```sh
idf.py -C tools/fixtures/stamp-c6lora-firmware -B /tmp/stamp-c6lora-build \
  -D SDKCONFIG=/tmp/stamp-c6lora-sdkconfig build
CARGO_BUILD_JOBS=2 cargo test -p esp32c6 stamp_c6lora --lib
tools/wasm-build.sh
tools/fetch-rom-elfs.sh web/wasm/fw
node tools/probe-stamp-c6lora.mjs --build /tmp/stamp-c6lora-build
```

测试固件通过真实 IDF 驱动验证错误 I²C 地址/针脚、错误 SPI CS、SX1262 版本、TxDone/DIO1、对端载荷、CRC 错误、射频控制脚、暂停、平台重启和客体软件重启。二进制只放在临时构建目录，不提交或公开托管。固件刻意采用较短的 SF7 包来验证协议，与官方示例的 SF12 参数不同；测试结果不能替代完整 RadioLib 示例兼容性。

最初通过的固件使用 IDF 轮询 SPI 传输。初次使用 `spi_device_transmit` 的排队传输停在第一笔 SPI 操作，20.02 秒客体时间内未配置射频芯片；失败回执保留在 [queued-spi-negative.json](evidence/stamp-c6lora-runtime/queued-spi-negative.json)。此结果限制的是当时验证的 SPI 驱动路径，不是已完成的收发结果。

后续独立修复补上共享 GP-SPI 的 `DMA_INT_SET` 写一置位语义：S3/C6/P4 在偏移 `0x44` 置位 RAW，ENA 决定 IRQ 是否对外可见，CLR 清除置位。寄存器不启动线路传输。IDF 5.5.1 的 `spi_hal_init` 用它触发第一次排队传输；C3 没有此寄存器，本次保留其原有行为，没有把保留地址变成中断入口。逐芯片头文件依据见 [spi-int-set-sources.json](evidence/stamp-c6lora-runtime/spi-int-set-sources.json)。

同一排队模式测试镜像在旧 WASM 上失败、新 WASM 上通过，输入 SHA-256 完全相同：[修复前](evidence/stamp-c6lora-runtime/queued-before-fix.json)、[修复后](evidence/stamp-c6lora-runtime/queued-after-fix.json)。两份回执保留各自 WASM hash、真实射频状态和客体 UART 标记。使用以下选项可重建排队模式；默认仍为轮询模式：

```sh
idf.py -C tools/fixtures/stamp-c6lora-firmware -B /tmp/stamp-c6lora-queued-build \
  -D SDKCONFIG=/tmp/stamp-c6lora-queued-sdkconfig -D STAMP_C6LORA_QUEUED_SPI=ON build
node tools/probe-stamp-c6lora.mjs --build /tmp/stamp-c6lora-queued-build --spi-mode queued
```

该对照验证了 C6 的 IDF 排队驱动；没有声称 S3/P4 的应用级排队传输已逐项验收，也没有把 C3 的 RAW 软件置位扩展进本次修复。`INT_SET` 读回零是写触发寄存器的模型策略，未做硅片读回测量。
