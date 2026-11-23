# M5Stack M5GO IoT Kit v2.7 运行范围

K006-V27 复用 `m5stack-basic` 的主机数字线路。模拟配置是经典 ESP32、16 MiB Flash、0 MiB PSRAM。ILI9342C 支持 320×240 显示、数字背光与复位。A/B/C 分别拉低 GPIO39/38/37。TF 卡使用 CLK18、MOSI23、MISO19、CS4。主机 G18/G23 同时接到 LCD，LCD 另用 CS14、DC27、RST33、BL32。按键上拉可在官方 M5 Core 原理图第 4 页核对，版次由 K006-V27 官方 PinMap 确认。[M5GO v2.7 产品资料](https://docs.m5stack.com/en/core/M5GO_IoT_Kit_v2.7) · [M5 Core 原理图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/480/M5-Core-Schematic_20171206.pdf)

## 固件结果

| 来源 | 实测 |
| --- | --- |
| 默认官方 v2.7 User Demo EasyLoader | 原样提取的 4 MiB 镜像进入 ROM 启动后在 `0x40121ea4` 触发整数除零。JIT 和解释器均复现。保留官方恢复入口。 |
| 官方 Core / M5GO UIFlow1 v1.15.5 | 原样镜像进入 ROM 启动后同样发生整数除零并重启。 |
| 官方 Core / M5GO UIFlow2 v2.5.5 16 MiB | 工作台从厂商 CDN 载入镜像，经真实 WASM 启动并绘制 320×240 画面。通过 UART0 的 MicroPython 读到 A/B/C 的低电平和释放状态。客体经 SPI 挂载 64 MiB FAT32，写入并读回测试文件。目录保持试运行。 |

原厂 User Demo 读取 MCPWM0 定时器配置，取周期字段再用它做除数。当前经典 ESP32 MCPWM0 没有寄存器模型，客体在该路径上读到零。未修改固件或仿真寄存器来跳过故障。[ESP-IDF v5.4 ESP32 MCPWM 寄存器定义](https://github.com/espressif/esp-idf/blob/v5.4/components/soc/esp32/register/soc/mcpwm_reg.h)

屏幕为非触摸屏。浏览器分别用 3D 键帽、实体键盘和 2D 面板触发 A、B、C，客体读到对应 GPIO 的低电平和松开后的高电平。侧面电源键、IP5306 供电、底座 RGB 灯带、麦克风、扬声器、MPU6886、红外、Grove A/B/C、M-Bus 与随附 Unit 的运行范围留待验证或实现。未接入实体设备。

## 复现

本地 EasyLoader 镜像从 [厂家下载](https://m5stack.oss-cn-shenzhen.aliyuncs.com/resource/docs/products/core/m5go_v2.7/M5GO%20IoT%20Starter%20Kit%20v2.7.exe) 的字节偏移 `0x6009e3` 取 4,194,304 字节。容器与提取结果的固定摘要见 [运行收据](evidence/m5stack-k006-v27/runtime.json) 与 [Core 固件来源记录](evidence/core-family/README.md)。镜像由调用方本地提供。

```sh
esp32sim --chip esp32 --board m5stack-basic --rom "$ESP32_ROM" --boot rom \
  --flash-mb 16 --psram-mb 0 --flash-image "$M5GO_USERDEMO_BIN" \
  --max-seconds 1.2 --console uart0 --no-jit --no-dump
```

在工作台选择 K006-V27 与试运行的 UIFlow2 v2.5.5 16 MiB。等待 UART0 显示 `V2.5.5` 和实际 LCD 画面。向 UART0 发送 Ctrl-C、Ctrl-C、Ctrl-A，进入 MicroPython raw REPL。按住或释放对应按键，重复读取三个 GPIO。每段程序以 Ctrl-D 执行。

```python
from machine import Pin, SDCard
import vfs

print([Pin(i, Pin.IN).value() for i in (39, 38, 37)])
sd = SDCard(slot=2, width=1, sck=18, mosi=23, miso=19, cs=4)
vfs.mount(sd, "/sd")
with open("/sd/m5go.txt", "w") as output:
    output.write("M5GO_SDSPI")
with open("/sd/m5go.txt", "r") as source:
    print(source.read())
print(sd.ioctl(4, 0), sd.ioctl(5, 0))
vfs.umount("/sd")
sd.deinit()
```

[UIFlow2 Core / M5GO 指南](https://docs.m5stack.com/en/uiflow2/m5core/program) · [UIFlow2 v2.5.5-16MB 镜像](https://m5burner-cdn.m5stack.com/firmware/d13aa198f7aebe2daeec262ab08e46d9.bin)
