# M5Stack Xiaozhi Card Kit · K146 外观与知识入库

带 2.7 英寸触摸墨水屏、音频和 LTE Cat.1 底座的小智语音卡片套件；手持机与底座分项保留尺寸。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Xiaozhi_Card_Kit)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k146.json) 与 [source audit](../evidence/m5stack-k146/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Handset source cluster 0/1/2/8 retained, plus source-positioned dock pieces 6/10/13/15. Tiny fragments 1/2 remain source-fidelity fragments rather than invented components. Product-envelope checks explicitly apply to handset component 0; full kit bounding box is not equated with handset size.

[model-plan.json](../../../web/platform/assets/m5stack-k146/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| cad-component-0 source-fidelity | 1.1920928955078125e-07 | 0 | 通过 |
| cad-component-1 source-fidelity | 0.0 | 0 | 通过 |
| cad-component-2 source-fidelity | 0.0 | 0 | 通过 |
| cad-component-6 source-fidelity | 0.0 | 0 | 通过 |
| cad-component-8 source-fidelity | 0.0 | 0 | 通过 |
| cad-component-10 source-fidelity | 0.0 | 0 | 通过 |
| cad-component-13 source-fidelity | 0.0 | 0 | 通过 |
| cad-component-15 source-fidelity | 0.0 | 0 | 通过 |
| handset-width | 54.0 | 54 | 通过 |
| handset-height | 84.0 | 84 | 通过 |
| handset-depth | 8.0 | 8 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k146/verification.json)：

- handset-front: qualitative-only; IoU 0.8457586601137359
- handset-back: qualitative-only; IoU 0.857761618173545
- edge: qualitative-only; IoU 0.20108970967342482
- dock: qualitative-only; IoU 0.3153776822170098

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01-extra.py prepare --ids m5stack-k146
python tools/onboard-lane-01-extra.py build --ids m5stack-k146
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k146`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k146-front.png` 等，再运行：

```sh
python tools/onboard-lane-01-extra.py verify --ids m5stack-k146
python tools/onboard-lane-01-extra.py publish --ids m5stack-k146
```

当前限制：

- Handset and dock are separate products in the kit; the CAD dock includes stand geometry beyond the 84×54×8 mm handset. No dock dimensions pass is asserted.
- Dock latch component identity is unlabelled in STL; retained source positions are not an electrical or assembly-service claim.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- 本轮继续按真实厂家照片审核；刻字、部分贴纸、端子及内部PCB未全部还原，不能视为整机1:1通过。
- Default scene retains native handset and dock cluster;54x84x8mm belongs to handset only, complete dock assembly envelope not certified by that size. Dock geometry is not scaled into handset dimensions.
