# M5Stack StickT2 K016-T2

StickT2 使用 ESP32-PICO-D4、4 MiB Flash、135×240 ST7789 屏幕和 FLIR Lepton 3.0。CLI、网页和嵌入页共用 classic ESP32 的 `stickt2` 板型，结构和引脚依据见[官方产品页](https://docs.m5stack.com/en/core/StickT2)、[固定版本的原厂源码](https://github.com/m5stack/M5-StickT/tree/a502cb40e0837bccd55a3e09389f3cbfc155cea8)和[来源清单](evidence/stickt2-20261122/sources.json)。

| 通路 | 支持范围 |
| --- | --- |
| ST7789 | G13/G15/G23/G18/G5 分别连接时钟、MOSI、D/C、复位和片选。客体通过 SPI 写入 GRAM，按厂家表裁切和旋转 135×240 可见窗口。 |
| 按键和 IMU | A/B 对应低电平 G37/G39。MPU6886 位于 SDA21/SCL22 的 `0x68`，支持轮询寄存器和主机注入运动数据。 |
| AXP192 | I²C `0x34`，沿用本型号的初始供电配置。LDO2 控制背光，LDO3 控制 LCD 芯片。Power 短按经 PEK 状态位送达客体。 |
| Lepton | CCI `0x2a`，复位 G26，VSYNC G38，SPI 读出沿 G2/G25/G0。支持初始化命令、简化同步及四段 Raw14 VoSPI 包。原厂程序读取合成计数并经 LCD 绘制伪彩色。 |

合成场可设背景和热点计数（1–16383）、热点中心（X 0–159、Y 0–119）和半径（0–160）。屏幕颜色和数字只用于检查固件采集和渲染。环境面板、`input({type:'thermal-environment', background, hotspot, centerX, centerY, radius})`、分享和录制回放共用该输入。录制从初始场开始，并按客体确认的时间记录后续输入。

真实温标、光学响应、完整 FFC 和 VoSPI 时序、RTC、麦克风、电池模拟与长按电源仍待实现。旋钮主机驱动使用 I²C `0x30`，押键读 `0x10`，累计增量读 `0x00..0x01`，向 `0x20` 写 `0xff` 清零。副控固件及 PA2/PA3/PA4 的归属未查明，板型保留地址 NACK。现有厂家 CAD 保留安装耳和原始尺寸，正面 A 键热区采用照片估算，侧面 B 键和 Power 通过 2D、键盘与 API 操作。

## 原厂镜像和验证边界

`m5stack-k016-t2-official-factory` 指向 M5Burner 原厂 v1.0 的 4,194,304 字节整镜像，完整 SHA-256 为 `bb33ffd41f9c6f9235b2fb44eac893b7ce6f70879f48eed9cb1ff75ef9419cd7`。启动镜像位于 `0x1000`。原始 SPI mode 字节为 `3`，与镜像内附的启动段摘要不一致；诊断时仅将副本中该字节改为 `2` 就能重算出存储值。加载仍走现有分区和芯片检查、完整文件哈希校验，并向 ROM 传入下载原始字节。固件保持 `trial`，`simulationVerified` 和 `hardwareValidated` 保持 `false`。[摘要诊断](evidence/stickt2-20261122/factory-integrity.json)

[历史记录](evidence/stickt2-20261122/historical.json)保留了共享 I²C FIFO 修复前后的结果。修复前三秒内地址阶段均解码为 `0x00`，客体报告 `error=2`，CCI 和 VoSPI 计数为零。修复后使用同一镜像各运行八秒，两个合成场的客体热点在屏幕上移动约 `(+30, -40)` 像素。历史浏览器记录使用另一份 WASM，文件哈希一并保留。原始日志、截图和会话细节依照[裁剪记录](evidence/stickt2-20261122/redactions.json)保存在测试输出目录或以哈希引用。

2026-11-23 原生复验复现了前置修订的 `error=2`。应用共享 FIFO 修复后，原厂程序在两次八秒运行中均执行 104 条 CCI 命令、读出 12,360 个 VoSPI 包并写入 LCD 139 次。显示热点从 `[69.52, 84.83]` 移到 `[99.52, 44.83]`。[复验收据](evidence/stickt2-20261122/native-20261123.json)

## 复验

将原厂 BIN 和公开的 `esp32_rev300_rom.elf` 路径交给探针。原生探针会校验两份 SHA-256 和启动段摘要负例，运行两种合成场并检查 CCI 初始化、完整 VoSPI 帧和客体绘制的热点位移。

```sh
node --test tools/platform-stickt2-test.mjs tools/platform-stickt2-environment-test.mjs
cargo test -p esp32 --lib stickt2
STICKT2_FACTORY_BIN=/path/to/original.bin ESP32SIM_ROM_DIR=/path/to/roms \
  cargo test --release -p esp32 --test stickt2_factory -- --ignored --nocapture
```

从当前源码运行 `tools/wasm-build.sh` 后，可以用 `tools/stickt2-preview.mjs` 启动仅监听回环地址的 2D 预览。浏览器探针复用已安装的 Playwright 和 Chromium，从目录声明的原厂 URL 加载同一份本地校验副本，检查工作台、mini 分享、iframe RPC、录制与回放产生的客体画面。截图和原始探针结果写入 `target/stickt2-research/browser/`。

```sh
PLAYWRIGHT_MODULE=/path/to/node_modules/playwright/index.mjs \
PLAYWRIGHT_BROWSERS_PATH=/path/to/playwright/browsers \
STICKT2_FACTORY_BIN=/path/to/original.bin ESP32SIM_ROM_DIR=/path/to/roms \
  node tools/stickt2-browser-test.mjs
```
