# M5Stack Stamp-S3 PIN1.27 · S007-PIN127

ESP32-S3微型控制模块，配8MB Flash、USB-C与1.27mm间距扩展引脚；此条目保持PIN1.27版本身份。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：One manufacturer cover mesh, native centered XY, common -1.3mm Z translation only. Dimension drawing shows 18x24x4.7mm body; 26mm specification length includes projecting USB-C, while one spec depth says4.6mm. The cover is not stretched to match the whole board. Exact PIN1.27 official documentation also links the fixed-revision Stamp-S3 STEP. Selected STEP solids add mechanical PCB outline, USB, antenna and screw; circuitry/header population is not copied from a sibling SKU. Native STEP coordinates share the cover frame. STEP cover solid27 is excluded due to its angular offset; exact STL cover retained.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 10 项，0 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-s007-pin127/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-s007-pin127/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-s007-pin127/source-audit.json)。复现：`python tools/onboard-lane-02.py prepare --ids m5stack-s007-pin127`、`build --ids m5stack-s007-pin127`，然后使用 `review-lane-02.html?id=m5stack-s007-pin127` 截取对应正背侧视，运行 `verify --ids m5stack-s007-pin127`。依赖见 `tools/requirements-model.txt`。

限制：

- Only source-backed exterior preview; board firmware execution is unavailable.
- PCB UNKNOWN is a blank estimated placeholder; visible inserts are approximate unless independently dimensioned.
- Reference photographs have uncalibrated lighting/perspective; no whole-product 1:1 pass or physical hardware validation is claimed.
- Official photos expose populated PCB and optional long pin headers; these are not supplied by the single cover STL. The model intentionally retains UNKNOWN instead of inventing their positions.
- Available photos are oblique; orthographic front/back silhouette verification is unavailable in this source set.
- Manufacturer STEP adds board mechanical outline and USB/antenna/screw geometry. It does not establish populated board layout or precise PIN1.27 header geometry.
- PIN1.27 header population and mechanical dimensions are not reconstructed. A PIN2.54-derived header draft was rejected because it belongs to a different SKU; only the exact PIN1.27-page linked STEP mechanical outline/USB is retained.
