# M5Stack NanoH2 · C149

基于ESP32-H2的微型Thread/Zigbee开发板，提供Grove、USB-C、用户按键、RGB与红外发射；本平台尚无H2执行模型。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Native assembled components 0 and 2 retained; components 1/3 are Y-translated printing copies. Rotate +90 degrees about Z and center by a rigid translation only, with USB at bottom and Grove at top per dimension drawing. Exterior CAD length is 23.4698mm versus official rounded 23.5mm, within predeclared 0.05mm comparison tolerance.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 5 项，0 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-c149/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-c149/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-c149/source-audit.json)。复现：`python tools/onboard-lane-02.py prepare --ids m5stack-c149`、`build --ids m5stack-c149`，然后使用 `review-lane-02.html?id=m5stack-c149` 截取对应正背侧视，运行 `verify --ids m5stack-c149`。依赖见 `tools/requirements-model.txt`。

限制：

- Manufacturer exterior CAD only; CPU, display, touch, radio and board-level firmware behavior are unavailable for this product.
- Materials are photograph-informed display approximations, not calibrated manufacturer material data.
- PCB UNKNOWN is an estimated blank placeholder, excluded from internal 1:1 claims; no inferred chips, copper or manufacturing holes.
- No physical device measurements or hardware flashing performed.
- PinMap header names ESP32-C6FH4 although SoC specification and product identify ESP32-H2FH4S; no C6 runtime or pin assumptions imported.
- Grove and USB visual inserts are estimated from official end views, separate from the two manufacturer shell parts; pin/contact dimensions are not manufacturing-verified.
- Paper markings use explicit transparent source-photo masks; unlabelled shell remains a recolorable material. Photo lighting and minor paper placement are not calibrated.
