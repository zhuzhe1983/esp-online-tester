# Stamp-Pico DIY Kit · K051-B 模型

K051-B 保留独立 DIY Kit 身份：一块 Stamp-Pico、下载器、排针、Grove 插座、内六角匙和纸贴；模型展示其中已贴纸的核心板，散件未声称预装或完整建模。

纸贴使用 K051-B 自己的官方照片，顶部标识与 C050-B 不同。独立的 17.6×22.4 mm 薄纸承载面保留真实跨槽纸面，并在中心螺丝处开孔；校正四角和孔位、透明遮罩均写入[模型计划](../../../web/platform/assets/m5stack-k051-b/model-plan.json)。纸厚 0.045 mm 和边界仍为照片估算，约 ±0.35 mm。

中心 M2×4 沉头内六角螺丝有独立实体及真实六角开口；背面金属环保留照片中的可见细节。黑色 PCB 背面使用该 SKU 官方照片，板厚 1 mm 与通孔受官方尺寸图约束；内部元件面仍是 `UNKNOWN`，没有用照片冒充完整板级 CAD。隐藏螺纹啮合、背环厚度和小尺寸细节仍有估算。

正式页面“分开查看”后旋转可以完整读到 `UNKNOWN`，恢复装配和恢复视角会归位；金色、半透明紫色只改变外壳，纸面、PCB 和螺丝不受影响。原始厂家三角形未改动，原 CAD 厚 4.4 mm 与官方整机 4.6 mm 的差异继续保留。

[本次验证](../evidence/m5stack-k051-b/model-refresh-2026-11-04.json)记录最终哈希、离线重建、两个纹理的逐像素承载面检查、照片对照、近景、材质及正式页面展开截图。[此前验证](../evidence/m5stack-k051-b/verification-before-refresh.json)与未带 `refresh-` 前缀图片属于旧版本。整体为部分外观还原，没有固件执行或实机兼容结论。

```sh
python tools/rebuild-product-model.py build m5stack-k051-b --scratch <fresh-directory>
python tools/check-stamp-pico-surfaces.py m5stack-k051-b
```

依赖见[模型重建](../model-reproduction.md)。厂家 CAD 为 MIT；照片与派生纹理保留厂家权利。后续运行适配使用[按键与 RGB 位置记录](../evidence/stamp-pico-control-surfaces.json)：已贴纸型号的透光情况未证实，不为灯效凭空开孔。
