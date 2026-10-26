# M5Stack Stamp-S3 PIN2.54 · S007-PIN254 外观与知识入库

预焊 2.54mm 排针的 ESP32-S3FN8 控制模块，带 USB-C、按键、RGB LED 和 23 路 GPIO。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Stamp-S3_PIN2.54)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-s007-pin254.json) 与 [source audit](../evidence/m5stack-s007-pin254/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Original STL cover in the same mechanical coordinate frame as the officially linked Stamp-S3 KiCad STEP. STEP native solids0/12/16/19/26/28 supply mechanical board outline, USB, antenna and screw. STEP solid27 is a differently aligned cover, excluded in favor of exact18×24mm Products STL. All use common Z translation-1.3mm; no scaling. Header pins are not present.

[model-plan.json](../../../web/platform/assets/m5stack-s007-pin254/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| shell source-fidelity | 7.152557390810443e-08 | 0 | 通过 |
| board-outline source-fidelity | 4.7328850527605937e-07 | 0 | 通过 |
| usb-tongue source-fidelity | 4.5377086266285005e-07 | 0 | 通过 |
| usb-shell source-fidelity | 4.7617376353059626e-07 | 0 | 通过 |
| usb-rear source-fidelity | 4.766015493373743e-07 | 0 | 通过 |
| antenna source-fidelity | 4.7563936789174477e-07 | 0 | 通过 |
| screw source-fidelity | 2.3741007737498876e-07 | 0 | 通过 |
| width | 18.000022888183594 | 18 | 通过 |
| height | 25.947933197021484 | 26 | 通过 |
| depth | 4.750001907348633 | 4.7 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-s007-pin254/verification.json)：

- front-oblique: qualitative-only; IoU 0.6654686204036494
- back-oblique: qualitative-only; IoU 0.6966870992458765

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01.py prepare --ids m5stack-s007-pin254
python tools/onboard-lane-01.py build --ids m5stack-s007-pin254
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-s007-pin254`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-s007-pin254-front.png` 等，再运行：

```sh
python tools/onboard-lane-01.py verify --ids m5stack-s007-pin254
python tools/onboard-lane-01.py publish --ids m5stack-s007-pin254
```

当前限制：

- Manufacturer exterior CAD only; board/display/peripheral runtime unavailable.
- No physical hardware measurements; colors are photograph-informed approximations, not calibrated materials.
- PCB has no audited complete board layout and is represented only by a visible neutral UNKNOWN placeholder.
- Official product text says4.6mm while the linked drawing says4.7mm; the conflict is retained, not treated as a known hardware revision.
- Mechanical STEP includes board outline/USB/antenna/screw; it does not provide complete populated PCB placement or copper traces.
- Header XY centers/count/pitch are official footprint facts; metal/body width and vertical extension are photo-informed estimates. Full fitted header height is unverified; module4.7mm depth excludes this added header.
