# M5Stack PaperMono-Lite · C153-Lite

ESP32-S3电子墨水屏阅读与交互终端，配3.97英寸触摸屏、前光和1150mAh电池；Lite版不含NFC与LoRa。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Native assembled components 0/2/3/4 share coordinates. Common translation only; duplicated printing layout 1/5/6/7 omitted. Independent dimension drawing matches 61mm body, 62mm including side buttons, 101mm height and 8mm depth. Lite and full PaperMono shells share exterior dimensions; this SKU source is fetched separately.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 8 项，0 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-c153-lite/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-c153-lite/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-c153-lite/source-audit.json)。复现：`python tools/onboard-lane-02.py prepare --ids m5stack-c153-lite`、`build --ids m5stack-c153-lite`，然后使用 `review-lane-02.html?id=m5stack-c153-lite` 截取对应正背侧视，运行 `verify --ids m5stack-c153-lite`。依赖见 `tools/requirements-model.txt`。

限制：

- Manufacturer exterior CAD only; CPU, display, touch, radio and board-level firmware behavior are unavailable for this product.
- Materials are photograph-informed display approximations, not calibrated manufacturer material data.
- PCB UNKNOWN is an estimated blank placeholder, excluded from internal 1:1 claims; no inferred chips, copper or manufacturing holes.
- No physical device measurements or hardware flashing performed.
- Paper markings use explicit transparent source-photo masks; unlabelled shell remains a recolorable material. Photo lighting and minor paper placement are not calibrated.
