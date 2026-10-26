# M5Stack PaperColor · C151 外观与知识入库

ESP32-S3R8 全彩电子纸终端，采用 4 英寸 400×600 屏幕，内置音频、温湿度、RTC、存储和 1250mAh 电池。

来源：[官方产品文档](https://docs.m5stack.com/en/core/PaperColor)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-c151.json) 与 [source audit](../evidence/m5stack-c151/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Original manufacturer components selected from assembled cluster; other components excluded with reasons. One rigid global transform, no scaling.

[model-plan.json](../../../web/platform/assets/m5stack-c151/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| shell-back source-fidelity | 1.895940300755683e-06 | 0 | 通过 |
| power-button source-fidelity | 1.8828964201134113e-06 | 0 | 通过 |
| button-b source-fidelity | 1.76368713056263e-06 | 0 | 通过 |
| button-a source-fidelity | 1.76368713056263e-06 | 0 | 通过 |
| corner-left source-fidelity | 1.8125915559608075e-06 | 0 | 通过 |
| corner-right source-fidelity | 3.8208008135143245e-07 | 0 | 通过 |
| button-c source-fidelity | 3.8208008135143245e-07 | 0 | 通过 |
| width | 70.8325424194336 | 70.83 | 通过 |
| height | 103.8825454711914 | 103.88 | 通过 |
| depth | 8.514999389648438 | 8.5 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-c151/verification.json)：

- front: pass; IoU 0.9776673890405562
- back: pass; IoU 0.9800392053379575
- side: qualitative-only; IoU 0.6103902368433747

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01.py prepare --ids m5stack-c151
python tools/onboard-lane-01.py build --ids m5stack-c151
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-c151`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-c151-front.png` 等，再运行：

```sh
python tools/onboard-lane-01.py verify --ids m5stack-c151
python tools/onboard-lane-01.py publish --ids m5stack-c151
```

当前限制：

- Manufacturer exterior CAD only; board/display/peripheral runtime unavailable.
- No physical hardware measurements; colors are photograph-informed approximations, not calibrated materials.
- PCB has no audited complete board layout and is represented only by a visible neutral UNKNOWN placeholder.
- Front e-paper cover is absent from shell STL; a dimension-drawing-based planar cover is explicit derived geometry, with optical stack/thickness unknown.
- Paper/optical projection is isolated from molded shell; small printed markings and hidden hardware not supplied by source CAD may remain incomplete.
