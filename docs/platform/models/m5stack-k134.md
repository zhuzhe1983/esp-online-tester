# M5Stack DinMeter · K134

面板式 ESP32-S3 仪表，带 1.14 英寸彩屏、旋转编码器、RTC 和宽压输入。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Retained source assembled cluster; only the recorded rigid rotation and translation are applied. Original millimetres and relative positions preserved.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 6 项，1 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-k134/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-k134/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-k134/source-audit.json)。复现：`python tools/onboard-lane-02-extra.py prepare --ids m5stack-k134`、`build --ids m5stack-k134`，然后使用 `review-lane-02.html?id=m5stack-k134` 截取对应正背侧视，运行 `verify --ids m5stack-k134`。依赖见 `tools/requirements-model.txt`。

限制：

- Orange mounting bracket extends beyond the main panel; official 53×30×32 mm does not state bracket-envelope convention. Unmatched overall dimensions remain failed.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- Default assembly includes the documented supplied back bracket; independent rear photo shows it removed, so silhouette comparison is not a like-for-like bracket configuration. No bracket/case scaling performed.
- Earlier draft showed one rear Grove and used a mirrored rear orientation. Rechecked exact rear photo: two white sockets, shown using camera up=-Y; prior single-port placement is rejected. No wire order inferred from this camera correction.

## PCB 穿模修正

旧版 48 × 25 × 1.6 mm 的 PCB 占位板放在外壳中部，与主壳实体相交约 284.39 mm³，从侧面可见灰色横条。根据厂家 STL 的后开口导轨和官方背照，将占位板收至 40.6 × 15.6 mm，保持 1.6 mm 的原估算厚度，板面移至后部，留出 Grove 座的模型空间。与主壳和原安装支架的体积相交均为零，主壳最小采样间隙约 0.29 mm。板形、厚度和位姿是由壳体限定的估算。实物背照中安装支架已拆下，侧面改动前后采用相同的装配状态和相机。 [官方产品页](https://docs.m5stack.com/en/core/DinMeter) · [测量记录](../evidence/m5stack-k134/pcb-clearance.json)

原色、金色和半透明紫色均通过 Chromium 检查。工作台点击「分开查看」后，前壳和后部硬件沿厚度方向分离，转动即可看清板上的 `UNKNOWN`。复位恢复原装配，旋钮区域仍可用。固件运行范围见单独的运行记录。 [前后对照](../evidence/m5stack-k134/pcb-clearance-comparison.webp) · [运行记录](../dinmeter-runtime.md)

使用 `python tools/verify-dinmeter-pcb.py --baseline da0738cd` 复核网格，需要 `numpy`、`trimesh`、`rtree` 和 `manifold3d`。用 Playwright 包装脚本打开本地平台，再运行 `run-code --filename=tools/fixtures/dinmeter-pcb-browser-check.js` 复验查看器。
