# M5Stack StickS3 · K150 外观与知识入库

ESP32-S3-PICO 棒状终端，集成彩屏、BMI270、音频、红外收发与 250 mAh 电池。

来源：[官方产品文档](https://docs.m5stack.com/en/core/StickS3)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k150.json) 与 [source audit](../evidence/m5stack-k150/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Retained source assembled cluster; only the recorded rigid rotation and translation are applied. Original millimetres and relative positions preserved.

[model-plan.json](../../../web/platform/assets/m5stack-k150/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| cad-component-0 source-fidelity | 9.262561810885472e-07 | 0 | 通过 |
| cad-component-3 source-fidelity | 4.196167004977269e-07 | 0 | 通过 |
| cad-component-4 source-fidelity | 9.512901293362575e-07 | 0 | 通过 |
| width | 24.000938415527344 | 24 | 通过 |
| height | 48.001869201660156 | 48 | 通过 |
| depth | 15.0 | 15 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k150/verification.json)：

- front: pass; IoU 0.9898767848282706
- back: pass; IoU 0.9754841077174956
- USB-Grove-end: qualitative-only; IoU 0.7681301352809362
- button-side: qualitative-only; IoU 0.4291570063989613

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 按 MIT 许可保留。官方照片及其裁切纹理版权归厂家，独立开放图片许可尚未核实。屏幕由模拟固件输出。`sticks3` 板型为部分适配，官方 UIFlow2 和小智均待下载并验证启动。[板级报告](../sticks3-runtime.md)

下列命令使用已保留的厂家原始文件，在独立目录重建模型。

```sh
python tools/rebuild-product-model.py plan m5stack-k150
python tools/rebuild-product-model.py build m5stack-k150 --scratch "$(mktemp -d)"
```

使用仓库要求的 Playwright wrapper 打开 `/platform/review-lane-01.html?id=m5stack-k150`。下列命令运行本型号的浏览器几何检查。

```sh
"$PWCLI" run-code --filename=tools/fixtures/sticks3-model-review-check.js
```

当前限制：

- KEY1/KEY2 通过 GPIO11/12 接入；独立 PM1 电源键不映射为普通 GPIO，音频/IR 仍无板级路由验证。
- 原厂 STL 包含外壳和前键。排母、接口、可见紧固件及背贴按本型号照片与原厂开口补建，内部 PCB 仍为占位。
- `sticks3` 板型实现 ST7789P3/PM1 屏幕电源门/BMI270 轮询子集；外观预览不证明官方固件兼容。
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- 外观与原厂照片的核验覆盖可见接口和背贴。刻字、完整内部结构及制造公差仍待核验。

## 按键区域和诊断

厂家组件 `cad-component-3` 对应 KEY1/G11。KEY2/G12 的右侧命中平面根据本型号侧照和背贴估算，误差约 1 mm。Chromium 工作台默认视角通过两个按键区域的悬停、按下和释放检查，诊断固件分别输出红色和绿色。屏幕中心仍可用于旋转设备。嵌入页可恢复 K150 模型。内部 `pcb-main` 标记为 `UNKNOWN`。[诊断记录](../evidence/sticks3-runtime-20261122/verification.json)

## 端口与背贴复核

2026-11-23 依据厂家尺寸 PDF、原理图及正对上下端面的本型号照片补建连接器。坐标沿现有模型轴，正面为 +Z，排母端为 +Y。前后外壳三角形、屏幕、按键和刚体装配变换保持原始校验值。

| 位置 | 装配与观察结果 |
| --- | --- |
| 上端 Hat2 | 8 列、2 排凹孔，中心距 2.54 mm，壳体塑高按原理图为 8.5 mm。CAD 开口限制为约 X ±10.48、Z −7.30 至 −2.30 mm。16 个孔中心都能通过外壳并抵达虚拟凹槽底部。凹槽深度及内部触点属于外观估算。 |
| 下端 USB-C | 口沿从 Z +1.40 校正到 +2.35 mm，朝 −Y，退入原厂外壳开口。套筒内留空，黑色绝缘座沿金属口内缘保留开孔。 |
| 下端 Grove | 口沿从 Z −3.70 校正到 −2.94 mm，朝 −Y，卡扣缺口向 −Z。两侧可见的浅色金属卡片随照片轮廓补齐。实际测得浏览器射线进入开口后首碰内壁，连接权限保持为外观预览。 |
| 背贴 | 原贴图在凹槽上方约 0.218 mm。新图保留厂家 1200 像素源照片中的 343×639 区域，沿纸边遮罩，在后凹槽平面外只留约 0.016 mm。顶边贴合排母外缘，下缘避让原厂模具唇边与两组硬件。中央圆孔按照片和尺寸图中的磁性背部结构补入可见承载面的估算件。斜视与三种外壳材质均检查实测像素。 |
| 分开查看 | 后壳、背贴、磁片、可见紧固件同向移动 18 mm。主体外壳、占位板、USB-C、Grove 与排母位置固定。复位到原始装配坐标。位移只用于查看。 |

离线重建校验 10 件资产和模型配置。Chromium 量得装配外径 24.001 × 48.002 × 15.000 mm。纸贴有效区域测试 834 个点，其中 791 个有就近承载，43 个位于照片可见纸桥或磁片周缘的容差范围，壳体唇边遮挡和超出支撑区各为零。[数字回执](../evidence/m5stack-k150/connector-correction.json) · [端口及纸贴近景](../evidence/m5stack-k150/connector-correction.webp) · [三种材质及拆分](../evidence/m5stack-k150/connector-appearance.webp)
