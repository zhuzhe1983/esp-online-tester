# M5Stack StickC-Plus · K016-P

ESP32-PICO-D4手持终端，1.14英寸135×240屏、MPU6886、RTC、蜂鸣器和120mAh电池。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Selected manufacturer components retain their native assembled relationship; one common rigid transform only. Omitted components are separately recorded. No nonuniform scale or geometry fitting. Exterior face inspection shows the product front is native -Z; rotate180deg aboutY before centering. No dimension fit.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 5 项，0 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-k016-p/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-k016-p/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-k016-p/source-audit.json)。复现：`python tools/onboard-lane-02.py prepare --ids m5stack-k016-p`、`build --ids m5stack-k016-p`，然后使用 `review-lane-02.html?id=m5stack-k016-p` 截取对应正背侧视，运行 `verify --ids m5stack-k016-p`。依赖见 `tools/requirements-model.txt`。

限制：

- The original factory image has a separately verified partial runtime. [K016-P runtime](../stickc-plus-runtime.md)
- Photograph-informed material colors are uncalibrated display approximations.
- PCB UNKNOWN is an estimated blank placeholder, not a populated or manufacturing-ready board.
- External connector/body inserts are photo-derived estimates where manufacturer CAD omits them.
- No physical hardware measurements or flashing performed.
- Front reference has an independent coin and off-center placement; qualitative comparison only until a fixed device crop is documented.
- Paper markings use explicit transparent source-photo masks; unlabelled shell remains a recolorable material. Photo lighting and minor paper placement are not calibrated.

## 上下端接口与背贴复查

K016-P 的原厂尺寸图显示顶部是八孔 HAT 母座，节距 2.54 mm。模型已补齐黑色母座、八条独立孔道和内侧底面。底部 Type-C 和 Grove 都沿原厂 24 mm 壳宽中线布置。Type-C 在屏幕侧，带凹入的金属套和舌片。白色 Grove 在背侧，保留两侧卡耳、四个触点和键槽朝向。浏览器以射线穿过八个方孔，并分别测得 Type-C、Grove 的可见槽深约 3.20 mm、4.79 mm。深处装配尺寸依照图像估算。[原厂资料](https://docs.m5stack.com/en/core/StickC-Plus)

原背贴跨过壳体 STL 缺失的上端承载区。按同型号原厂背面照片校正印刷边界后，模型补了上段承载片和磁铁位置，保留两个黄铜安装环和两枚银色螺丝。实测 2,233 个不透明纸面样点都有支撑，最大间隙为 0.025 mm。原色、金色、紫透和两端近景已对照。背贴之外的原厂壳仍可换材质，装配包围盒为 24 × 48 × 13.5 mm。Grove 线序见官方资料，插头装配方向尚未实物核准，连线仍关闭。

爆炸图沿厚度展开为前壳、PCB 和后壳三层。背贴、承载片、磁铁、背部紧固件和 B 键区域随后盖移动，HAT、USB-C、Grove 及其可见卡耳随 UNKNOWN 板一起移动。

可离线重生成模型和贴图。运行 `python tools/rebuild-product-model.py build m5stack-k016-p --scratch <空目录>`，核对原 CAD、原始照片和生成文件的 10 个 SHA-256。[前后对照图](../evidence/m5stack-k016-p/connector-and-label-comparison-20261123.webp) · [验模记录](../evidence/m5stack-k016-p/connector-and-label-20261123.json)
