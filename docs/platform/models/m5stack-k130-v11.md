# M5Stack Dial v1.1 · K130-V11

ESP32-S3FN8旋钮交互终端，圆形240×240触摸屏、编码器与RFID。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Selected manufacturer components retain their native assembled relationship; one common rigid transform only. Omitted components are separately recorded. No nonuniform scale or geometry fitting. Corrected global Z-axis180degree presentation orientation against exact rear photo: power terminal +Y, USB -Y, A physical-X/red and B physical+X/black.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 12 项，3 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-k130-v11/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-k130-v11/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-k130-v11/source-audit.json)。复现：`python tools/onboard-lane-02.py prepare --ids m5stack-k130-v11`、`build --ids m5stack-k130-v11`，然后使用 `review-lane-02.html?id=m5stack-k130-v11` 截取对应正背侧视，运行 `verify --ids m5stack-k130-v11`。依赖见 `tools/requirements-model.txt`。

限制：

- Manufacturer exterior geometry only; exact product board/CPU/peripheral execution is unavailable.
- Photograph-informed material colors are uncalibrated display approximations.
- PCB UNKNOWN is an estimated blank placeholder, not a populated or manufacturing-ready board.
- External connector/body inserts are photo-derived estimates where manufacturer CAD omits them.
- No physical hardware measurements or flashing performed.
- Native source envelope is smaller than the complete product32.3mm depth; glass, contacts and rotating assembly tolerances not verified. Screen is explicitly round.
- Rejected earlier preview: USB pointed +Y and two Grove mouths were missing. Exact SKU back photo establishes2rearGrove ports, Ared/Bblack. Their width is alongY, normal-Z; still allowConnections:false until physical wire order/mating checked.
