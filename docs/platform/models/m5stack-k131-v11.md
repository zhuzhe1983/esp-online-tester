# M5Stack Air Quality v1.1 · K131-V11 外观与知识入库

集成 SEN55、SCD40 与墨水屏的空气质量设备；保留 v1.1 的硬件身份与针脚资料。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Air_Quality_v1.1)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k131-v11.json) 与 [source audit](../evidence/m5stack-k131-v11/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Retained source assembled cluster; only the recorded rigid rotation and translation are applied. Original millimetres and relative positions preserved. Three additional instances reuse exact source clip7 with explicit rigid transforms based on the four visible mounting tabs in the official back photograph; copy placement is photo-inferred +/-0.4mm, not original manufacturer assembled coordinates.

[model-plan.json](../../../web/platform/assets/m5stack-k131-v11/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| cad-component-0 source-fidelity | 1.716613766689079e-06 | 0 | 通过 |
| cad-component-2 source-fidelity | 7.629394502828291e-07 | 0 | 通过 |
| cad-component-4 source-fidelity | 1.716613766689079e-06 | 0 | 通过 |
| cad-component-7 source-fidelity | 1.895427701015251e-06 | 0 | 通过 |
| cad-component-8 source-fidelity | 1.716613766689079e-06 | 0 | 通过 |
| mount-clip-photo-bottom-left source-fidelity | 1.895427701015251e-06 | 0 | 通过 |
| mount-clip-photo-top-left source-fidelity | 1.895427701015251e-06 | 0 | 通过 |
| mount-clip-photo-top-right source-fidelity | 1.895427701015251e-06 | 0 | 通过 |
| body-width | 56.0 | 56 | 通过 |
| body-height | 72.00000333786011 | 72 | 通过 |
| body-depth | 26.49999976158142 | 26.5 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k131-v11/verification.json)：

- front: fail; IoU 0.9444224474712279
- back: pass; IoU 0.9898824726916299
- sensor-side: qualitative-only; IoU 0.6650406380899719
- USB-side: qualitative-only; IoU 0.8519693749818055

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01-extra.py prepare --ids m5stack-k131-v11
python tools/onboard-lane-01-extra.py build --ids m5stack-k131-v11
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k131-v11`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k131-v11-front.png` 等，再运行：

```sh
python tools/onboard-lane-01-extra.py verify --ids m5stack-k131-v11
python tools/onboard-lane-01-extra.py publish --ids m5stack-k131-v11
```

当前限制：

- Manufacturer CAD contains one mounting clip. Three additional instances reuse its exact triangles with photo-inferred rigid assembly to match the four clips visible on the official product; positions are estimated +/-0.4mm, not independently verified assembly instructions.
- Dimension checks exclude the optional lug protrusion for body measurements; overall source geometry remains separately visible.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- 本轮继续按真实厂家照片审核；刻字、部分贴纸、端子及内部PCB未全部还原，不能视为整机1:1通过。
