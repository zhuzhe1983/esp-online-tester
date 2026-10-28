# T-Embed CC1101 V1.0 部分运行适配

限定原版 V1.0（2024-07-29 原理图），不覆盖 Plus 或 V1.1。固定官方 `factory.bin` 从 ROM 启动；实际 native、网页 Worker 和浏览器已验证菜单、320×170 屏幕、GPIO0/6、GPIO4/5 编码器及 GPIO14 的八颗 RGB 输出。产品保持 `partial`，默认出厂固件保持 `trial`，未实机验证。

## 电气依据与限制

厂家提交 `c36826e0f91311cb591c4545fccf91e254fc55dc` 的 `utilities.h`、`Setup214_LilyGo_T_Embed_PN532.h`、原理图和出厂程序共同确定接线。来源 URL、实际抓取时间、大小、SHA-256 见 [sources.json](evidence/lilygo-tembed-runtime/sources.json)；已有原理图与引脚来源保留在产品知识记录。镜像 SHA-256 为 `e7a00ad05e6228d1508c82c79993c2bde25c6be415df611a4b7cae66c119c050`，未提交第三方 BIN。

- LCD：SCLK11、MOSI9、CS41、D/C16、背光21。按真实 SPI 路由接收 DCS 命令，缺少 D/C 或错误路由不会写屏。固定玻璃方向与厂商 rotation 3 对齐，保留 ST7789 GRAM、睡眠和显示开关。
- 原理图的 LCD 使用常供电 VDD3V3；GPIO15 开关的是 RGB 所用 VCC3V3。LCD reset 为 RC 上拉，厂商驱动设置 `TFT_RST=-1`，不把音频 GPIO40 接成 reset。
- 编码器 A4/B5，每次 API 步进发送完整四相电平；该固定出厂程序每两个边沿计一次，因此菜单移动两项。界面每圈 16 步是交互约定，实物机械卡点未标定。
- BQ25896 只实现 I²C GPIO8/18 上的身份、配置、只读寄存器和软件 reset 子集。依据 TI SLUSC76C（2018-05）§9.3.1、§9.4；Table 6 将 EN_ILIM 默认值定义为 1，与 Figure 25 不一致，本模型采用字段定义。ADC 保留上电值，不生成测量完成；充电、模拟电量、看门狗计时和 OTG 未实现。
- PN532/BQ27220 未实现，官方启动检测仍显示 `Mixed / Unknown allowMenu=1`，没有修改固件或移除硬件检查。电池、NFC、CC1101、SD、音频、红外、无线射频及整机睡眠均不计为通过。

## 实际输出验证

[worker.json](evidence/lilygo-tembed-runtime/worker.json) 记录平台 API → 实际 Worker → ROM 固件结果。以下 RGB565 FNV64 与独立 native PNG 逐像素还原后的结果相等：

| 输入与页面 | FNV64 |
| --- | --- |
| GPIO6 进入菜单 | `61fc3a6e3c6aff88` |
| 顺时针四相，菜单移两项 | `df3395d50c34b146` |
| 逆时针四相，恢复菜单 | `61fc3a6e3c6aff88` |
| 用户键八次、旋钮按压，TFT 信息页 | `b1bdc75bf4e7ed76` |

随后进入官方 WS2812 页，实际 RMT 输出八颗 `[24,0,0]`；GPIO6 将亮度改为八颗 `[48,0,0]`。退出页面后客体 `strip.clear()/show()` 输出全黑。不是前端灯光动画或直接写 framebuffer。

浏览器检查使用鼠标拖动真实 CAD 旋钮、点击中心及空格映射 GPIO6，再验证相同固件帧；18 项正面/斜视/背面可见性检查覆盖按压、旋钮及屏幕四角。分享 mini 恢复精确型号、固件 hash 与编码器能力，无页面错误。[浏览器记录](evidence/lilygo-tembed-runtime/browser.json)、[实际 TFT 页面](evidence/lilygo-tembed-runtime/browser-tft.png)。所有任务浏览器已关闭。

屏幕中心按 CAD 前框开窗 x=-32.01…11.86、y=-11.92…11.92 mm 修正，避免左侧文字被遮挡。活动区尺寸仍为照片估算，玻璃距前框 0.29 mm 是安装深度估计，未宣称厂家面板 CAD。旋钮直接使用原 CAD 部件。侧键位置与 GPIO6 对应关系未确认，因此侧面不猜测映射；使用操作面板或空格键。RGB 链序与灯孔尚未标定。内部 PCB 继续标为 UNKNOWN。

## 复现

```sh
cargo test --release -p esp32s3 board::tembed_cc1101 --lib
cargo clippy -p esp32s3 --all-targets -- -D warnings
tools/wasm-build.sh
TEMBED_FACTORY_BIN=/path/to/factory.bin ESP32SIM_ROM_DIR=/path/to/roms node tools/tembed-factory-test.mjs
```

浏览器使用 Playwright wrapper 打开此 checkout 的 `/platform/`，将同一镜像放在忽略目录 `output/tembed/factory.bin`，执行 `run-code --filename tools/check-tembed-browser.js`。脚本在精确官方 URL 上返回缓存字节，保留运行时 SHA/布局检查，不替换镜像。测试结束以同一 session 执行 `close` 并确认退出。

四项 Rust 板级边界测试覆盖错误 SPI/DC、显示开关与背光、RGB 错引脚/断电/重新输出以及 BQ 身份/只读/reset。无共享 tick 或设备调度修改，无 golden 更新。最终整体 CI 结果记录在本轮集成收尾报告。
