# Stamp-S3A S007-V033 研究检查点

尚未启用公开目录。S3A 仍是 ESP32-S3FN8、8 MB Flash、G0 按键与 G21 WS2812B；与基础 S3 的关键差别是 RGB 的 BL_3V3 供电来自 AW35122FDR，EN 连接 GPIO38 / DISP_BL。官网对比表也明确说明 RGB 电源与预留 FPC 背光复用。v0.3.3 原理图的主稳压器标 JW5712，而官网规格仍写 MUN3CAD01-SC，保留冲突，不推断模拟电源行为。

官网链接的 S3A Arduino Quick Start 明确 `LED_EN 38` 并设为输出高；UIFlow2 Quick Start 指向通用 stamps3 页，后续须核对该官方镜像在 GPIO38 上实际输出。不能只复制基础版启动蓝灯的结论。

拟交付数字子集：真实 GPIO38 输出高时才允许 G21 RMT 写灯；电源关闭清除颜色，再开电源不自动恢复旧颜色，必须收到新的客体帧。输出使能/方向改变也要经过供电判断。GPIO0 用户按键独立验证；FPC 屏幕、模拟电源与网络仍不包含。

外观保持当前具体 SKU 厂家盖板和 UNKNOWN PCB，不把旧 Stamp-S3 的天线/按键 STEP 填进新版本。按钮和 LED 的新增位置以 S3A 官方照片/尺寸核对。来源实际抓取时间与 hash 见 [sources.json](evidence/stamps3a/sources.json)；无可证实出厂 BIN 时保持官方源码/教程入口，不将 UIFlow2 冒充 factory。
