# M5Stack PaperS3 · C139

ESP32-S3触摸电子纸设备，配4.7英寸960×540屏、16MB Flash、8MB PSRAM与1800mAh电池。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Components0/3/4 already assembled. Common -3.85mm Z translation only; distant copies1/2/5 omitted. Drawing separately gives121.5mm body and6mm projecting bottom hook,67mm body and0.7mm button. Total height127.5mm is not confused with body specification.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 8 项，0 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-c139/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-c139/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-c139/source-audit.json)。复现：`python tools/onboard-lane-02.py prepare --ids m5stack-c139`、`build --ids m5stack-c139`，然后使用 `review-lane-02.html?id=m5stack-c139` 截取对应正背侧视，运行 `verify --ids m5stack-c139`。依赖见 `tools/requirements-model.txt`。

限制：

- Only source-backed exterior preview; board firmware execution is unavailable.
- PCB UNKNOWN is a blank estimated placeholder; visible inserts are approximate unless independently dimensioned.
- Reference photographs have uncalibrated lighting/perspective; no whole-product 1:1 pass or physical hardware validation is claimed.
- Rear photo says V1.2; image appearance is not used to establish the users hardware revision.
- External PCB pads/battery connector are documented but not individually modeled; no populated board CAD available.
- The active screen-window size/center is a visual estimate from the exact official photos; the matching ET047TC1 panel name alone is not treated as proof of a calibrated PaperS3 window measurement.
- Paper markings use explicit transparent source-photo masks; unlabelled shell remains a recolorable material. Photo lighting and minor paper placement are not calibrated.
