# 原版 Cardputer K132 / K132-V11 运行范围

两款原版 Cardputer 使用同一个 `cardputer` 板级模型：ESP32-S3、8 MB Flash、无 PSRAM，ST7789V2 的实际 240×135 可视窗口，以及 74HC138 选行的 56 键矩阵。它们与 Cardputer-Adv 的 TCA8418 键盘不是同一实现。

K132 和 V1.1 官方页面分别链接 `/481/Sch_M5Cardputer.pdf` 与 `/1127/Sch_M5Cardputer.pdf`；本次下载两份内容的 SHA-256 均为 `6016c1fe6099b9e3f0d0c88f07a3fba1aa44a3da8a7a91dd7040ad0fffa2041e`。共用范围限于已核对的 LCD、GPIO 键盘和 IR 检测负载。V1.1 的 Stamp 模块修订等产品差异仍保留，不据同图推断整机功能等价。

## 已实现路径

- LCD：GPIO35 MOSI、36 SCLK、37 CS、34 D/C、33 reset；使用实际 SPI 路由、ST7789 GRAM 和固件绘图。共用 `cardputer_lcd.rs` 与 Adv 的既有屏幕代码，不绘制预设固件截图。
- 键盘：74HC138 A0/A1/A2 接 GPIO8/9/11；七路输入按 GPIO13、15、3、4、5、6、7 排列。物理键位 `(row, column)` 对应 `selector = 3-row + (column 为偶数 ? 4 : 0)`，输入线索引为 `floor(column/2)`。
- GPIO 路由、IO_MUX 和输出使能决定选择线是否有效。组合逻辑响应在同一 GPIO 读取时间点生效，不改变 guest 时钟，不强制单指令执行。默认 waveform 板继续保留原有调度。
- 松开的开关释放输入，使用固件配置的上拉/下拉；闭合的开关连接到 74HC138 的选中 LOW 或未选中 HIGH。多键导致对向输出时使用 LOW 优先的数字近似；未建模模拟电流、串扰、鬼键和机械抖动。
- GPIO44 的红外 LED 经 22Ω 接地。官方发布包内的 M5GFX 利用输入上拉仍读 LOW 的特征识别原版 Cardputer；模型仅提供这一数字负载，在 GPIO 输出模式释放外部强制输入。它不代表红外光学发射已实现。
- G0 是 GPIO0 低电平按键；机械电源开关、音频、SD 卡、无线/HID、红外应用和电池模拟未通过整机验收。固件电池 ADC 目前读 0，菜单电量不是硬件状态证明。

ESP-IDF v4.4.6 的 S3 `gpio_reg.h`、`io_mux_reg.h`、`spi_reg.h`已核对：GPIO 输出选择 256、MCU_SEL bits12–14、FUN_IE bit9、FUN_PU bit8、FUN_PD bit7、PAD_DRIVER bit2、SPI SIO bit17。与 Adv 使用的 v5.4.2 在这些字段一致。详见来源清单。

## Agent 输入

```js
await espTester.input({ type: 'matrix-key', row: 2, column: 13, pressed: true });
await new Promise(resolve => setTimeout(resolve, 150)); // 主机时长；严格测试以 guest receipt/stats 为准
await espTester.input({ type: 'matrix-key', row: 2, column: 13, pressed: false });
```

输入坐标是物理 4×14 键位；Enter 为 `(2,13)`，右为 `(3,12)`，左为 `(3,10)`。API 与 Adv 共用消息形状，`capabilities().input.keyboard.model` 分别返回 `gpio-matrix-74hc138` 与 `tca8418`。按键回执只表示已排入真实设备输入，仍须等待固件输出。暂停时允许释放，不允许新按下。

## 官方固件和验证

M5Burner 的 `Cardputer User Demo`（fid `d7ffe780b6414ca0135b2052494a6e2d`）发布 v1.1，日期 2025-01-11：

- URL：`https://m5burner-cdn.m5stack.com/firmware/7c269fa466d3b5069e891ba6dce8fe8f.bin`
- 3,020,384 bytes；SHA-256 `66576b6f670a2a1a75c181948aaadad89c4f1be61747acff8a5031187d043b27`。
- HTTP GET 200、`Access-Control-Allow-Origin: *`；本地探测仅在 RAM 中读取固件，不将第三方二进制镜像到仓库/服务器。用户浏览器可使用校验后的本地缓存。
- 实际串口报告 ESP-IDF v4.4.6、`f0b37ae-dirty`，启动画面 V1.1。公共基础提交 `f0b37ae5ebe709925fa54cca643418d96210f216` 可访问，但有 dirty 改动，且实际 M5GFX 检测路径与该提交子模块不同，不能声称精确复现构建。当前 main `a34d7ebcd508fb903a2b1123be8c8b54abd55d07` 的键盘扫描相同，Fn/HID 逻辑后来有变更。

可复现实测命令：

```sh
node tools/probe-official-cardputer.mjs \
  https://m5burner-cdn.m5stack.com/firmware/7c269fa466d3b5069e891ba6dce8fe8f.bin \
  66576b6f670a2a1a75c181948aaadad89c4f1be61747acff8a5031187d043b27 \
  output/official-cardputer 8 controls
# V1.1 使用独立产品/存储作用域，复测相同已核对硬件路径：
ESP32SIM_PROBE_PRODUCT_ID=m5stack-k132-v11 node tools/probe-official-cardputer.mjs \
  https://m5burner-cdn.m5stack.com/firmware/7c269fa466d3b5069e891ba6dce8fe8f.bin \
  66576b6f670a2a1a75c181948aaadad89c4f1be61747acff8a5031187d043b27 \
  output/official-cardputer-v11 8 controls
```

验收要求实际识别 `board_M5Cardputer`、完整启动画面、Enter 释放进入菜单、右键选 CHAT、左键回 RECORD。菜单动画初始和返回位置有最多 2 px 水平差异；检查 160×75 内区在有限整数平移后逐像素完全相同，不使用宽松颜色误差。原始全帧 hash 失败仍保留。早期缺 IR 负载被误识别成 VAMeter 的裁切画面不算通过。

来源、截图、输入时间、负结果、构建 hash、回归和性能检查在 [官方固件证据](evidence/cardputer-original-official/README.md)。未刷写实体设备，未做实机对比。此路径是产品级部分兼容，不能据菜单中存在某个应用就称该应用的所有外设已实现。
