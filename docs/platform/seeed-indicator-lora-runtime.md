# SenseCAP Indicator LoRa：独立板型与真实数字接线

为含 SX1262 的三个版本增加独立板型，复用仓库已有的 SX1262 数字器件及 Indicator
GX 显示模型。新板没有修改 CPU、总线、调度器或共享无线协议实现；普通 D1/D1S
继续使用原来的无无线器件板型，没有多出 radio 对象或每 tick 的可选设备分支。

| 产品 | 独立 runtime board | 本次范围 |
| --- | --- | --- |
| SenseCAP Indicator D1L | `seeed-sensecap-indicator-d1l` | GX 显示/输入，SX1262 数字 SPI、BUSY、DIO1、数字包 |
| SenseCAP Indicator D1Pro | `seeed-sensecap-indicator-d1pro` | 同上；RP2040 与环境传感器仍未模拟 |
| SenseCAP Indicator for Meshtastic | `seeed-sensecap-indicator-meshtastic` | 同上；独立产品身份与默认固件身份保持分开 |

这里的 GX 是当前明确采用的 ST7701/480×480 显示与 0x48 触摸配置，不能推及所有
DX 硬件修订。D1/D1S 的既有 GX/DX 入口仍无 LoRa。新板通过原来的
`BoardModel::lora_snapshot` 和 `lora_receive_at` 接口暴露数字无线状态。

## 可核验接线

