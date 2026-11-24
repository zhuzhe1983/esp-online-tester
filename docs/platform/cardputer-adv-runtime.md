# Cardputer-Adv 官方固件运行范围

`cardputer-adv` 板级模型执行固定 hash 的官方 User Demo v0.3，通过真实 ST7789V2 寄存器和 TCA8418 扫描队列生成 240×135 画面并处理矩阵键盘。平台没有替换固件、在主机绘制菜单或把按键改成串口命令。支持状态为部分兼容；未做实机对照。

M5Burner 将这个镜像标为 v0.3；镜像内部 app version 是 `ADV-V0.2-5-g452db44`、ESP-IDF v5.4.2。保留两个版本标识，不推断它们来自同一可重现源码提交。当前可查的官方 `CardputerADV` 分支固定于 `b549eac0a3c65bc108186c276b8fac0a214aaa4e`，用于核对驱动与输入含义。

## 已实现的硬件路径

| 部件 | 路径与范围 |
| --- | --- |
| ST7789V2 | MOSI35/CLK36/CS37/DC34/RST33；SPI2 三线 RDDID 探测，正式绘制使用 SPI3；真实窗口、RGB565、RAM 与 MADCTL；原生 RAM 中可见区域转为 240×135 |
| TCA8418 | I2C 0x34、SDA8/SCL9、IRQ11；4×14 物理键映射到 7×8 扫描矩阵；双次稳定扫描、10 项 FIFO、按下/释放编码、事件数、溢出策略、W1C 与中断电平 |
| I2C | 软件探测与两个硬件 I2C 控制器共享同一组外设状态；无模拟电气、争用或时钟拉伸 |
| ES8311 数字音频 | `0x18`、SDA8/SCL9；G41 BCLK、G43 LRCK、G46 RX、G42 TX；标准 I2S PCM 经真实 S3 GDMA，受 codec 静音、增益、复位与电源控制，范围见下文 |
| G0 | GPIO0，低电平有效；原有 button API 路径 |
| 可写 Flash | 通用设备存储按产品、板型、芯片、容量和初始 Flash 内容隔离，见 [设备存储](device-storage.md) |

键盘模型每 25 ms guest 时间扫描一次，两次稳定观察后产生事件。这个数字近似保留扫描和去抖的顺序，通常导致 25–50 ms 响应；它不是逐行电气扫描或实测延迟。TI 的 50 µs GPIO 去抖与矩阵扫描是不同机制，不混用。`INT_CFG` 开启时，对有待取事件的 W1C 清除产生 50 µs 中断释放，再重新拉低。空闲且没有中断释放计时器时不安排扫描。

模型未实现矩阵鬼键、电气二极管网络、GPI 事件、keylock 解锁引擎、精确单行扫描和所有 GPIO 寄存器功能。输入 API 表示物理键位，不承诺 USB/Bluetooth HID。

## Agent 输入与真实验收

```js
await espTester.input({ type: 'matrix-key', row: 2, column: 13, pressed: true });
await espTester.input({ type: 'matrix-key', row: 2, column: 13, pressed: false });
```

`row` 为 0–3，`column` 为 0–13；`pressed` 必须为 boolean。暂停时允许释放，不允许新按下。回执的 `guestTime` 是输入送达 worker 时的实际时钟；`delivery: 'queued'` 表示进入硬件输入队列，不能单独当作固件已响应的证据。多个键分别按下、释放即可形成组合键。能力表提供 `input.keyboard` 的行列数、模型与布局。

官方固件开机后等待按键；Enter 松开可离开开机图。菜单中物理位置 `(3,12)` 向右、`(3,10)` 向左。验收等待官方 spring 动画在实际 guest 时间内稳定，再比较真实 framebuffer。首次进入与来回后存在稳定的 1 px 横向差异；回到原项时允许这一平移，但要求 160×75 内部区域每个像素完全相同，保留原位置 hash 失败记录。同时检查下半屏已覆盖开机图，避免只看上部变化而漏掉 DMA 丢帧。

复现命令要求明确传入 URL/hash，第三方固件仅进入测试进程内存：

```sh
ESP32SIM_PROBE_CLOCK_TIMEOUT_MS=60000 node tools/probe-official-cardputer-adv.mjs \
  https://m5burner-cdn.m5stack.com/firmware/4ed4ab3a388a74e805971f37d6adcc9f.bin \
  7bb1532427c875445b16186b6139afa371f67ddbdc83ae5bfe4a9089c7883b74 \
  output/cardputer-adv-check 8 controls
```

网站可在用户浏览器本地缓存并复核 hash；本站服务器和 Git 不镜像该二进制。

主页面的真实键盘组合输入和浏览器失焦释放已单独检查。矩阵键录制覆盖 Enter 与右键的按下、释放；生成的分享链接在 mini 页面载入后保持未启动，点击回放才开始执行。本次主页面与分享页两次回放的最终 framebuffer 均与录制 hash 相同，最大输入迟到约 69.3 ms；这不保证其他录制逐帧一致。测试只清空隔离浏览器的设备数据，未自动清除用户存储。回执见 `evidence/cardputer-adv-official/browser-inputs.json` 与 `matrix-recording.json`。

## 本次修正的通用边界

