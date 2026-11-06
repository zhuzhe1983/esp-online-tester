# CoreMatrix Port A 扩展

CoreMatrix C158 的 Port A 黄线为 G0/SDA、白线为 G1/SCL，与内部 I²C 器件共用总线；红线 5V 由 M5IOE1 IO5 控制。依据为[官方产品文档](https://docs.m5stack.com/zh_CN/core/CoreMatrix)、v0.7 原理图及[已固定的来源记录](evidence/m5stack-c158/source-audit.json)。端子形状本身不构成兼容依据。

当前接入 U001-D ENV-IV（SHT40 0x44、BMP280 0x76）和 U136 DLight（BH1750FVI 0x23）。它们复用已有的寄存器、转换等待、CRC、校准与供电模型，接在与板内器件相同的 GPIO0/1 软件/硬件 I²C 总线上。固件实际设置 IO5 后才可寻址；切断外接 5V 不会让内置 PM1 失去响应。U006 是一层被动并联分线器：两个不同地址模块可以共用，重复地址、同一插口、嵌套 Hub 和不支持的 GPIO/ADC/RMT 器件仍拒绝。

未修改的官方 UIFlow 2.5.4 经 C61 ROM 启动后，从 USB raw REPL 接收仅驻留 RAM 的 [grove.py](../../tools/fixtures/corematrix-uiflow/grove.py)，得到如下客体读数：

| 主机环境输入 | 官方固件中的实际 I²C 结果 |
| --- | --- |
| 22 °C / 60 %RH | SHT40 `62 03 5e 87 2a 51`，两组 CRC 正确；22.00122 °C / 59.999084 %RH |
| 990 hPa | BMP280 校准字节及原始测量补偿得到 990.00072 hPa |
| 1000 lx | BH1750 单次高精度转换返回 1200，标称换算为 1000 lx |

同一原版固件还通过 GPIO3/4 错线 NACK、0x45 错址 NACK、IO5 关闭后的 NACK、重新供电清除旧照度结果、逐个叶子断开而保留其他地址、全部断开后内置 PM1 仍可读，以及真实 CPU 复位后重新寻址与测量。内部存储没有写入 `boot.py`、`main.py` 或替代 M5Things 模块；原版启动缺失 M5Things 的问题仍保留。

复现命令：

```sh
cargo test -p esp32c61 --test corematrix_grove
node --test tools/platform-corematrix-grove-test.mjs tools/platform-unit-env4-test.mjs tools/platform-unit-dlight-test.mjs tools/platform-unit-hub-test.mjs
ESP32SIM_ROM_DIR=<roms> ESP32SIM_COREMATRIX_FIRMWARE=<official-image.bin> cargo test --release -p esp32c61 --test corematrix_grove_firmware -- --ignored --nocapture
```

原生固件验收与来源 hash 见[回执](evidence/corematrix-grove/runtime.json)。这确认数字协议和客体代码链路；没有实机测试，不包含模拟电压、上拉阻值、线长、负载电流、传感器误差或光谱校准。Web 前端使用同一接口与源代码，生产 WASM 和浏览器验收由 CoreMatrix 集成报告另行记录，不能从原生结果直接继承。
