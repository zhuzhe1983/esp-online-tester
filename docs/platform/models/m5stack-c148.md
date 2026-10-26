# M5Stack PowerHub · C148

ESP32-S3 与 STM32G031 组成的多路电源与总线控制主机，支持 RS485、CAN、USB、电流监测和 NP-F 系列电池。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Native shell assembly rotated180degrees about Z so antenna at top left, USB output at left, buttons at right as official front. Optional source battery envelope6 omitted for C148 without battery. All selected CAD triangles retain millimetres/relative placement. External connector and antenna estimates separately marked.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 9 项，1 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-c148/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-c148/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-c148/source-audit.json)。复现：`python tools/onboard-lane-02-extra.py prepare --ids m5stack-c148`、`build --ids m5stack-c148`，然后使用 `review-lane-02.html?id=m5stack-c148` 截取对应正背侧视，运行 `verify --ids m5stack-c148`。依赖见 `tools/requirements-model.txt`。

限制：

- Official gallery asset filenames use C124/K124 legacy names; they are linked by the current C148 document. They do not establish another SKU equivalence.
- Manufacturer shell/source inserts have 32.8 mm depth versus official 38.5 mm with protrusions; antenna/terminal complete geometry is not supplied and the failure is retained.
- NP-F battery is optional and is not silently added to the base product model.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Source CAD without optional battery is32.8mm deep, while specified product38.5mm includes the antenna; no CAD scaling used. The external antenna envelope is photo-derived, exact hidden SMA connection unknown.
- Grove, USB, DC, CAN/RS485 and expansion contacts are explicit exterior estimates. No cable is enabled without an independently verified transformed wire order; no STM32 power-management behavior is simulated.
- Rear and side printed labels are not reconstructed from oblique photographs; missing text remains a limitation.
