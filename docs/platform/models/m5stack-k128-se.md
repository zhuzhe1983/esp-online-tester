# M5Stack CoreS3-SE · K128-SE 外观与知识入库

ESP32-S3 触摸屏终端，含 AXP2101、RTC 与音频；按 SE 文档维护，不引入标准 CoreS3 的摄像头和 IMU。

来源：[官方产品文档](https://docs.m5stack.com/en/core/CoreS3-SE)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k128-se.json) 与 [source audit](../evidence/m5stack-k128-se/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Retained source assembled cluster; only the recorded rigid rotation and translation are applied. Original millimetres and relative positions preserved.

[model-plan.json](../../../web/platform/assets/m5stack-k128-se/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| cad-component-0 source-fidelity | 2.8610229563241774e-07 | 0 | 通过 |
| cad-component-2 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| width | 54.0 | 54 | 通过 |
| height | 54.0 | 54 | 通过 |
| depth | 16.510000228881836 | 15.5 | 未通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k128-se/verification.json)：

- front: pass; IoU 0.9793899965759739
- back: pass; IoU 0.9898113789568231
- USB-Grove-side: qualitative-only; IoU 0.813775455805523
- SD-side: qualitative-only; IoU 0.7898176777908092

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01-extra.py prepare --ids m5stack-k128-se
python tools/onboard-lane-01-extra.py build --ids m5stack-k128-se
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k128-se`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k128-se-front.png` 等，再运行：

```sh
python tools/onboard-lane-01-extra.py verify --ids m5stack-k128-se
python tools/onboard-lane-01-extra.py publish --ids m5stack-k128-se
```

当前限制：

- Source lower cover reaches overall 16.5 mm depth; table states 15.5 mm. No stretching or cover removal was used to force a pass.
- No camera, IMU or battery capacity has been copied from sibling CoreS3 configurations.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- 本轮继续按真实厂家照片审核；刻字、部分贴纸、端子及内部PCB未全部还原，不能视为整机1:1通过。
- Rear M-BUS open housing, four silver fastener heads and rubber feet are photo-derived external details; dimensions/depth are estimates, not manufacturer connector CAD.
