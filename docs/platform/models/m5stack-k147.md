# M5Stack AtomS3R AI Chatbot Kit · K147 外观与知识入库

由 AtomS3R 与 Atomic Voice Base 组成的语音交互套件；彩屏、IMU、磁力计和音频部件分别记录。

来源：[官方产品文档](https://docs.m5stack.com/en/core/AtomS3R-AI_Chatbot)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k147.json) 与 [source audit](../evidence/m5stack-k147/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Retained source assembled cluster; only the recorded rigid rotation and translation are applied. Original millimetres and relative positions preserved.

[model-plan.json](../../../web/platform/assets/m5stack-k147/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| cad-component-0 source-fidelity | 4.76837158203125e-07 | 0 | 通过 |
| cad-component-1 source-fidelity | 4.76837158203125e-07 | 0 | 通过 |
| cad-component-4 source-fidelity | 4.76837158203125e-07 | 0 | 通过 |
| width | 24.000011444091797 | 24 | 通过 |
| height | 24.000011444091797 | 24 | 通过 |
| AtomS3R-depth | 12.899999856948853 | 12.9 | 通过 |
| base-depth | 10.000000953674316 | 14.14 | 未通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k147/verification.json)：

- front: pass; IoU 0.983504062727244
- back: pass; IoU 0.990673807408119
- USB-Grove-side: qualitative-only; IoU 0.4034132857167385
- vent-side: qualitative-only; IoU 0.39962913998248717

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01-extra.py prepare --ids m5stack-k147
python tools/onboard-lane-01-extra.py build --ids m5stack-k147
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k147`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k147-front.png` 等，再运行：

```sh
python tools/onboard-lane-01-extra.py verify --ids m5stack-k147
python tools/onboard-lane-01-extra.py publish --ids m5stack-k147
```

当前限制：

- Official specifications list AtomS3R 12.9 mm and separate base 14.14 mm; source assembled stack is 22.9 mm with a 10 mm exposed base. Base mating/envelope convention is unverified.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- 本轮继续按真实厂家照片审核；刻字、部分贴纸、端子及内部PCB未全部还原，不能视为整机1:1通过。
