# PaperMono 官方固件运行范围

`papermono` 板级模型能够运行固定 hash 的 M5Stack UserDemo 1.2.0，生成真实 480×800 仪表盘，并处理 A/B 与触摸。ESP32-S3 执行官方二进制，SPI 写入 SSD1677 的两个 RAM 平面；只有固件发出刷新命令且 BUSY 完成，才发布像素。主机没有绘制仪表盘、替换页面或修改固件。

这是一条已验证的部分兼容路径，未做实机对照。PaperMono 使用 `papermono`，Lite 使用 `papermono-lite`；二者共用显示、按键、触摸及 SD/RGB 路径。完整版新增 ST25R3916 读卡器数字子集，官方程序检测为 Pro，实际无卡扫描结束显示 `NO CARD`。SX1262 数字报文子集已能初始化、发送 `Hello!`，并在显式输入匹配的虚拟对端报文后显示 `Rx PEER`；没有对端时不生成收包。Lite 不接入 NFC/LoRa 器件，保持 Lite 布局。

主线合并后的官方固件、Lite 区分、配件路由与真实网页复核见 [整合记录](evidence/papermono-integration/README.md)。旧页面中的 Worker 不会自动替换；刷新页面后重新启动即可，无需换固件或清空 Flash/TF 存储。

### NFC 与完整版识别

UserDemo 的 `Hal::detectBoardVariant()` 从 I2C `0x50` 读取身份寄存器（读命令 `0x7F`）；类型 `0x05` 且 revision 非零时选择 Pro，否则选择 Lite。新增模型包含 A/B/test 寄存器空间、512 字节有界 FIFO、复位、振荡器、数字发送、TX 完成和无应答计时器、GPIO6 中断及屏蔽/读清。固定官方程序实际发出 A/B/F/V 模式命令；没有 RF peer 时，不产生 RX 数据或虚构卡片 UID。

电源按 **2026-10-27 抓取的原理图内容**处理：虽然 PDF 地址带 `V0.6.2_20260522`，NFC 页标注 2026-07-28 / V0.2，U1 负载开关为 NC，R1=0Ω 旁路，主板注明 `PYB_NFC_EN` 为 spare GPIO。因此软件中的 `setNfcPower` / IOE4 名称不作为断电依据；NFC 随系统电源关闭而复位、NACK。现有 PMIC 模型用整机断电请求代表该电源域，尚未细化全部稳压轨。没有实测用户板次，不能据此推定所有量产修订都相同。

此扫描还暴露了已有 S3 SPI 模型遗漏：IDF 5.5.1 通过 SPI2/3 `DMA_INT_SET`（偏移 `0x44`）软件置位 `TRANS_DONE` 启动中断队列。此前 LoRa 初始化会卡住首事务，连带阻塞 UI；补齐真实寄存器行为后，缺失 SX1262 的错误路径能正常返回。没有跳过 LoRa 初始化，也没有修改官方固件。来源、负面结果和复现见 [NFC 阶段证据](evidence/papermono-nfc/README.md)。

### LoRa 数字报文子集

按原理图接入 PM1 GPIO2 控制的 LoRa 供电、IOE10 复位、IOE2 天线开关、GPIO21 BUSY、GPIO5 DIO1，以及 SCK39/MOSI38/MISO40/NSS41。官方程序使用 SPI3、软件 NSS 和 RX GDMA；新增 S3 RX DMA 沿用有界描述符遍历器，只访问内部 SRAM，错误描述符、MMIO 地址及不支持的 EOF 模式产生 DMA 错误。

模型处理真实寄存器/命令、256 字节 FIFO、LoRa 参数、发送完成、接收/超时 IRQ、屏蔽与清除、复位/断电。发送内容来自客体 FIFO，完成时刻依据配置计算空中时间；不模拟射频传播、干扰或真实接收强度。BUSY 使用手册典型值与明确的数字延迟，未作实机时序校准。GFSK、CAD、LR-FHSS、测距及远程无线中继未实现。

