# M5Stack AtomS3 · C123 外观与知识入库

ESP32-S3FN8 方形彩屏终端，含 128×128 GC9107 屏幕与 MPU6886；不混同 AtomS3R 的 IMU 和 PSRAM 配置。

来源：[官方产品文档](https://docs.m5stack.com/en/core/AtomS3)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-c123.json) 与 [source audit](../evidence/m5stack-c123/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Retained source assembled cluster; only the recorded rigid rotation and translation are applied. Original millimetres and relative positions preserved.

[model-plan.json](../../../web/platform/assets/m5stack-c123/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| cad-component-1 source-fidelity | 4.76837158203125e-07 | 0 | 通过 |
| cad-component-2 source-fidelity | 4.76837158203125e-07 | 0 | 通过 |
| width | 24.000015258789062 | 24 | 通过 |
| height | 24.0 | 24 | 通过 |
| depth | 12.899999618530273 | 12.9 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-c123/verification.json)：

- front: qualitative-only; IoU 0.933487594372287
- back: qualitative-only; IoU 0.681877602727726
- USB-side: qualitative-only; IoU 0.4793085385018334
- rear-angle: qualitative-only; IoU 0.24407655264407316

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 partial；GC9107、GPIO41 按键与 MPU6886 轮询子集的验证见 [运行时记录](../atoms3-runtime.md)。官方固件可尝试运行，未逐一验证。

公开重生成入口（使用 `tools/requirements-model.txt` 对应环境）：

```sh
python tools/rebuild-product-model.py plan m5stack-c123
python tools/rebuild-product-model.py build m5stack-c123 --scratch output/model-rebuild-m5stack-c123
```

每次选择新的 scratch 目录；默认离线，从仓库中固定 hash 的公开资产恢复输入，不依赖历史队列或本机抓取目录。工具只写 scratch，不覆盖公开资产。此次 [重建比较](../evidence/atoms3-runtime/model-rebuild.json) 确认几何字节一致、完整模型元信息语义一致，并保留屏幕盖板 `btnA`。旧轮廓指标及其测量时间原样保留；本次仅升级交互元信息与运行时限制，浏览器交互另见 [本次验证](../evidence/atoms3-runtime/browser.json)。

当前限制：

- Marketing image shows 12.8 mm depth while technical table states 12.9 mm; CAD matches the latter.
- Manufacturer STL lacks connector inserts, physical display assembly and PCB; blank screen plane and neutral board are explicitly estimates.
- Partial atoms3 board model: GC9107 SPI display, GPIO41 screen-cover button and MPU6886 polled I2C registers. PWM brightness, IR, IMU FIFO/interrupt/ODR, ST7735 panel variants and physical orientation calibration remain unimplemented or unverified; geometry preview is not firmware compatibility evidence.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- 本轮继续按真实厂家照片审核；刻字、部分贴纸、端子及内部PCB未全部还原，不能视为整机1:1通过。