固定 [Meshtastic 2.5.11 variant.h](https://github.com/meshtastic/firmware/blob/v2.5.11.8e2a3e5/variants/seeed-sensecap-indicator/variant.h)
与 [Seeed v1.0.0 SX126x 板驱动](https://github.com/Seeed-Solution/SenseCAP_Indicator_ESP32/blob/ed94f161a51b2ed1077d7eb4b887ac9f0d45fdaf/components/lora/sx126x_sensecap_board.c)
对下列物理连接一致。来源的实际获取时间、字节数及 SHA-256 见
[sources.json](evidence/seeed-indicator-lora-20261122/sources.json)。

| 连接 | 物理端点 | 模型行为 |
| --- | --- | --- |
| I²C SDA / SCL | ESP32-S3 GPIO39 / GPIO40 | 原有 0x20 扩展器的实际路由；错线不能 ACK |
| LoRa SCK / MOSI / MISO | GPIO41 / GPIO48 / GPIO47 | 经 `SpiPins` 传输；错误时钟/数据线不写器件，错误 MISO 返回未连接值 |
| LoRa NSS | 扩展器 P0 | 配置为输出并拉低才能选中；P0 拉高才结束一次器件事务 |
| LoRa RESET | 扩展器 P1 | 低有效复位；显式输出高后经过现有器件启动 BUSY 期限 |
| LoRa BUSY | 扩展器 P2 | 读到真实 SX1262 模型状态，不固定为“就绪” |
| LoRa DIO1 | 扩展器 P3 | 包完成/超时等已实现数字 IRQ 进入扩展器输入寄存器 |
| 扩展器 INT | GPIO42 | 低有效；读取对应端口才更新已观察输入并解除 INT，读另一端口不代替确认 |
| RF switch | SX1262 内部 DIO2 控制 | 复用已有器件命令；不虚构 ESP32 RF 使能 GPIO |

Meshtastic 的 GPIO64–67 是 `IO_EXPANDER=0x40` 的软件别名，不是 ESP32-S3 的
实体 GPIO。`SpiPins.cs` 不用不存在的第 64 位冒充 NSS。制造商驱动使用 SPI3，
Meshtastic 使用 Arduino SPI2；物理矩阵正确时两者均可连接到同一器件。

早期官方系统框图的部分 SPI/IRQ 标签与以上两个固定源码版本不同。当前接线采用
两个源码一致的值，保留这个资料版次差异，没有宣称取得完整的 LoRa 供电原理图。

## 事务与中断路径

新 wrapper 包含原 `SeeedIndicator` 实例和独立无线状态。最初的无线接线提交只为
基类增加读取现有扩展器引用的方法；随后根据原始 Meshtastic 的实际寄存器补充
RGB 数据线映射，详见下文。普通无无线版本依然没有无线状态。

扩展器 I²C 写操作完成每个字节后同步真实输出/方向状态，因此 NSS 的短脉冲不会
等到下一次板 tick 才被发现。两次 SPI 调用只要 P0 保持低，就属于同一事务；
P0 释放会终结事务，后续数据不能继续读取上次命令。读取输入前同步 BUSY/DIO1，
读取后保留共享扩展器自身的方向、极性和分端口中断确认行为。

数字包使用原器件的配置、FIFO、忙期限和空中时长计算。TX 完成通过 DIO1→P3→
INT→GPIO42 进入客体；只产生数字 TX 记录，不回送为 RX。接收必须由宿主显式
提供包，且器件处于接收状态、参数匹配、没有重叠接收、载荷长度不超过 255 字节。
真正完成前不会把包写成已收到。

显示初始化和无线共用 SPI 引脚。当前有界实现要求 LCD 的 P4 片选保持未选中才
处理 radio SPI；同时选中两者属于未建模的共享总线争用，返回未连接值，不猜测
MISO 冲突。RGB 帧传输、触摸和背光仍委托原显示模型。

## 电源与无线边界

官方板驱动的 `power_ctrl` 回调不操作 GPIO；设备按 USB 供电配置运行。当前使用
一个**理想数字供电域**作为 radio 和扩展器的供电条件，这是模型假设，不是对
缺失完整原理图的电路测量。测试可以切断这一内部供电状态验证复位/NACK；没有
把某个用户按键或未证实的 GPIO 变成无线供电开关。

未模拟电源斜率、上拉电阻、电平争用、RF 模拟开关、天线匹配、路径损耗、频谱、
法规限制、真实无线覆盖或物理空中链路。数字包 API 始终报告 `physicalRf:false`。
`config.frequency` 沿用现有 SX1262 API，表示频率寄存器字而非直接的 Hz；注入时
必须显式提供对端的完整配置，接口不会根据接收机状态自动修正不匹配的参数。
使用该 API 不会广播到网络或实体无线设备。

## 原始 Meshtastic 的 RGB 数据线映射

[固定 2.5.11 的 TFTDisplay.cpp](https://github.com/meshtastic/firmware/blob/v2.5.11.8e2a3e5/src/graphics/TFTDisplay.cpp)
使用 `Panel_ST7701` / `Bus_RGB`，可见面板是 480×480。虽然面板的
`memory_width` 配置为 800，所调查的库实际按 `panel->width()/height()` 分配
DMA 帧并设置活动时序；实机指令执行后的寄存器也明确为 480×480。

原始完整镜像在 6 秒时的 GPIO 矩阵显示：物理 DATA0…7 接到
LCD_DATA8…15，物理 DATA8…15 接到 LCD_DATA0…7。这个交换与
[LovyanGFX 的 `rgb565sig_tbl`](https://github.com/mverch67/LovyanGFX/blob/894e4340c94d78b1fb56d180185f773cc1f45099/src/lgfx/v1/platforms/esp32s3/Bus_RGB.cpp)
一致。板模型现在根据真实 GPIO 路由区分厂家 IDF 演示的直接映射和这种字节组
交换，再从真实 DMA 字节恢复 RGB565。它没有通过固件名称选择颜色顺序，也没有
把 800 宽缓冲猜成一个可见区域。缺失数据线、控制线、错误位宽、错误帧尺寸和
截断 payload 继续拒绝。

来源有一个明确限制：发布版 `platformio.ini` 将 LovyanGFX 指向可变的
`#develop`，没有锁定构建提交。上面的 `894e4340` 是发布日前的历史源码候选，
不能冒充该二进制已证实的构建依赖；实际执行后的 GPIO 矩阵才是本次映射修正的
直接证据。五份源码正文均通过返回的 Git blob SHA 验证，URL、时间、字节数和
SHA-256 与精简寄存器收据见
[display-routing.json](evidence/seeed-indicator-lora-20261122/display-routing.json)。

该收据同时保留零帧的负结果：LCD START/RGB 模式已开，DMA 首链已停止，
VSYNC 中断尚未出现。共享 LCD 计时与首链重启的问题另行检查；数据线映射的
板级修正和下面的测试不能单独证明原始 Meshtastic 画面已经显示。

此映射检查点新增两项回归用例：逐位 RGB565 颜色与真实矩阵的直接/交换映射，
以及所有数据/控制线缺失、错误位宽、800 宽/高、截断缓冲的拒绝。该检查点只做了
源码/寄存器审阅、差异检查和证据隐私检查，**尚未运行这两项 Cargo 测试**；整合
构建与实际原始固件运行的结果单独记录，不能把后文早先的测试通过结果用于它们。

本提交没有替代固件、伪造初始化返回状态、注入成功串口文字或覆盖应用画面。
原始 Meshtastic 完整镜像与最终生产 WASM 的启动/UI 结果需要单独记录，不能从
下面的板级测试推导“原厂完整兼容”。三个产品继续标为 `partial`。

## 验证

[checks.json](evidence/seeed-indicator-lora-20261122/checks.json) 保存命令、测试名称、
源码摘要与边界。定向检查结果：

- 6 项新 radio 单元测试通过：实际端口选中、正确/错误 SPI 路由、NSS 连续、
  RESET/BUSY、理想供电切断、TX 与显式 RX 数字流程。TX 用例先清除 BUSY 引起的
  扩展器中断，再检查真正 DIO1 包完成边沿，避免把旧中断误算为 TX 成功。
- 3 项原 Indicator 显示/触摸单元测试通过，保持原 GX/DX 路由和复位约束。
- 3 项新增真实 MMIO 集成测试通过：三个精确 LoRa 板型与普通无无线板型区分；
  GPIO 矩阵→硬件 I²C→扩展器→SPI 身份读取；BUSY 通过输入寄存器产生 GPIO42
  的实际电平变化和按端口确认。
- `cargo clippy -p esp32s3 --all-targets -- -D warnings` 通过。

MMIO 地址和信号位核对 Meshtastic 镜像描述中的 ESP-IDF 4.4.7。新增 I²C header
摘要在本来源收据；GPIO/SPI/IO_MUX header 沿用已实际取得的
[4.4.7 收据](evidence/seeed-ee-20261122/sources.json)。仅所引用字段与现有 5.5.x
transport fixture 相同，没有宣称两个 IDF 版本全部寄存器相同。

复现定向检查：

```sh
cargo test -p esp32s3 --lib seeed_indicator
cargo test -p esp32s3 --test seeed_indicator_lora
cargo clippy -p esp32s3 --all-targets -- -D warnings
```

这些是板级正确性检查，不是主机性能计时。全 workspace 门槛、最终 WASM 原始
固件运行、平台数字包 API 与原 main 的性能对照在整合阶段分别执行。
