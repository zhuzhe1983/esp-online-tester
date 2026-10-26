# M5Stack Paper v1.1 · K049-B

ESP32电子纸交互终端，配4.7英寸960×540触摸屏、1150mAh电池、温湿度传感器与三路Grove。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Components0 and3 share assembled coordinates;1 and2 are translated print-layout copies. Common -90deg Z rotation and center translation only. Export width67.0721mm differs by0.0721mm from67.0mm drawing; keep failed0.05mm check explicit.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 5 项，1 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-k049-b/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-k049-b/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-k049-b/source-audit.json)。复现：`python tools/onboard-lane-02.py prepare --ids m5stack-k049-b`、`build --ids m5stack-k049-b`，然后使用 `review-lane-02.html?id=m5stack-k049-b` 截取对应正背侧视，运行 `verify --ids m5stack-k049-b`。依赖见 `tools/requirements-model.txt`。

限制：

- Only source-backed exterior preview; board firmware execution is unavailable.
- PCB UNKNOWN is a blank estimated placeholder; visible inserts are approximate unless independently dimensioned.
- Reference photographs have uncalibrated lighting/perspective; no whole-product 1:1 pass or physical hardware validation is claimed.
- Joysticks, reset actuator and exposed terminal details are incomplete; dimensions and electrical controls stay in knowledge without invented operation.
- Source width misses independent0.05mm drawing tolerance by0.0221mm; no corrective geometry scaling applied.
- Paper markings use explicit transparent source-photo masks; unlabelled shell remains a recolorable material. Photo lighting and minor paper placement are not calibrated.
