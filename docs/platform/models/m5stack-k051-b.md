# M5Stack Stamp-Pico DIY Kit · K051-B 外观与知识入库

ESP32-PICO-D4 模块 DIY 套件，包含下载器和连接配件；与 C050-B 五件装分开维护。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Stamp-Pico_DIY_Kit)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k051-b.json) 与 [source audit](../evidence/m5stack-k051-b/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Retained source assembled cluster; only the recorded rigid rotation and translation are applied. Original millimetres and relative positions preserved.

[model-plan.json](../../../web/platform/assets/m5stack-k051-b/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| cad-component-0 source-fidelity | 3.433227533378158e-07 | 0 | 通过 |
| cad-component-1 source-fidelity | 1.0490417512443173e-07 | 0 | 通过 |
| width | 18.0 | 18 | 通过 |
| height | 24.027936935424805 | 24 | 通过 |
| depth | 4.400000095367432 | 4.6 | 未通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k051-b/verification.json)：

- covered-front: qualitative-only; IoU 0.44323880502747426
- component-side: qualitative-only; IoU 0.9568273910499063
- board-back: qualitative-only; IoU 0.9599687992111499
- covered-angle: qualitative-only; IoU 0.48199784877371893

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01-extra.py prepare --ids m5stack-k051-b
python tools/onboard-lane-01-extra.py build --ids m5stack-k051-b
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k051-b`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k051-b-front.png` 等，再运行：

```sh
python tools/onboard-lane-01-extra.py verify --ids m5stack-k051-b
python tools/onboard-lane-01-extra.py publish --ids m5stack-k051-b
```

当前限制：

- Model covers the Stamp-Pico module only; downloader, headers and loose kit accessories have no matching assembly CAD in this directory.
- CAD thickness 4.4 mm differs from documented product 4.6 mm; retained as a failure rather than stretched.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Browser inspection identifies the connected STL as a heat-resistant shield; actual populated PCB geometry and edge pad assignments remain unknown.
- 本轮继续按真实厂家照片审核；刻字、部分贴纸、端子及内部PCB未全部还原，不能视为整机1:1通过。
- Heat-resistant printed film uses an independently recorded four-corner perspective rectification; oblique-source uncertainty remains. Central fastening screw geometry is still absent, not painted into the film.
