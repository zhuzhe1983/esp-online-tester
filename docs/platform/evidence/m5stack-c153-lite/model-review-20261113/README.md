# PaperMono-Lite 外观与输入映射修正

固定 Lite 原厂 STL 与照片保持独立来源，没有借用 Full 的 NFC/LoRa 标识。补充保留的官方尺寸图和底视照片与原有来源 hash 完全一致，[重新核对记录](source-rechecks.json)另记本次获取时间。

本次修正：

- 前盖原来顶面 Z=3.3 mm，低于 CAD 安装沿 Z=3.4 mm，导致安装结构穿出。改为图纸的 59×99 mm 圆角盖板，顶面 Z=3.8 mm，屏幕 Z=3.82 mm。厚度和装配深度为照片估计，屏幕仍使用运行时纹理。
- 按 Lite CAD 的侧面胶囊开口加入 8×2 mm 光窗，保留 0.038 mm 开口余量。`ledIndex: 0` 接现有客体 RGB 输出通道，使用生产 RGB 渲染函数。红、绿、青及熄灭均检查；不添加未经校准的光晕。
- 为实际 A/B 键帽补上 `controlId`，侧面最近射线命中对应输入。电源键仅保留外观，未伪造电源控制。
- 背盖原有法线与绕序正确。将薄平面改为独立实体，按官方图纸采用 58.9×34.9 mm、R2 圆角；黑色材质随场景受光，不烘焙照片亮斑。厚度和粗糙度仍为估计。
- 背贴遮罩原先在原图 Y=922 截断，修到纸边 Y=944，恢复 USB-C／TF 标注；黑盖区域仍透明，不把黑盖照片当纸贴。

[验收记录](verification.json)绑定最终模型、计划和资产 hash。原厂壳体与三个键帽的 267,264 字节几何与基线 `44a4cb59` 完全一致；9 个资产及模型配置离线重建通过。原生 CAD 的 8 项尺寸检查通过，实际实体装配包围盒为 62×101×8.025 mm，其中背面多出的 0.025 mm 是避免共面的渲染偏移，官方厚度仍是 8 mm。

无头浏览器截取原色、金色、紫透正背侧视，以及上下、右侧、斜视、LED 四状态，共 19 张；普通视图保留整机，仅 `near-side` 有意放大局部。纸贴、前盖、黑盖、键帽、屏幕和 LED 均不随外壳改色。浏览器错误和警告均为零。截图保留在忽略的 `output/playwright/`，记录中保存路径、字节数及 hash；旧版外观证据不覆盖。

复现：

```sh
python tools/rebuild-product-model.py build m5stack-c153-lite --scratch output/lite-model-rebuild
node --test tools/platform-papermono-lite-model-test.mjs tools/platform-manufacturer-model-test.mjs tools/platform-rgb-led-view-test.mjs
python tools/validate-platform-catalog.py
```

模型工具依赖见 `tools/requirements-model.txt`。启动仓库静态服务后，用 Playwright 技能的跟踪 wrapper 无头打开 `web/platform/review-lane-02.html?id=m5stack-c153-lite`，将 `tools/papermono-lite-model-review.js` 的函数交给 `run-code`，结束后用同一 wrapper 会话执行 `close` 并检查退出。当前任务的浏览器和静态服务均已关闭。

本轮是几何、材质及生产渲染/输入映射验收，没有运行新固件或连接实机。给生产 RGB 函数提供测试值不等于客体产生了新输出；既有固件结果以 [PaperMono 运行范围](../../../papermono-runtime.md)为准。内部 PCB 仍标为 UNKNOWN，材料颜色、反射率和光窗厚度未经实机校准，不宣称整机 1:1 还原。
