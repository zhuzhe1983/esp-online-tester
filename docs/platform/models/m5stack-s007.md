# M5Stack Stamp-S3 · S007 外观与知识入库

ESP32-S3FN8 核心模块，提供 23 路引脚、USB-C、按键和 RGB LED；支持 SMT/DIP 与飞线连接。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Stamp-S3)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-s007.json) 与 [source audit](../evidence/m5stack-s007/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Retained source assembled cluster; only the recorded rigid rotation and translation are applied. Original millimetres and relative positions preserved. Secondary exact official STEP shares native coordinate system with STL cover; USB/antenna/screw retained without scaling. Front references include bare-board states while preview retains optional mechanical cover, so photo coverage remains partial.

[model-plan.json](../../../web/platform/assets/m5stack-s007/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| cad-component-0 source-fidelity | 7.152557390810443e-08 | 0 | 通过 |
| board-outline source-fidelity | 4.7328850527605937e-07 | 0 | 通过 |
| usb-tongue source-fidelity | 4.5377086266285005e-07 | 0 | 通过 |
| usb-shell source-fidelity | 4.7617376353059626e-07 | 0 | 通过 |
| usb-insulator source-fidelity | 4.766015493373743e-07 | 0 | 通过 |
| antenna source-fidelity | 4.7563936789174477e-07 | 0 | 通过 |
| cover-screw source-fidelity | 2.3741007737498876e-07 | 0 | 通过 |
| width | 18.000022888183594 | 18 | 通过 |
| height | 25.947933197021484 | 24 | 未通过 |
| depth | 4.750001907348633 | 4.7 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-s007/verification.json)：

- component-side: qualitative-only; IoU 0.9168698555873984
- label-side: qualitative-only; IoU 0.9162908522602491
- USB-end: qualitative-only; IoU 0.2530992665256716
- shield-and-headers: qualitative-only; IoU 0.4491614537360402

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01-extra.py prepare --ids m5stack-s007
python tools/onboard-lane-01-extra.py build --ids m5stack-s007
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-s007`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-s007-front.png` 等，再运行：

```sh
python tools/onboard-lane-01-extra.py verify --ids m5stack-s007
python tools/onboard-lane-01-extra.py publish --ids m5stack-s007
```

当前限制：

- Manufacturer STL is a single connected heat-resistant shield; no separate PCB assembly or component identities can be certified from its connectivity.
- Different official photographs show optional heat shield and pin headers. Those alternatives are not silently combined into this source configuration.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Browser inspection identifies the connected STL as a heat-resistant shield; actual populated PCB geometry and edge pad assignments remain unknown.
- 本轮继续按真实厂家照片审核；刻字、部分贴纸、端子及内部PCB未全部还原，不能视为整机1:1通过。
- Officially linked mechanical STEP has board outline, USB, antenna and screw but lacks a full populated PCB. Exact component shapes retained; neutral UNKNOWN label remains on unverified board, no chip layout invented.
