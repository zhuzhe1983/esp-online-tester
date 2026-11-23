# M5Stack StickS3 K150 板级适配

`m5stack-k150` 使用独立的 `sticks3` 板型，状态为 `partial`。原生运行器与浏览器 WebAssembly 共用板级实现。按键 A/B 低有效，屏幕为 135 × 240 且无触摸。可导入完整 ESP32-S3 本地镜像。板级诊断固件与两项 M5Burner 官方试运行固件各有独立的验收记录。

## 硬件与模拟范围

| 硬件 | K150 定义与实现 |
| --- | --- |
| SoC | ESP32-S3-PICO-1-N8R8，Flash 和 Octal PSRAM 各 8 MiB。S3 eFuse `PKG_VERSION=1` 用于 M5GFX 板型识别。 |
| LCD | ST7789P3，MOSI39/SCK40/DC45/CS41/RST21/BL38。RGB565 可见窗为控制器 GRAM 的 `(52,40)` 至 `(186,279)`。支持 DCS 睡眠、显示开关、复位和 GPIO/PWM 背光。 |
| LCD 电源 | M5PM1 GPIO2 推挽高电平打开 L3B。掉电复位 LCD 并清空模拟 GRAM。 |
| 内部 I²C | SDA47/SCL48，含 BMI270 `0x68`、M5PM1 `0x6e` 和 ES8311 `0x18`。两个控制器可通过同一物理总线访问。 |
| BMI270 | 接收完整 Bosch 配置传输并支持寄存器轮询。外部虚拟 IMU 样本可更改加速度和陀螺仪读数。中断和唤醒路径待实现。 |
| 按键 | KEY1/A 对应 GPIO11，KEY2/B 对应 GPIO12。电源键由 M5PM1 管理，PM1 复位、主控断电与下载模式仍待实现。 |
| 音频与红外 | ES8311 仅支持初始化寄存器和 ID。I2S 板级接线、AW8737 放大器以及 IR TX46/RX42 尚无模拟行为。 |
| 外设 | 目录保留 Grove G9/G10 和 Hat2-Bus 的 16 针定义。附件电气路由和模型插接待验证。 |

ST7789P3 读回 ID、18-bit 像素流、光学效果及全部旋转模式尚未验收。按键几何见 [K150 外观报告](models/m5stack-k150.md)。引脚、板载芯片和屏幕窗来自[厂商产品页](https://docs.m5stack.com/en/core/StickS3)及 [M5GFX 固定版本](https://github.com/m5stack/M5GFX/blob/65c816a38386062d2c3dc47d41237a9083acbc75/src/M5GFX.cpp#L2995-L3063)，逐源字节摘要见 [厂家资料记录](evidence/sticks3-runtime-20261122/sources.json)。

## 固件来源

M5Burner 精确设备 `STICKS3`（`1897100000000000020`）的 205 个项目中有 6 个官方项目，厂商描述均未指明出厂恢复用途。社区上传的 `StickS3_Factory_code` 保持社区来源，产品未设置出厂默认。调查时间、项目 ID、发布版本和元数据摘要见 [固件来源记录](evidence/sticks3-runtime-20261122/firmware-sources.json)。

| 固件目录 | 项目 / 固定版本 | 2026-11-23 普通下载 |
| --- | --- | --- |
| UIFlow2 v2.5.5 | `1897200000000001693` / `2122962646011428866` | HTTP 400 `BURN_CAPTCHA_INVALID` |
| 小智 v2.2.6 | `1897200000000001480` / `1897300000000003049` | HTTP 400 `BURN_CAPTCHA_INVALID` |

两个固定版本需要完成 M5Burner 验证才能下载。本次取得的响应为 JSON，镜像字节、发布方 SHA-256、布局及启动结果均缺失。UIFlow2 已撤回的旧版本 `2117450155839557633` 返回 HTTP 404 `VERSION_NOT_FOUND`。两项现行官方固件保持 `alternative`、`trial` 与 `simulationVerified: false`。[下载核对记录](evidence/sticks3-runtime-20261122/download-access.json)

## 诊断复现

使用 ESP-IDF v5.5.1 构建 [K150 诊断应用](../../tools/fixtures/sticks3-firmware)，在浏览器导入完整合并镜像。应用由 ESP32-S3 ROM 启动，经原有 I²C/SPI/GPIO 路径读取器件、输出屏幕像素，并按 A/B 状态更新颜色。源码、构建和浏览器操作见 [诊断说明](../../tools/fixtures/sticks3-firmware/README.md)。

```sh
cargo test -p esp32s3 --lib board::sticks3::tests
cargo test -p esp32s3 --test sticks3
cargo clippy -p esp32s3 --lib --test sticks3 -- -D warnings
node --test tools/platform-sticks3-test.mjs
```

Rust 测试覆盖屏幕窗口四角、错误 SPI/I²C 路由、PM1 掉电、背光和重启、BMI270 配置及寄存器采样。SoC 按键测试运行 Xtensa 指令验证单键、组合、保持和释放。目录测试覆盖 8 MiB 芯片布局、按键语义、模型几何摘要和 M5Burner 错误映射。

Headless Chromium 从真实 S3 ROM 启动合并诊断镜像。固件串口报告了 8 MiB PSRAM、PM1 `50`、BMI270 `24`、ES8311 `83/11`，并完成 BMI270 配置。默认 3D 视角中，正面 A 和右侧 B 点按均在固件里改变画面。2D 画面随键盘 A、A+B、B、全部释放依次呈现红、蓝、绿、白的中心方块。虚拟 X 轴 1 g 姿态使固件读数从 0 变为 4096。嵌入页恢复 K150 场景及 3D 模型，分享链接不携带本地固件。帧和镜像摘要见 [浏览器诊断记录](evidence/sticks3-runtime-20261122/verification.json)。

2026-11-23 在四款 Stick 的合并构建 `cd49997f` 上复跑了相同的诊断。串口、键盘与 3D 按键画面、IMU 读数及嵌入页结果均与单独构建一致。[合并构建复验](evidence/sticks3-runtime-20261122/combined-20261123.json)
