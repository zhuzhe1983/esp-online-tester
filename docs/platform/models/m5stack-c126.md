# M5Stack AtomS3R · C126

紧凑型Atom模块，控制器、传感器、屏幕或音频配置以此SKU的官方资料为准。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本轮保留原几何与照片证据，补上屏幕按钮交互与 [部分板级模拟](../atoms3r-runtime.md)。官方固件可尝试运行，未逐一验证。

装配：Selected manufacturer components retain their native assembled relationship; one common rigid transform only. Omitted components are separately recorded. No nonuniform scale or geometry fitting.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 5 项，0 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-c126/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-c126/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-c126/source-audit.json)。复现：`python tools/rebuild-product-model.py plan m5stack-c126`、`python tools/rebuild-product-model.py build m5stack-c126 --scratch output/atoms3r-public-rebuild`。本轮只重建比较元信息与几何；历史照片轮廓测试未重跑，原失败结果保留。依赖见 `tools/requirements-model.txt`。

限制：

- Partial ST7735-batch runtime: SPI display, LP5562 direct white backlight, G41 button and BMI270 polled subset. BMM150 AUX, RGB output, IR and older GC9107 batch are not implemented.
- Photograph-informed material colors are uncalibrated display approximations.
- PCB UNKNOWN is an estimated blank placeholder, not a populated or manufacturing-ready board.
- External connector/body inserts are photo-derived estimates where manufacturer CAD omits them.
- No physical hardware measurements or flashing performed.
- Paper markings use explicit transparent source-photo masks; unlabelled shell remains a recolorable material. Photo lighting and minor paper placement are not calibrated.
- Visible rear female-header housing/recess outlines are photo-derived approximations; they are separate from the shell and UNKNOWN PCB. Exact crimp/contact internals are unknown.
