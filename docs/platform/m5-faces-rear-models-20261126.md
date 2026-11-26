# Faces Kit 与 StackChan Core 的两件模型修订

基于 `55078007161db22bfe6f61cd8796e19658fcc4e3`，本次只交付有证据的两个修订。K005 从没有 `model.url` 的 2D 条目增加部分套件模型；C156 补精确型号背面照片可证实的纸贴和可见小件。产品运行配置、固件列表、默认固件与试运行结果没有变更。

| SKU | 修订 | 仍未知 |
| --- | --- | --- |
| K005 | Gray 上部、A009 长底座、三块面板、35/20/8 独立键帽和独立充电座；四条纸贴独立承载 | 完整 K005 工程 CAD、整套尺寸、装配 Z、隐藏结构、I²C 面板行为 |
| C156 | 后纸贴不规则 alpha 遮罩；避开两电容、螺钉和电池口；两电容端面、四个黄铜嵌件可见端环单独材质 | 真实 PCB Z、元件高度、嵌件隐藏长度、精确板级 CAD |

C156 的纸贴放在估计的 48×48 mm 主板背面（z=-1.5 mm）而非把整张后照片投到壳体、孔壁和内部卡扣；平面向外偏置 0.015 mm。照片 crop `[233,230,964,971]` 以 54 mm 壳体外框为参考，后视 X 镜像。透明区不含塑料、板底、黄铜环、电容、连接器与螺钉。3 mm 电容高度仅是显式估计；端环只表示可见的 0.6 mm 端部，不外推螺纹长度。原件号 C156 与 CoreS3 家族 CAD 对应关系继续保留为 partial。

原厂照片版权与 Core1/CoreS3 CAD 授权分别记录。K005 原始 DWG/CDR、家族 STL、已装配网格快照和本次使用照片留在本地资产，源 URL、实际 UTC、大小与 SHA-256 留在 plan。FACES 固件源码只引用固定提交和抓取 hash，并未因公开仓库就推定许可或复制整份源码。

K159 已审阅独立电池/显示器 FPC 照片，但没有足够资料确定它们在装配后的坐标和路由，本轮不添加猜测结构。U215 厚度与已记录的完整官方包络有差异，未用整体拉伸伪装验收。因此本轮确有可靠证据的模型修复只有两件，不凑五件。

检查命令与最终 hash：

```
python tools/build-m5-missing-models.py --output /tmp/m5-faces-rear m5stack-k005 m5stack-c156
node tools/check-m5-faces-rear-models.mjs
python tools/validate-platform-catalog.py
node --test tools/platform-knowledge-test.mjs tools/platform-watchlist-test.mjs tools/platform-config-test.mjs tools/platform-catalog-loading-test.mjs tools/platform-model-binary-test.mjs tools/platform-model-inspection-test.mjs
node tools/check-evidence-privacy.mjs
```

最终 loader 记录见 [loader-checks.json](evidence/m5-faces-rear-20261126/loader-checks.json)。离线重建的 `model.json`/几何/纹理与已交付输出逐字节比较。Node 检查使用真正生产 loader 和 CPU 射线；图片解码与 canvas 使用替身，没有 WebGL。未完成同视角照片叠图、浏览器文字可读性、接口实际插配、固件运行或实机验收。本次几何检查不能作为上述项目的通过结论。
