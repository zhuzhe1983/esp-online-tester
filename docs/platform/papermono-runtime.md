# PaperMono 官方固件运行范围

`papermono` 板级模型能够运行固定 hash 的 M5Stack UserDemo 1.2.0，生成真实 480×800 仪表盘，并处理 A/B 与触摸。ESP32-S3 执行官方二进制，SPI 写入 SSD1677 的两个 RAM 平面；只有固件发出刷新命令且 BUSY 完成，才发布像素。主机没有绘制仪表盘、替换页面或修改固件。

这是一条已验证的部分兼容路径，未做实机对照。PaperMono 和 Lite 共用此显示、按键与触摸路径；NFC 还未实现，官方固件因此检测为 Lite，不能以它证明 Pro 的 NFC/LoRa 功能可用。

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
- TF/SD、NFC、LoRa、音频、睡眠唤醒、Wi-Fi 扫描/NTP 和外接附件尚未在此板上验收。官方页面的 INIT FAIL 或 NO DATA 保留，不伪造成功。
- M5IOE1 未实现输入上拉的电气网络；软件 I2C 使用理想上拉，不模拟总线争用、时钟拉伸或模拟电气特性。
- FT6336G 只支持单点报告；手势、第二触点、精确扫描节奏和面板固件版本未知。芯片/固件版本寄存器不伪造实测值。
- 远程二进制不进入本站服务器或 Git；浏览器可按既有策略本地缓存并复核 hash。此运行验收不代表刷机或实机验证。

来源版本、负面结果、最终截图及检查记录见 [证据目录](evidence/papermono-official/README.md)。来源依据为 [官方仓库](https://github.com/m5stack/M5PaperMono-UserDemo/tree/e55877dc04b89e13a8519cee5435dc326499e1ca)、[固定版 M5GFX](https://github.com/m5stack/M5GFX/tree/02107b828acb3eb782fccd747863638d64c876d7)、[官方产品资料](https://docs.m5stack.com/en/core/PaperMono)、Solomon Systech SSD1677 rev1.0、FocalTech FT6x06 Application Note 和 [NXP I2C 规范 rev7](https://www.nxp.com/docs/en/user-guide/UM10204.pdf)。
