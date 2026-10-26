# M5Stack Basic v2.7 · K001-V27

ESP32基础主机，2英寸320×240屏、三枚实体按键、16MB Flash及110mAh电池。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Multi-file manufacturer OBJ bottom shell and STL front/side/button retain native coordinates. Global rigid X-rotation maps front+Y to+Z. The supplied single front key is repeated at x+12/+24mm matching the three equivalent photographed CAD openings. No shell deformation.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 193 项，1 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-k001-v27/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-k001-v27/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-k001-v27/source-audit.json)。复现：`python tools/onboard-lane-02.py prepare --ids m5stack-k001-v27`、`build --ids m5stack-k001-v27`，然后使用 `review-lane-02.html?id=m5stack-k001-v27` 截取对应正背侧视，运行 `verify --ids m5stack-k001-v27`。依赖见 `tools/requirements-model.txt`。

限制：

- Manufacturer exterior geometry only; exact product board/CPU/peripheral execution is unavailable.
- Photograph-informed material colors are uncalibrated display approximations.
- PCB UNKNOWN is an estimated blank placeholder, not a populated or manufacturing-ready board.
- External connector/body inserts are photo-derived estimates where manufacturer CAD omits them.
- No physical hardware measurements or flashing performed.
- Official gallery lacks clean matching orthographic front/back/side views; selected independent product photos include packaging/scale/oblique views, so silhouette scores are not acceptance proof. Bottom drawing is linked by thev2.7page from aBASICv2.6 resource; applicability is recorded rather than inferred as identical electronics.
- Rejected visual draft: estimated front-glass atZ8.55mm hid the three manufacturer keys whose tops areZ8.50mm. The plane is nowZ8.48mm; this is a visual depth correction, not measured glass thickness.