通过 `capabilities().lora` 检查支持范围，`readLoRa()` 读取状态及最近 16 个发送报文，`receiveLoRa(packet)` 显式提交虚拟对端参数、数据、RSSI/SNR 和 CRC 错误标记。频率、SF、带宽、编码率、同步字、包头、CRC 和 IQ 配置必须匹配；断电、复位、天线关闭或不在接收状态时拒绝。输入回执只表示已安排数字接收，必须继续检查客体读 FIFO、串口或画面。报文不自动进入录制/分享，也不会自动回显或联网。详见 [LoRa API](../../skills/esp-online-tester/references/api.md#virtual-lora-packets) 与 [来源及闭环证据](evidence/papermono-lora/README.md)。

## 已实现的硬件路径

| 部件 | 路径与验证范围 |
| --- | --- |
| SSD1677 | SPI2，MOSI14/CLK15/CS16/DC17，BUSY18；800×480 原生 RAM 旋转为 480×800 肖像画面；窗口、地址自动递增/递减、双平面、更新控制、模式切换与 M5GFX 的四灰阶/差分 LUT 类别 |
| M5IOE1 / M5PM1 | I2C 0x4F / 0x6E；IO3 供电、IO5 复位控制 EPD 接收；IO6 复位、IO13 供电控制触摸；控制器断电/复位保留电子纸的已显示图像 |
| 软件与硬件 I2C | SDA47/SCL48；M5GFX 先用软件 I2C 探测，再用硬件 I2C1；两个控制器与软件探测共享同一组外设寄存器，不复制状态 |
| FT6336G | I2C 0x38、IRQ4；单点坐标、按下/释放、供电/复位门控；报告保留到驱动完成稳定双读，坐标直接采用 480×800 肖像坐标 |
| A/B | GPIO2 / GPIO3，低电平有效；通过原有 GPIO 输入 API 送入固件 |
| 可写 Flash | 使用通用设备存储：按产品、板型、芯片、容量和初始 Flash 内容隔离。见 [设备存储](device-storage.md) |

数字模型将控制操作 BUSY 设为 1 ms、刷新 BUSY 设为 20 ms，沿现有 guest cycle 队列调度。它们是明确的兼容性间隔，**不是实测电子纸刷新时间**。没有改变 CPU 时钟、推进倍率或通用调度器，也没有启用 EX218 的单指令量子。GPIO 软件 I2C 复用已存在的输出回调。

四灰阶端点按固定版本 M5GFX 的绝对/差分 LUT 语义解释，像素来自实际 RAM 位。它不模拟电压波形、温度、残影或未知固件自定义 LUT 的光学结果。用户不应据此判断真实电子纸的速度、灰度精度和寿命。

## 官方程序的实际输入验收

从同一原始固件初态启动，先等待实际 guest 时间到 15 s，再发送输入。每次按住和等待都以 worker 的实际 cycle 回执为基准。

| 输入 | 官方画面结果 |
| --- | --- |
| B 短按 | BL 30% → 20% |
| A 短按 | BL 20% → 30%，亮度标签像素恢复为初始 hash |
| 触摸 (45,78) | “+”使 BL 30% → 40% |
| 触摸 (412,500) | 静音图标变化 |
| 再次触摸 (412,500) | 静音图标像素恢复为原 hash |

测试按局部 framebuffer 区域比较 hash，避免把时钟或其他区域刷新误当输入成功。画面中的亮度文字变化证明固件接收了操作；模型未模拟真实前光的光照效果。静音图标变化证明 UI 状态切换，不是音频输出验证。

复现命令要求调用者显式传入远程地址及 hash，二进制仅进入进程内存：

```sh
ESP32SIM_PROBE_CLOCK_TIMEOUT_MS=60000 node tools/probe-official-papermono.mjs \
  https://m5burner-cdn.m5stack.com/firmware/376080df0d31b0026e9a67a75d02d571.bin \
  72c290bc2ffa216041b276660277369bd17ecef92315e367d98bb2c96c8428fa \
  output/papermono-check 15 controls
```

初次绘制的单个 WASM run 可能占用较长主机时间，因此探测器使用 60 s 回执窗口。早期 10 s 窗口超时记录保留；这不代表电子纸在实机需要相同时长。正常网站仍保留明确加载/准备和运行状态。

## 当前限制

- RX8130 仅提供寄存器 RAM，供板型探测和本次会话的固件设置使用。日期/时钟、闹钟、掉电保持和唤醒未实现；启动画面可见无效日期，不校正为主机时间。
- BMI270 的识别、配置和寄存器子集存在，但 PaperMono 没有持续采样时钟或姿态 API；UserDemo 的三次初始化检测失败。画面上的默认向量不是已验证传感器采样。
- NFC 已验证读卡器无卡路径；RF peer、卡内容/防冲突、卡模拟、模拟测量/校准与唤醒未实现。LoRa 仅支持上述数字报文子集。音频、深睡复位唤醒、Wi-Fi 扫描/NTP 和外接附件尚未在此板上验收；EDC Book 的轻睡眠范围见下方补充。TF/SD 与 RGB 的新增范围见下方补充。未实现功能的 INIT FAIL 或 NO DATA 保留，不伪造成功。
- M5IOE1 只对已接线的 TF 插卡检测提供数字输入，未实现通用输入上拉的电气网络；软件 I2C 使用理想上拉，不模拟总线争用、时钟拉伸或模拟电气特性。
- FT6336G 只支持单点报告；手势、第二触点、精确扫描节奏和面板固件版本未知。芯片/固件版本寄存器不伪造实测值。
- 远程二进制不进入本站服务器或 Git；浏览器可按既有策略本地缓存并复核 hash。此运行验收不代表刷机或实机验证。

来源版本、负面结果、最终截图及检查记录见 [证据目录](evidence/papermono-official/README.md)。来源依据为 [官方仓库](https://github.com/m5stack/M5PaperMono-UserDemo/tree/e55877dc04b89e13a8519cee5435dc326499e1ca)、[固定版 M5GFX](https://github.com/m5stack/M5GFX/tree/02107b828acb3eb782fccd747863638d64c876d7)、[官方产品资料](https://docs.m5stack.com/en/core/PaperMono)、Solomon Systech SSD1677 rev1.0、FocalTech FT6x06 Application Note 和 [NXP I2C 规范 rev7](https://www.nxp.com/docs/en/user-guide/UM10204.pdf)。

## TF 卡与 RGB 补充（2026-10-27）

TF 卡已接入 ESP32-S3 原生 SDMMC 主机，复用现有 SD 卡协议与 IDMAC 模型。接线按官方 `hal_tf_card.cpp`：CLK13、CMD12、D0/D1/D2/D3 为 11/10/9/8，IOE14 供电、IOE1 插卡低有效；没有把它误接为 SPI CS。主机核对 GPIO 矩阵、IO_MUX 输入使能、SYSTEM 时钟与复位、板上供电；错误路由不响应。DMA 只访问内部可 DMA SRAM，越界和 MMIO 地址产生主机错误，不能递归写设备或使模拟器 panic。卡媒体在 guest 芯片复位时保留，主机寄存器重置。

网站加载 PaperMono/Lite 固件时自动生成空白 64 MiB FAT32 卡；用户可导入 1–128 MiB、512 KiB 整数倍的完整磁盘镜像，也可导出或清空为默认 FAT32。导入和清空会先保存当前 Flash/SD、重新建立模拟器并停在 ready。SD 在浏览器 IndexedDB 单独存储，按产品、板型、初始完整 Flash SHA-256 隔离；导入镜像另保留初始 hash。自动保存变更的 4 KiB 页，基础镜像单独保存；软件重启、页面刷新和重新加载同一固件不清空。其他页面保存/清空后以版本和 epoch 防止旧写者覆盖。持久化不可用时显示仅本页内存；不能承诺关页保存成功。

接口：`attachSdCard({bytes?,format:'fat32'})`、`exportSdCard()`、`clearSdCard()`、`flushSdCard()`；状态在 `observe().sdCard`，事件为 `sd-card`。导出返回 `{bytes:Uint8Array,filename}`；上传字节和卡内容不进入分享 URL，非空白 SD 拒绝录制分享。关闭页面只做尽力保存，强杀进程/掉电前最后一批未提交写入可能丢失。

RGB 是三个独立通道：PM1 `PWR_CFG` bit4 驱动红光，IOE PWM2/IO8 驱动绿光，PWM1/IO9 驱动蓝光。PWM duty 为 12 bit，遵守输出模式、push-pull、使能与极性，向 viewer 发布 0–255 RGB；红光为开关值。固件驱动的 `leds` 事件和 `observe().leds` 是同一板级输出，前端不得根据点击动画伪造颜色。

CrossPoint 的 PaperMono 适配分支使用 native SDMMC + SdFat `FsVolume`。FAT32 是本平台默认且已由官方 UserDemo 实际挂载验证的格式；不能据此说该分支只支持 FAT32，也不能把其他 e-reader 的格式要求混用。该固定源码是当前适配分支，不声称可复现已发布二进制。当前收录的完整 CrossPoint 1.5.0 在 SD 挂载前发生启动异常，保留试运行入口与真实失败，尚未验证阅读、建库或卡内配置写入。细节及输入/截图/来源见 [SD/RGB 证据](evidence/papermono-sd-rgb/README.md)。


## Lite 板名与已有存储兼容

Lite 的执行板名改为 `papermono-lite`，旧 Flash/SD 持久化键继续使用原 `papermono` 板名；仅产品 ID `m5stack-c153-lite` 使用此别名。键内产品 ID、芯片、容量及初始 Flash SHA 仍参与隔离，完整版不会读到 Lite 数据。没有复制旧库、自动清空或在清空后回退恢复旧条目。回归覆盖旧页/卡恢复、手动清空、旧 writer epoch 拒绝和跨产品隔离。两种板型接受同系列固件 manifest；分享仍以产品 ID 选择真实执行板，板型兼容不等于固件功能已验证。


## EDC Book 3.1.2 轻睡眠与阅读（2026-10-29）

固定 SHA-256 `06eb3b595fc47bc5d35f760a16cdc5795ea8c0fad6295bcee8db5268cf906986`
的完整镜像使用 `papermono`、16 MiB Flash、8 MiB PSRAM，从真实 ROM 启动。
原程序进入 ESP32-S3 轻睡眠后等待 RTC 唤醒标志；此前模拟器没有处理睡眠，
因此触发了程序设置的安全看门狗。现在保留双核上下文，暂停数字时钟，继续 RTC
和板载外设的计时，并按程序设置处理定时器、GPIO、EXT0/EXT1 唤醒与看门狗暂停。

该固件还使用动态生成的 SSD1677 波形分块刷新页面。模型现在识别有效阶段中的
“保持原像素”和单极性黑白驱动，避免后续分块把已绘制的文字擦掉。混合极性和
VSH2 波形仍沿用原有四灰阶处理；EDC Book 短脉冲产生的光学灰度只作黑白端点
预览，不代表真实屏幕的抗锯齿、残影或刷新速度。

验收以实际 framebuffer 和客体输入为准，覆盖欢迎页、最近阅读、内置快速入门
指南及翻页。原始固件未修改，也无需清空 Flash 或 TF 卡。该结果只针对上述固定
版本与完整版 PaperMono；深睡、整机电源、网络功能、其他书籍格式及 Lite 上的
EDC Book 尚未验收。来源、负面结果和复现命令见
[轻睡眠与显示修复记录](../evidence/s3-light-sleep-edcbook/README.md)。
