# M5Stack Atom VoiceS3R · C126-ECHO

带 ES8311 音频编解码、MEMS 麦克风、扬声器和按键的 ESP32-S3-PICO 语音模块。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Retained source assembled cluster; only the recorded rigid rotation and translation are applied. Original millimetres and relative positions preserved.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 5 项，0 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-c126-echo/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-c126-echo/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-c126-echo/source-audit.json)。复现：`python tools/onboard-lane-02-extra.py prepare --ids m5stack-c126-echo`、`build --ids m5stack-c126-echo`，然后使用 `review-lane-02.html?id=m5stack-c126-echo` 截取对应正背侧视，运行 `verify --ids m5stack-c126-echo`。依赖见 `tools/requirements-model.txt`。

限制：

- Speaker perforations and button cutout are manufacturer mesh; the speaker/microphone interior and visible connector inserts are not supplied as assembly CAD.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