- SPI `USER.SIO` 使用 D/MOSI 输入路由，保留矩阵、IO_MUX、输入缓冲和反相门禁；四线模式继续使用 Q/MISO。
- GPIO `PAD_DRIVER` 开漏 HIGH 释放到原有上下拉，LOW 驱动；启用、禁用和外部输入仍按原有数字模型处理。M5GFX 正是通过开漏加下拉区分 Adv 的内部 I2C 上拉与原版 Cardputer。
- 一个 SPI USR 数据阶段结束在 GDMA descriptor 中间时，保留未读位置供下一次 USR 使用；完整消费后才提交 descriptor 完成、EOF 与 owner writeback。未模拟 DMA 内部预取 FIFO 的精确时刻。

源位值核对 ESP-IDF v5.4.2 与本地 v5.5.1；v5.4.2 C3/C6 的 SIO bit17、PAD_DRIVER bit2 亦一致。没有改 CPU 时钟或单指令调度。回归包括 SPI2/SPI3、即时与原有可选延迟完成路径。历史机制见 EX217 的 GPIO 输入解析、EX235 的 SPI3 和 EX214 的 descriptor walker；此处是正确性修复，不宣称性能优化。

## 当前限制

ES8311 已有下述数字通路测试；BMI270 使用已有识别/配置子集。实机音频、任意固件的录音/语音应用、持续 IMU 采样及姿态桥、SD、IR、LoRa、GPS 物理 UART、Wi-Fi 应用、Bluetooth/USB HID、供电和睡眠均未在这块板上验收。菜单包含这些应用，不代表功能已可用。独立显示开关/睡眠指令的即时画面发布未单独验收。屏幕前光和光学色彩未做硬件校准。普通 Cardputer 与 v1.1 的键盘电路不同，不能复用此板型宣称兼容。

## ES8311 数字 PCM 子集（2026-11-25）

控制端使用共享 `Es8311State`，替代通用寄存器占位；GPIO 位操作与两个硬件 I2C 控制器看见同一状态。芯片数字复位保留控制寄存器；`FAh.INI_REG` 恢复默认值并保持复位，直到固件解除。MCU 重新连接板级设备不会暗中给外部 codec 断电。

RX 从声明了 G41/G43/G46 的 `PcmSource`（或显式控制器输入）取实际 PCM，经 I2S 打包、codec 增益/静音处理后写入 GDMA 描述符指定的 guest 内存。TX 读取真实 DMA 载荷，处理 codec 后发布原有 PCM16 输出。IO_MUX、输入缓冲、矩阵反相、输出使能或任一物理线路不符时，该板的采样为零；不会改写固件识别结果。全双工 TX 可使用同一控制器的 TX 或 RX 成对时钟，不能混接两组时钟。

目前支持标准从设备 I2S、正常 LRCK 极性，16/24/32 位容器，RX 24 位紧凑或填充打包。源与原有输出桥的有效精度仍为 PCM16；补足的低位为零。外部输入须按单声道复制到两个 lane；TX 保留首个左 slot，右声道选择和仅右 slot 输出暂不支持。序列格式、未知字长、ALC/自动静音、DRC、ADC/DAC 内部回环等不在此子集内，相关通路保留静音。未模拟逐比特边沿、MCLK/分频锁定、上电延迟、EQ/HPF、软斜坡、模拟噪声、耳机/功放负载或电气时序。

实现的控制包括 `00h` CSM/数字复位、`01h` 各方向时钟使能、`0Dh/0Eh/12h/14h` 电源和输入选择、`09h/0Ah` 静音、`31h` DAC 静音/反相，以及 `14h/16h/17h/32h` PGA、ADC scale、数字音量与反相。音量 `BFh` 为 0 dB、每级 0.5 dB；PGA 每级 3 dB，ADC scale 每级 6 dB。稳态增益在寄存器写入时计算，采样四舍五入并饱和到 PCM16；不改变采样率、语速或音高。ADC 源定义为增益前的数字参考，不能视为已校准的实际麦克风声压。

`I2s.bytes_in` 在 S3 上累计成功写入 DMA 的载荷字节，包含发生后续故障之前已写的前缀；描述符回写不计入。静音仍产生零字节载荷并计数；停止或禁用 RX、不属于 DMA 的描述符、拒绝的目的地址不凭源样本数量增加计数。这个值证明 DMA 写入，不能单独证明应用已录音、识别或完成网络交互。

`cargo test --release -p esp32s3 --test cardputer_adv_audio` 通过实际控制器 I2C 事务与 S3 DMA 内存/PCM 断言验证上述边界；独立 GDMA 测试覆盖未对齐写入中途故障。它们属于可重复数字模型检查，未访问物理设备。精确提交、检查和原生性能调查见 [本次证据](../evidence/cardputer-adv-kernel-20261125/README.md)。

来源、真实像素截图、负面结果及最终检查见 [证据目录](evidence/cardputer-adv-official/README.md)。主要依据为 [官方产品资料](https://docs.m5stack.com/en/core/Cardputer-Adv)、[官方厂测源码](https://github.com/m5stack/M5Cardputer-UserDemo/tree/b549eac0a3c65bc108186c276b8fac0a214aaa4e)、[M5GFX 0.2.15](https://github.com/m5stack/M5GFX/tree/0.2.15)、[TI TCA8418 SCPS215G](https://www.ti.com/lit/ds/symlink/tca8418.pdf) 和 [Sitronix ST7789V2 v1.0](https://files.waveshare.com/upload/c/c9/ST7789V2.pdf)。
