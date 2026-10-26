# M5Stack Stamp-S3Bat DIP · S015-DIP

ESP32-S3-PICO-1-N8R8电池管理模块，8MB Flash/8MB PSRAM，9P×2.54mm排针、USB-C与电池接口。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Selected manufacturer components retain their native assembled relationship; one common rigid transform only. Omitted components are separately recorded. No nonuniform scale or geometry fitting.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 4 项，3 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-s015-dip/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-s015-dip/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-s015-dip/source-audit.json)。复现：`python tools/onboard-lane-02.py prepare --ids m5stack-s015-dip`、`build --ids m5stack-s015-dip`，然后使用 `review-lane-02.html?id=m5stack-s015-dip` 截取对应正背侧视，运行 `verify --ids m5stack-s015-dip`。依赖见 `tools/requirements-model.txt`。

限制：

- Manufacturer exterior geometry only; exact product board/CPU/peripheral execution is unavailable.
- Photograph-informed material colors are uncalibrated display approximations.
- PCB UNKNOWN is an estimated blank placeholder, not a populated or manufacturing-ready board.
- External connector/body inserts are photo-derived estimates where manufacturer CAD omits them.
- No physical hardware measurements or flashing performed.
- Source cover is17.76×29.62×5.5mm. The full DIP product18×29.8×12mm includes missing PCB and through-hole pins; full-product dimensions are intentionally not claimed passed.
