# Stamp-Pico Mate · K051 模型

保留 K051 未贴纸盖板的外观与原始厂家三角形，补充可见天线顶面、M2×4 沉头内六角螺丝、带通孔的黑色 PCB 及该 SKU 官方背面照片。盖板上的 M5 结构仍来自原始 CAD，随外壳材质变化；天线、螺丝和板面材质独立。

K051 包含一块核心板、9P/6P 公母排针、90° HY2.0-4P 插座、1.5 mm 内六角匙和耐高温贴纸。官方照片分别展示了公母排针装法，模型不把这些散件同时预装，也未复制 C050-B 已贴纸外观。散件资料保留，但没有完整几何。

板厚 1 mm、孔径和孔距来自该产品链接的官方尺寸图。外露天线顶面仅按该 SKU 正视照片建立轮廓；厚度、Z 位置、背面环厚度与螺丝细节仍有估算，隐藏的天线折脚、完整 PCB 元件布局和层叠未知。正面的 `UNKNOWN` 避开盖板按钮槽与真实孔洞，正式页面“分开查看”后拖动旋转可完整读到；恢复视角会还原装配。

[验证记录](../evidence/m5stack-k051/model-refresh-2026-11-04.json)包含逐文件一致的离线重建、背面贴图逐像素承载面检查、原色/金色/半透明紫色、紧固件与天线近景，以及正式页面展开截图。照片贴图不是独立 PCB CAD 验证。原 CAD 厚 4.4 mm 与官方完整产品 4.6 mm 差异继续保留，整体仍为部分外观还原，不代表固件或实机兼容。

```sh
python tools/rebuild-product-model.py build m5stack-k051 --scratch <fresh-directory>
python tools/check-stamp-pico-surfaces.py m5stack-k051
```

工具依赖见[模型重建](../model-reproduction.md)；[模型计划](../../../web/platform/assets/m5stack-k051/model-plan.json)固定来源、坐标、板形和观察组。[此前验证](../evidence/m5stack-k051/verification-before-refresh.json)及无 `refresh-` 前缀图片属于旧版本。CAD 为厂家 MIT；照片和派生纹理仍保留厂家权利。
