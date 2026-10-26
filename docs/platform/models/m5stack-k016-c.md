# M5Stack StickC · K016-C 外观与知识入库

ESP32-PICO-D4 棒状终端，含 0.96 英寸屏幕、MPU6886、AXP192 与 95 mAh 电池；资料与 Plus、StickS3 分开维护。

来源：[官方产品文档](https://docs.m5stack.com/en/core/StickC)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k016-c.json) 与 [source audit](../evidence/m5stack-k016-c/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Manufacturer source is separated along Z. Components 2/3/4 are rigidly translated by -13.189 mm in source Z before the global Y-axis half turn. This assembly position is an enclosure-seam estimate, not a documented mating transform; no geometry scaled. All source vertices and transforms are retained for review.

[model-plan.json](../../../web/platform/assets/m5stack-k016-c/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| cad-component-0 source-fidelity | 0.0 | 0 | 通过 |
| cad-component-1 source-fidelity | 0.0 | 0 | 通过 |
| cad-component-2 source-fidelity | 1.2969970697440658e-07 | 0 | 通过 |
| rear-fastener-3 source-fidelity | 1.2969970697440658e-07 | 0 | 通过 |
| rear-fastener-4 source-fidelity | 1.2969970697440658e-07 | 0 | 通过 |
| width | 24.0 | 25.5 | 未通过 |
| height | 48.0 | 48.2 | 未通过 |
| depth | 13.004759788513184 | 13.7 | 未通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k016-c/verification.json)：

- marketing-angle: qualitative-only; IoU 0.256834375
- back: pass; IoU 0.9545566126415241
- front-package: qualitative-only; IoU 0.5863828276052558

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01-extra.py prepare --ids m5stack-k016-c
python tools/onboard-lane-01-extra.py build --ids m5stack-k016-c
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k016-c`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k016-c-front.png` 等，再运行：

```sh
python tools/onboard-lane-01-extra.py verify --ids m5stack-k016-c
python tools/onboard-lane-01-extra.py publish --ids m5stack-k016-c
```

当前限制：

- Assembled shell estimate is 24×48×13 mm, smaller than current document 25.5×48.2×13.7 mm; all dimensional failures remain visible. The directory may represent another housing revision.
- Source assembly transform is estimated. Do not treat this model as a confirmed 1:1 match to current K016-C housing.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- 本轮继续按真实厂家照片审核；刻字、部分贴纸、端子及内部PCB未全部还原，不能视为整机1:1通过。
- Source separated shells are only provisionally assembled by recorded translation; rear small pieces are fasteners, not user keys. No exact enclosure assembly certification.
