# M5Stack DinMeter v1.1 · K134-V11 外观与知识入库

Stamp-S3A 驱动的 1/32 DIN 面板控制器，配 1.14 英寸彩屏、旋钮编码器、RTC 和两路 Grove 扩展。

来源：[官方产品文档](https://docs.m5stack.com/en/core/DinMeter_v1.1)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k134-v11.json) 与 [source audit](../evidence/m5stack-k134-v11/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Source component2 is main panel shell at x0..53,z-30..0; component4 is orange knob. Rotation X+90deg maps original outward -Y to +Z, centered by[-26.5,15,7.2]. Optional clamp0 and all translated duplicates excluded; no scaling.

[model-plan.json](../../../web/platform/assets/m5stack-k134-v11/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| shell source-fidelity | 9.5367431640625e-07 | 0 | 通过 |
| encoder-knob source-fidelity | 4.2915344167226976e-07 | 0 | 通过 |
| stamp-s3a-cover source-fidelity | 9.5367431640625e-07 | 0 | 通过 |
| width | 53.0 | 53 | 通过 |
| height | 30.0 | 30 | 通过 |
| depth | 29.55000114440918 | 32 | 未通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k134-v11/verification.json)：

- front: pass; IoU 0.9923098049986268
- back: pass; IoU 0.9901870974905619
- side: qualitative-only; IoU 0.36296015010721944

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。本次模型交互修订与实际官方固件的有限适配见 [运行记录](../dinmeter-v11-runtime.md)；不改变原有几何误差和未知内部件结论。

重生成：

```sh
python tools/onboard-lane-01.py prepare --ids m5stack-k134-v11
python tools/onboard-lane-01.py build --ids m5stack-k134-v11
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k134-v11`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k134-v11-front.png` 等，再运行：

```sh
python tools/onboard-lane-01.py verify --ids m5stack-k134-v11
python tools/onboard-lane-01.py publish --ids m5stack-k134-v11
```

当前限制：

- Manufacturer exterior CAD only; runtime now has a separately verified ST7789P3/encoder subset. Unsupported electronics remain explicit in the runtime report.
- No physical hardware measurements; colors are photograph-informed approximations, not calibrated materials.
- PCB has no audited complete board layout and is represented only by a visible neutral UNKNOWN placeholder.
- Bare panel configuration excludes the separate orange mounting clamp; its source component0 extends to38.5mm beyond the30mm panel and is not present in chosen photos.
- Rear Stamp-S3A PCB/connectors are not supplied in this shell STL; full32mm depth is not reached by the imported shell/knob alone. This failed envelope check is retained.
- StampS3A exterior cover is exact sourced component, but its fitted location and terminal bodies are photo estimates; complete rear PCB, white battery socket and screw not yet verified.

## PCB 穿模修正

旧版 43 × 22 × 1 mm 的矩形 PCB 占位板与后部导轨相交约 121.31 mm³，形成外壁上的灰色横线。两版厂家主壳 STL 内容相同，各用自身官方背照核对装配。v1.1 采用 40.6 × 15.6 mm 的深色占位板，厚度保留 1 mm 的原估算，板面与 Stamp-S3A 盖之间留出约 0.10 mm 的模型间隙。与主壳和 Stamp-S3A 盖的体积相交均为零，主壳最小采样间隙约 0.31 mm。厂家公开 STL 和照片覆盖可见外壳和后口，隐藏板形与元件位置仍为未知。 [官方产品页](https://docs.m5stack.com/en/core/DinMeter_v1.1) · [测量记录](../evidence/m5stack-k134-v11/pcb-clearance.json)

已用 Chromium 检查正面、背面、侧面，金色和半透明紫色下 PCB 保持自身材质。工作台的爆炸图沿厚度展开前壳、PCB 和后方 Stamp-S3A 盖。USB-C、两路 Grove 和完整的 DC 座随 `UNKNOWN` 板移动。转动后可从前侧斜视读取 `UNKNOWN`，复位恢复原装配。 [前后对照](../evidence/m5stack-k134-v11/pcb-clearance-comparison.webp)

两版共用 `python tools/verify-dinmeter-pcb.py --baseline da0738cd` 的网格测量和 `tools/fixtures/dinmeter-pcb-browser-check.js` 的浏览器复验脚本。
