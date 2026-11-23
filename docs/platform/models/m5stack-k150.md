# M5Stack StickS3 · K150 外观与知识入库

ESP32-S3-PICO 棒状终端，集成彩屏、BMI270、音频、红外收发与 250 mAh 电池。

来源：[官方产品文档](https://docs.m5stack.com/en/core/StickS3)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k150.json) 与 [source audit](../evidence/m5stack-k150/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Retained source assembled cluster; only the recorded rigid rotation and translation are applied. Original millimetres and relative positions preserved.

[model-plan.json](../../../web/platform/assets/m5stack-k150/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| cad-component-0 source-fidelity | 9.262561810885472e-07 | 0 | 通过 |
| cad-component-3 source-fidelity | 4.196167004977269e-07 | 0 | 通过 |
| cad-component-4 source-fidelity | 9.512901293362575e-07 | 0 | 通过 |
| width | 24.000938415527344 | 24 | 通过 |
| height | 48.001869201660156 | 48 | 通过 |
| depth | 15.0 | 15 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k150/verification.json)：

- front: pass; IoU 0.9898767848282706
- back: pass; IoU 0.9754841077174956
- USB-Grove-end: qualitative-only; IoU 0.7681301352809362
- button-side: qualitative-only; IoU 0.4291570063989613

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 按 MIT 许可保留。官方照片及其裁切纹理版权归厂家，独立开放图片许可尚未核实。屏幕由模拟固件输出。`sticks3` 板型为部分适配，官方 UIFlow2 和小智均待下载并验证启动。[板级报告](../sticks3-runtime.md)

重生成：

```sh
python tools/onboard-lane-01-extra.py prepare --ids m5stack-k150
python tools/onboard-lane-01-extra.py build --ids m5stack-k150
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k150`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k150-front.png` 等，再运行：

```sh
python tools/onboard-lane-01-extra.py verify --ids m5stack-k150
python tools/onboard-lane-01-extra.py publish --ids m5stack-k150
```

当前限制：

- KEY1/KEY2 通过 GPIO11/12 接入；独立 PM1 电源键不映射为普通 GPIO，音频/IR 仍无板级路由验证。
- Labels, connector inserts and populated PCB are not part of the imported shell CAD.
- `sticks3` 板型实现 ST7789P3/PM1 屏幕电源门/BMI270 轮询子集；外观预览不证明官方固件兼容。
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- 本轮继续按真实厂家照片审核；刻字、部分贴纸、端子及内部PCB未全部还原，不能视为整机1:1通过。

## 按键区域和诊断

厂家组件 `cad-component-3` 对应 KEY1/G11。KEY2/G12 的右侧命中平面根据本型号侧照和背贴估算，误差约 1 mm。Chromium 工作台默认视角通过两个按键区域的悬停、按下和释放检查，诊断固件分别输出红色和绿色。屏幕中心仍可用于旋转设备。嵌入页可恢复 K150 模型。透明与金色材质、极端视角以及内部尺寸待进一步测量。内部 `pcb-main` 标记为 `UNKNOWN`，Grove 连接器的空间朝向尚未校准。[诊断记录](../evidence/sticks3-runtime-20261122/verification.json)
