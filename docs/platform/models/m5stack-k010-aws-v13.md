# M5Stack Core2 for AWS v1.3 · K010-AWS-V13 外观与知识入库

带 ATECC608B 安全芯片、BMI270、10 颗 RGB LED 和扩展底座的 Core2 for AWS v1.3；不同于原版 MPU6886 配置。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Core2_For_AWS_v1.3)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k010-aws-v13.json) 与 [source audit](../evidence/m5stack-k010-aws-v13/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Retained source assembled cluster; only the recorded rigid rotation and translation are applied. Original millimetres and relative positions preserved.

[model-plan.json](../../../web/platform/assets/m5stack-k010-aws-v13/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| cad-component-0 source-fidelity | 8.010864256391415e-07 | 0 | 通过 |
| light-diffuser-3 source-fidelity | 8.010864256391415e-07 | 0 | 通过 |
| cad-component-5 source-fidelity | 8.010864256391415e-07 | 0 | 通过 |
| light-diffuser-6 source-fidelity | 5.340576194612368e-07 | 0 | 通过 |
| width | 54.000003814697266 | 54 | 通过 |
| height | 54.0 | 54 | 通过 |
| depth | 23.511363983154297 | 23.7 | 未通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k010-aws-v13/verification.json)：

- front: pass; IoU 0.9925360240505892
- back: pass; IoU 0.9937895734937918
- USB-Grove-side: qualitative-only; IoU 0.9102870813397129
- SD-side: qualitative-only; IoU 0.8918509338524021

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01-extra.py prepare --ids m5stack-k010-aws-v13
python tools/onboard-lane-01-extra.py build --ids m5stack-k010-aws-v13
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k010-aws-v13`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k010-aws-v13-front.png` 等，再运行：

```sh
python tools/onboard-lane-01-extra.py verify --ids m5stack-k010-aws-v13
python tools/onboard-lane-01-extra.py publish --ids m5stack-k010-aws-v13
```

当前限制：

- CAD stack depth is 23.5 mm versus 23.7 mm specification; discrepancy retained.
- Security element, AWS identity, certificates and provisioned keys are not emulated or inferred.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- 本轮继续按真实厂家照片审核；刻字、部分贴纸、端子及内部PCB未全部还原，不能视为整机1:1通过。
