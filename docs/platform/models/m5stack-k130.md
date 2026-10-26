# M5Stack Dial · K130 外观与知识入库

ESP32-S3FN8 智能旋钮，采用 1.28 英寸圆形触摸屏，集成旋转编码器、RFID、RTC 与蜂鸣器。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Dial)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k130.json) 与 [source audit](../evidence/m5stack-k130/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Selected assembled origin cluster0/2/5/6/8/10/12/15/16; negative-source-Z front becomes+Z by rigid Y180deg and Z-2.16mm. Remaining source bodies are translated print copies.

[model-plan.json](../../../web/platform/assets/m5stack-k130/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| stamp-cover source-fidelity | 3.2424926743601645e-07 | 0 | 通过 |
| front-bezel source-fidelity | 1.5258789076710855e-07 | 0 | 通过 |
| inner-carrier source-fidelity | 1.5258789076710855e-07 | 0 | 通过 |
| grip-ring source-fidelity | 1.5258789076710855e-07 | 0 | 通过 |
| rear-shell source-fidelity | 3.2424926743601645e-07 | 0 | 通过 |
| rear-nut source-fidelity | 2.050399778852352e-07 | 0 | 通过 |
| stamp-bracket source-fidelity | 1.5258789076710855e-07 | 0 | 通过 |
| display-carrier source-fidelity | 1.0073184952830161e-07 | 0 | 通过 |
| spacer source-fidelity | 3.33786012163273e-08 | 0 | 通过 |
| width | 50.81659698486328 | 51 | 未通过 |
| height | 50.81659698486328 | 51 | 未通过 |
| depth | 30.334999084472656 | 32.3 | 未通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k130/verification.json)：

- front: pass; IoU 0.9784491316309455
- back: pass; IoU 0.9731339796752716
- side-oblique: qualitative-only; IoU 0.8628937468048012

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01.py prepare --ids m5stack-k130
python tools/onboard-lane-01.py build --ids m5stack-k130
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k130`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k130-front.png` 等，再运行：

```sh
python tools/onboard-lane-01.py verify --ids m5stack-k130
python tools/onboard-lane-01.py publish --ids m5stack-k130
```

当前限制：

- Manufacturer exterior CAD only; board/display/peripheral runtime unavailable.
- No physical hardware measurements; colors are photograph-informed approximations, not calibrated materials.
- PCB has no audited complete board layout and is represented only by a visible neutral UNKNOWN placeholder.
- Official page supplies nominal51×51×32.3mm; the selected source plastic assembly is50.8166mm diameter and30.32mm deep. Missing connector/panel stack or nominal difference is unresolved; source geometry not scaled.
- Circular active display is blank; no factory firmware frame is rendered.
- Rear connectors and populated PCB are absent from shell geometry.
- Rear terminal mouths are independent photo estimates; exact red/black Grove housings, power socket internals and stamp printed label remain partial.
