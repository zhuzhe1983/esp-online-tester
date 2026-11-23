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

厂家 CAD 使用 MIT。官方照片与背贴原稿保留厂家版权。屏幕使用实时固件画面。K016-C 的板级支持范围见 [运行记录](../stick-k016-c-runtime.md)。

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
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- 本轮继续按真实厂家照片审核；刻字、部分贴纸、端子及内部PCB未全部还原，不能视为整机1:1通过。
- Source separated shells are only provisionally assembled by recorded translation; rear small pieces are fasteners, not user keys. No exact enclosure assembly certification.

## 顶部排母、底部接口和背贴复验

顶部原有开口宽约 21.05 mm。8 个内凹方形插孔按厂家给出的 2.54 mm 间距排在壳体开口内，口沿外法向为模型 +Y。底部 Type-C 与 Grove 分别贴合原有开口的前侧与后侧，朝模型 −Y。Type-C 金属口围绕内缩舌片，Grove 保留带锁扣缺口的塑料座及内部触点，制造商照片可见的橙色分隔条遮住原先露出的占位 PCB 边。插入深度、可见触点和分隔条按外观估算。两口及排母用于外观显示。[厂家顶部端子照片](https://m5stack.oss-cn-shenzhen.aliyuncs.com/image/m5-docs_content/core/m5stickc_04.png) · [厂家产品图](https://docs.m5stack.com/en/core/StickC)

背贴取自同款设备的 3007 × 5314 像素原稿，使用原图透明边界生成 1495 × 2657 像素纹理。背贴面约 20.42 × 36.28 mm，按官方背照对齐顶部排母及下方紧固件。旧 CAD 缺失的两小段承载面按对应照片和现有外壳轮廓补齐。正面、顶部、底部、背面和斜侧面的 Chromium 检查确认了插孔和贴边，原色、金色和半透明紫色下背贴保持原色。[厂家背贴原稿](https://static-cdn.m5stack.com/resource/docs/products/core/m5stickc/m5stickc_04.webp)

分开查看时，后壳、背贴、承载面、可见紧固件和底部装饰框沿模型 −Z 同步移动，PCB 占位和接口留在原装配坐标。12 mm 是查看偏移。原始 STL 与导出的原厂网格保持原有哈希，本次仅更新派生件和贴图，原有外壳尺寸差异仍按前表保留。[复验数据](../evidence/m5stack-k016-c/connector-decal-20261123.json) · [前后对照](../evidence/m5stack-k016-c/connector-decal-20261123.webp)

当前模型可用 `python tools/rebuild-product-model.py build m5stack-k016-c --scratch <新目录>` 从仓库提供的文件离线重建。重建覆盖派生件、分开查看分组、贴图及每个已固定的素材。
